//! Criar lobby e escolher fila pelo `/ws` (#17), com o agente conectado no
//! mock-lcu: a UI cria o lobby, troca a fila e sai, e vê cada mudança chegar
//! antes da resposta do comando.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::lobby::Queue;
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
    let dir = std::env::temp_dir().join(format!("mock-lcu-lobby-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    // Sem reprodução: o lobby só muda pelos comandos.
    config.replay_start = ReplayStart::Never;
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

fn lobby_queues(messages: &[ServerMessage]) -> Vec<Option<Queue>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Lobby { lobby } => Some(lobby.as_ref().and_then(|l| l.queue)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn cria_lobby_troca_a_fila_e_sai() {
    let dir = temp_dir("comandos");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    // Espera conectar e o agente reler o estado pelo REST (o invocador
    // chega junto com a releitura do lobby). A gravação começa fora de lobby.
    loop {
        match recv(&mut ui).await {
            ServerMessage::Hello { state, .. } if state.summoner.is_some() => {
                assert_eq!(state.lobby, None);
                break;
            }
            ServerMessage::Summoner { summoner: Some(_) } => break,
            _ => {}
        }
    }
    assert_eq!(mock.phase(), "None");

    // Cria o lobby: a mensagem `lobby` chega antes da resposta.
    send(
        &mut ui,
        2,
        Command::CreateLobby {
            queue: Queue::RankedSolo,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(lobby_queues(&before), [Some(Queue::RankedSolo)]);
    let lobby = before
        .iter()
        .find_map(|m| match m {
            ServerMessage::Lobby { lobby } => lobby.clone(),
            _ => None,
        })
        .unwrap();
    assert_eq!(lobby.queue_id, 420);
    assert_eq!(lobby.max_lobby_size, 2);
    assert!(lobby.is_leader);
    assert_eq!(lobby.members.len(), 1);

    // Troca a fila.
    send(&mut ui, 3, Command::CreateLobby { queue: Queue::Aram }).await;
    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(lobby_queues(&before), [Some(Queue::Aram)]);

    assert_eq!(mock.phase(), "Lobby");

    // Uma UI nova recebe o lobby no hello.
    let mut late = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            assert_eq!(state.lobby.and_then(|l| l.queue), Some(Queue::Aram));
        }
        other => panic!("esperava hello: {other:?}"),
    }

    // Sai do lobby.
    send(&mut ui, 4, Command::LeaveLobby).await;
    let (before, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(lobby_queues(&before), [None]);
    // A fase volta para `None` pelo WAMP (antes ou depois da resposta), e
    // nenhum lobby reaparece.
    let is_idle = |m: &ServerMessage| matches!(m, ServerMessage::GameflowPhase { phase } if phase.as_deref() == Some("None"));
    let mut seen = before;
    while !seen.iter().any(is_idle) {
        seen.push(recv(&mut ui).await);
    }
    assert_eq!(lobby_queues(&seen), [None], "{seen:?}");
    assert_eq!(mock.phase(), "None");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sem_lol_responde_not_connected() {
    let dir = temp_dir("sem-lol");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    send(
        &mut ui,
        1,
        Command::CreateLobby {
            queue: Queue::NormalDraft,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(matches!(
        result,
        CommandResult::Error {
            code: ErrorCode::NotConnected,
            ..
        }
    ));
}
