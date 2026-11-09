//! Personalizadas abertas (#264) pelo `/ws`, com o agente conectado no
//! mock-lcu e a gravação do 16.22: a UI pede a lista, entra numa
//! personalizada com senha (a errada é recusada pela LCU) e chega no lobby
//! dela sem ser o líder.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
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
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-personalizadas-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
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

async fn until_response(ui: &mut Ui, id: u32) -> (Vec<ServerMessage>, CommandResult) {
    let mut before = Vec::new();
    loop {
        match recv(ui).await {
            ServerMessage::Response { id: got, result } if got == id => return (before, result),
            other => before.push(other),
        }
    }
}

async fn wait_summoner(ui: &mut Ui) {
    loop {
        match recv(ui).await {
            ServerMessage::Hello { state, .. } if state.summoner.is_some() => break,
            ServerMessage::Summoner { summoner: Some(_) } => break,
            _ => {}
        }
    }
}

#[tokio::test]
async fn lista_e_entra_na_personalizada_com_senha() {
    let dir = temp_dir("entra");
    let _mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    send(&mut ui, 1, Command::RefreshCustomGames).await;
    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    let games = before
        .into_iter()
        .find_map(|m| match m {
            ServerMessage::CustomGames {
                custom_games: Some(g),
            } => Some(g),
            _ => None,
        })
        .expect("a lista chega antes da resposta");
    let names: Vec<&str> = games.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["5v5 da firma", "ARAM fechado", "1v1 no meio"]);
    let locked = &games[1];
    assert!(locked.has_password);
    assert_eq!(locked.owner, "Garen Top");

    // Sem senha, o agente nem chega na LCU; com a errada, a LCU recusa.
    let join = |id, password: Option<&str>| Command::JoinCustomGame {
        id,
        password: password.map(Into::into),
        as_spectator: None,
    };
    send(&mut ui, 2, join(locked.id, None)).await;
    let (_, result) = until_response(&mut ui, 2).await;
    assert!(
        matches!(
            result,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ),
        "{result:?}"
    );
    send(&mut ui, 3, join(locked.id, Some("errada"))).await;
    let (_, result) = until_response(&mut ui, 3).await;
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

    send(
        &mut ui,
        4,
        join(locked.id, Some(mock_lcu::custom_games::PASSWORD)),
    )
    .await;
    let (before, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    let lobby = before
        .into_iter()
        .rev()
        .find_map(|m| match m {
            ServerMessage::Lobby { lobby: Some(l) } => Some(l),
            _ => None,
        })
        .expect("o lobby chega antes da resposta");
    assert!(!lobby.is_leader);
    let custom = lobby.custom.expect("lobby de personalizada");
    assert_eq!(custom.settings.name, "ARAM fechado");
}
