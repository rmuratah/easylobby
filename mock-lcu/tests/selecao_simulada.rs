//! Seleção de campeões simulada (#440) pelo `/ws`: o agente conectado no
//! mock-lcu com `--selecao draft` acompanha o draft inteiro, com os bots
//! agindo sozinhos, sem nenhum evento da gravação.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::champ_select::{ChampSelect, ChampSelectActionType, ChampSelectPhase};
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ws::protocol::ServerMessage;
use easylobby_agent::ws::{self, WsConfig};
use futures_util::StreamExt;
use mock_lcu::scenario::{Mode, Options, Timings};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-selecao-simulada-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
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

#[tokio::test]
async fn draft_simulado_do_planejamento_a_partida() {
    let dir = temp_dir("draft");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    config.champ_select = Some(Options {
        mode: Mode::Draft,
        cell: Some(1),
    });
    // Etapas curtas; o pick do jogador espera o `advance` do teste.
    config.timings = Timings {
        planning: 300,
        ban: 600,
        reveal: 200,
        pick: 60_000,
        finalization: 300,
        bot: 100,
    };
    let mock = MockLcu::start(config).await.unwrap();

    let mut open: Vec<ChampSelect> = Vec::new();
    loop {
        match recv(&mut ui).await {
            ServerMessage::ChampSelect {
                champ_select: Some(cs),
            } => {
                let my_pick = cs
                    .pending_action
                    .as_ref()
                    .is_some_and(|a| a.action_type == ChampSelectActionType::Pick);
                open.push(cs);
                if my_pick {
                    // O jogador "não escolhe": avançar dá um campeão a ele.
                    assert!(mock.advance_champ_select());
                }
            }
            ServerMessage::ChampSelect { champ_select: None } if !open.is_empty() => break,
            _ => {}
        }
    }

    let first = &open[0];
    assert_eq!(first.local_player_cell_id, 1);
    assert_eq!(first.my_team.len(), 5);
    assert_eq!(first.their_team.len(), 5);
    let mut phases: Vec<ChampSelectPhase> = open.iter().map(|cs| cs.timer.phase).collect();
    phases.dedup();
    assert_eq!(
        phases,
        [
            ChampSelectPhase::Planning,
            ChampSelectPhase::BanPick,
            ChampSelectPhase::Finalization,
            ChampSelectPhase::GameStarting,
        ]
    );
    let mut pending: Vec<ChampSelectActionType> = open
        .iter()
        .filter_map(|cs| cs.pending_action.as_ref())
        .inspect(|a| assert_eq!(a.actor_cell_id, 1))
        .map(|a| a.action_type)
        .collect();
    pending.dedup();
    assert_eq!(
        pending,
        [ChampSelectActionType::Ban, ChampSelectActionType::Pick]
    );
    // Todo mundo terminou com um campeão.
    let last = open.last().unwrap();
    assert!(last
        .my_team
        .iter()
        .chain(&last.their_team)
        .all(|p| p.champion_id.is_some()));
    assert!(!mock.champ_select_running());
    assert_eq!(mock.phase(), "InProgress");
}
