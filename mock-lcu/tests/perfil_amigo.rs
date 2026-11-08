//! Perfil de outro jogador (#255) pelo `/ws`, com o agente conectado no
//! mock-lcu e o conjunto `exemplo`: a UI pede o perfil do amigo e recebe o
//! invocador, os ranks, a maestria e o histórico dele; um jogador que a LCU
//! não conhece responde `notFound`.

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

/// O amigo do conjunto `exemplo`.
const AMIGO: &str = "fake-puuid-0002";

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-perfil-amigo-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/exemplo");
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
async fn abre_o_perfil_do_amigo() {
    let dir = temp_dir("abre");
    let _mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    send(
        &mut ui,
        1,
        Command::OpenPlayerProfile {
            puuid: AMIGO.into(),
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    let profile = before
        .into_iter()
        .find_map(|m| match m {
            ServerMessage::PlayerProfile {
                player_profile: Some(p),
            } => Some(p),
            _ => None,
        })
        .expect("o perfil chega antes da resposta");
    assert_eq!(profile.summoner.puuid, AMIGO);
    assert_eq!(profile.summoner.summoner_level, 143);
    assert!(!profile.private_profile);
    let ranked = profile.ranked.expect("ranks do amigo");
    assert_eq!(ranked.solo_duo.tier.as_deref(), Some("PLATINUM"));
    assert_eq!(ranked.flex.tier, None);
    let ids: Vec<u32> = profile
        .champion_masteries
        .iter()
        .map(|m| m.champion_id)
        .collect();
    assert_eq!(ids, [103, 222, 412]);
    let history = profile.match_history.expect("histórico do amigo");
    assert_eq!(history.matches.len(), 10);
    assert!(!history.has_more);
    assert!(history.matches.iter().all(|m| m.player.puuid == AMIGO));

    // Um jogador que a LCU não conhece.
    send(
        &mut ui,
        2,
        Command::OpenPlayerProfile {
            puuid: "fake-puuid-9999".into(),
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 2).await;
    assert!(
        matches!(
            result,
            CommandResult::Error {
                code: ErrorCode::NotFound,
                ..
            }
        ),
        "{result:?}"
    );
}
