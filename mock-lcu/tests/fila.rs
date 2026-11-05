//! Entrar e sair da fila pelo `/ws` (#19), com o agente conectado no
//! mock-lcu: a UI cria o lobby, entra na fila, vê o tempo na fila subir e
//! sai.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::lobby::Queue;
use easylobby_agent::matchmaking::{Matchmaking, SearchState};
use easylobby_agent::ws::protocol::{
    ClientMessage, Command, CommandResult, ErrorCode, ServerMessage,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-fila-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    // Sem reprodução: o lobby e a fila só mudam pelos comandos.
    config.replay_start = ReplayStart::Never;
    // O relógio da fila anda 20 vezes mais rápido.
    config.speed = 20.0;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

async fn start_agent(dir: &Path) -> String {
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}{}", listener.local_addr().unwrap(), ws::WS_PATH);
    tokio::spawn(ws::run(lcu, listener, WsConfig::default()));
    url
}

async fn recv(ui: &mut Ui) -> ServerMessage {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(20), ui.next())
            .await
            .expect("o agente não mandou nada a tempo")
            .expect("o agente fechou o /ws")
            .unwrap();
        if let Message::Text(text) = frame {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

async fn send(ui: &mut Ui, id: u32, command: Command) {
    let json = serde_json::to_string(&ClientMessage { id, command }).unwrap();
    ui.send(Message::text(json)).await.unwrap();
}

/// Lê mensagens até a resposta do comando `id`, devolvendo as de antes e o
/// resultado.
async fn until_response(ui: &mut Ui, id: u32) -> (Vec<ServerMessage>, CommandResult) {
    let mut before = Vec::new();
    loop {
        match recv(ui).await {
            ServerMessage::Response { id: got, result } if got == id => return (before, result),
            other => before.push(other),
        }
    }
}

fn searches(messages: &[ServerMessage]) -> Vec<Option<Matchmaking>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Matchmaking { matchmaking } => Some(matchmaking.clone()),
            _ => None,
        })
        .collect()
}

/// Espera o agente conectar e reler o estado pelo REST.
async fn wait_connected(ui: &mut Ui) {
    loop {
        match recv(ui).await {
            ServerMessage::Hello { state, .. } if state.summoner.is_some() => {
                assert_eq!(state.matchmaking, None);
                return;
            }
            ServerMessage::Summoner { summoner: Some(_) } => return,
            _ => {}
        }
    }
}

#[tokio::test]
async fn entra_na_fila_conta_o_tempo_e_sai() {
    let dir = temp_dir("comandos");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_connected(&mut ui).await;

    // Fora de lobby, a LCU recusa.
    send(&mut ui, 1, Command::StartQueue).await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(
        matches!(
            result,
            CommandResult::Error {
                code: ErrorCode::Lcu,
                ..
            }
        ),
        "{result:?}"
    );

    send(&mut ui, 2, Command::CreateLobby { queue: Queue::Aram }).await;
    assert_eq!(until_response(&mut ui, 2).await.1, CommandResult::Ok);

    // Entra na fila: a busca chega antes da resposta.
    send(&mut ui, 3, Command::StartQueue).await;
    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    let search = searches(&before)
        .into_iter()
        .flatten()
        .next()
        .expect("a busca chega antes da resposta");
    assert_eq!(search.search_state, SearchState::Searching);
    assert_eq!(search.queue_id, 450);
    assert!(search.estimated_queue_time > 0.0);
    assert_eq!(mock.phase(), "Matchmaking");

    // O tempo na fila sobe, de segundo em segundo.
    let mut times = Vec::new();
    while times.last().is_none_or(|t| *t < 3.0) {
        if let ServerMessage::Matchmaking {
            matchmaking: Some(m),
        } = recv(&mut ui).await
        {
            assert_eq!(m.search_state, SearchState::Searching);
            times.push(m.time_in_queue);
        }
    }
    assert!(times.windows(2).all(|w| w[1] == w[0] + 1.0), "{times:?}");

    // Uma UI nova recebe a busca no hello.
    let mut late = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            let search = state.matchmaking.expect("na fila");
            assert!(search.time_in_queue >= 3.0);
            assert_eq!(state.lobby.and_then(|l| l.queue), Some(Queue::Aram));
        }
        other => panic!("esperava hello: {other:?}"),
    }

    // Sai da fila: `null` antes da resposta.
    send(&mut ui, 4, Command::StopQueue).await;
    let (before, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(searches(&before).last(), Some(&None));
    // A fase volta para `Lobby` pelo WAMP, o lobby continua e a busca não
    // volta.
    let in_lobby = |m: &ServerMessage| matches!(m, ServerMessage::GameflowPhase { phase } if phase.map(|p| p.as_str()) == Some("Lobby"));
    let mut seen = before;
    while !seen.iter().any(in_lobby) {
        seen.push(recv(&mut ui).await);
    }
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    send(&mut ui, 5, Command::GetState).await;
    let (before, _) = until_response(&mut ui, 5).await;
    match before
        .iter()
        .rev()
        .find(|m| matches!(m, ServerMessage::Hello { .. }))
    {
        Some(ServerMessage::Hello { state, .. }) => {
            assert_eq!(state.matchmaking, None);
            assert_eq!(
                state.lobby.as_ref().and_then(|l| l.queue),
                Some(Queue::Aram)
            );
        }
        _ => panic!("esperava hello: {before:?}"),
    }
    assert_eq!(mock.phase(), "Lobby");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sem_lol_responde_not_connected() {
    let dir = temp_dir("sem-lol");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    for (id, command) in [(1, Command::StartQueue), (2, Command::StopQueue)] {
        send(&mut ui, id, command).await;
        let (_, result) = until_response(&mut ui, id).await;
        assert!(matches!(
            result,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }
}
