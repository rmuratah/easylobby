//! Sair do easylobby encerra o LoL junto, contra o mock-lcu: com a UI
//! oficial fechada pelo agente, o Sair pede `/process-control/v1/process/quit`;
//! com a UI reaberta pelo botão de emergência, o LoL fica como está.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ws::protocol::{CommandResult, LcuStatus};
use easylobby_agent::ws::{self, ExitAction, Hub, WsConfig};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-sair-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

/// Sobe o agente com um [`Hub`] de fora, como o app desktop, e espera
/// conectar no mock.
async fn start_agent(dir: &Path) -> Hub {
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hub = Hub::new();
    tokio::spawn(ws::run_with_hub(
        lcu,
        listener,
        WsConfig::default(),
        hub.clone(),
    ));
    let connected = async {
        while !matches!(hub.state().lcu, LcuStatus::Connected { .. }) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(20), connected)
        .await
        .expect("o agente não conectou no mock a tempo");
    hub
}

#[tokio::test]
async fn sair_encerra_o_lol_com_a_ui_oficial_fechada() {
    let dir = temp_dir("encerra");
    let mock = start_mock(&dir).await;
    let hub = start_agent(&dir).await;
    assert!(!mock.ux_open());
    assert!(!mock.quit_requested());

    assert_eq!(
        hub.close_league_on_exit().await.unwrap(),
        ExitAction::QuitClient
    );
    assert!(mock.quit_requested());
    // Encerrar não passa pela UI oficial.
    assert!(!mock.ux_open());

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sair_com_a_ui_oficial_reaberta_deixa_o_lol_aberto() {
    let dir = temp_dir("emergencia");
    let mock = start_mock(&dir).await;
    let hub = start_agent(&dir).await;

    assert_eq!(hub.launch_ux().await, CommandResult::Ok);
    assert_eq!(
        hub.close_league_on_exit().await.unwrap(),
        ExitAction::KeepOpen
    );
    assert!(!mock.quit_requested());
    assert!(mock.ux_open());

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
