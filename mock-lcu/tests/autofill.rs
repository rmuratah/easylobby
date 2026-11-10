//! Aviso de autofill pelo `/ws` (#360), com o agente conectado no mock-lcu
//! parado na vez do jogador escolher (gravação 16.22): o jogador cai em
//! autofill, a UI vê o aviso, marca como visto e ele não volta nesta
//! seleção.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::champ_select::{ChampSelect, ChampSelectActionType};
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::{EventFixture, FixtureSet};
use easylobby_agent::ws::protocol::{ClientMessage, Command, CommandResult, ServerMessage};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

const SESSION: &str = "/lol-champ-select/v1/session";

/// A gravação cortada na vez do jogador escolher, já com a seleção aberta
/// no gameflow. Devolve também a sessão desse momento.
fn fixtures_until_champ_select() -> (FixtureSet, Value) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut set = FixtureSet::load(&dir).unwrap();
    let first = set
        .events
        .iter()
        .position(|e| {
            e.uri == SESSION
                && ChampSelect::from_json(&e.data)
                    .and_then(|cs| cs.pending_action)
                    .is_some_and(|a| a.action_type == ChampSelectActionType::Pick)
        })
        .expect("a gravação 16.22 tem seleção de campeões");
    set.events.truncate(first + 1);
    let session = set.events[first].data.clone();
    (set, session)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-autofill-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> (MockLcu, Value) {
    let (fixtures, session) = fixtures_until_champ_select();
    let events = fixtures.events.clone();
    let mut config = Config::new(fixtures);
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();
    for event in events {
        mock.emit(event);
    }
    (mock, session)
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

async fn connect(url: &str) -> Ui {
    tokio_tungstenite::connect_async(url).await.unwrap().0
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

/// Espera uma seleção que passe em `check`.
async fn wait_champ_select(ui: &mut Ui, check: impl Fn(&ChampSelect) -> bool) -> ChampSelect {
    loop {
        let cs = match recv(ui).await {
            ServerMessage::Hello { state, .. } => state.champ_select,
            ServerMessage::ChampSelect { champ_select } => champ_select,
            _ => continue,
        };
        if let Some(cs) = cs.filter(|cs| check(cs)) {
            return cs;
        }
    }
}

#[tokio::test]
async fn mostra_o_aviso_de_autofill_e_marca_como_visto_pelo_ws() {
    let dir = temp_dir("aviso");
    let (mock, session) = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;

    let cs = wait_champ_select(&mut ui, |_| true).await;
    assert!(!cs.autofill_notice, "a gravação 16.22 não tem autofill");

    assert!(mock.autofill_local_player());
    let cs = wait_champ_select(&mut ui, |cs| cs.autofill_notice).await;
    assert!(cs.local_player().unwrap().is_autofilled);
    assert!(!mock.autofill_displayed());

    send(&mut ui, 1, Command::AckAutofill).await;
    let mut hidden = false;
    loop {
        match recv(&mut ui).await {
            ServerMessage::ChampSelect {
                champ_select: Some(cs),
            } => hidden = !cs.autofill_notice,
            ServerMessage::Response { id: 1, result } => {
                assert_eq!(result, CommandResult::Ok);
                break;
            }
            _ => {}
        }
    }
    assert!(hidden, "a seleção sem o aviso chega antes da resposta");
    assert!(mock.autofill_displayed());

    // A LCU manda a sessão de novo (o timer andou, por exemplo): o jogador
    // continua em autofill, mas o aviso não volta.
    let mut again = session;
    let cell = again["localPlayerCellId"].clone();
    for player in again["myTeam"].as_array_mut().unwrap() {
        if player["cellId"] == cell {
            player["isAutofilled"] = true.into();
        }
    }
    again["timer"]["adjustedTimeLeftInPhase"] = 1234.into();
    mock.emit(EventFixture {
        t_ms: 0,
        uri: SESSION.into(),
        event_type: "Update".into(),
        data: again,
    });
    let cs = wait_champ_select(&mut ui, |cs| cs.timer.time_left_ms == 1234).await;
    assert!(cs.local_player().unwrap().is_autofilled);
    assert!(!cs.autofill_notice);

    mock.shutdown().await;
}
