//! Jogadores sugeridos para convidar (#377) pelo `/ws`, com o agente
//! conectado no mock-lcu: a UI pede a lista, dispensa um e não consegue
//! dispensar quem não está nela.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::players::{SuggestedPlayer, SuggestedPlayerReason};
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
    let dir =
        std::env::temp_dir().join(format!("mock-lcu-sugeridos-{name}-{}", std::process::id()));
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

/// Espera o agente conectar e reler o estado pelo REST.
async fn wait_summoner(ui: &mut Ui) {
    loop {
        match recv(ui).await {
            ServerMessage::Hello { state, .. } if state.summoner.is_some() => break,
            ServerMessage::Summoner { summoner: Some(_) } => break,
            _ => {}
        }
    }
}

fn last_suggested(messages: &[ServerMessage]) -> Option<Vec<SuggestedPlayer>> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::SuggestedPlayers { suggested_players } => Some(suggested_players.clone()),
        _ => None,
    })
}

fn is_error(result: &CommandResult, expected: ErrorCode) -> bool {
    matches!(result, CommandResult::Error { code, .. } if *code == expected)
}

#[tokio::test]
async fn lista_e_dispensa_sugeridos_pelo_ws() {
    let dir = temp_dir("lista");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    send(&mut ui, 1, Command::LoadSuggestedPlayers).await;
    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    let list = last_suggested(&before).expect("a lista chega antes da resposta");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].reason, SuggestedPlayerReason::FriendOfFriend);
    assert!(!list[0].common_friend_name.is_empty());
    assert_eq!(list[1].reason, SuggestedPlayerReason::PreviousPremade);

    send(
        &mut ui,
        2,
        Command::DismissSuggestedPlayer {
            summoner_id: list[0].summoner_id,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let after = last_suggested(&before).expect("a lista nova chega antes da resposta");
    assert_eq!(after, list[1..]);

    // Relida, a LCU não traz de volta quem foi dispensado.
    send(&mut ui, 3, Command::LoadSuggestedPlayers).await;
    let (_, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);

    // Quem não está na lista: recusado sem falar com a LCU.
    send(
        &mut ui,
        4,
        Command::DismissSuggestedPlayer {
            summoner_id: list[0].summoner_id,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 4).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    mock.shutdown().await;
}
