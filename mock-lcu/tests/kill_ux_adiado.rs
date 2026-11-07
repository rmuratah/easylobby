//! Fechamento adiado da UI oficial contra o mock-lcu: fechar a UI logo que o
//! LoL abre faz o Vanguard fechar o LoL com o erro VAN7003, então o agente
//! espera `kill_ux_delay` antes do primeiro `kill-ux`. O botão de emergência
//! durante a espera cancela o fechamento.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ws::protocol::{CommandResult, LcuStatus};
use easylobby_agent::ws::{self, Hub, WsConfig};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;

const DELAY: Duration = Duration::from_millis(600);

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-kill-ux-adiado-{name}-{}",
        std::process::id()
    ));
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

/// Sobe o agente com o fechamento adiado e espera conectar no mock.
async fn start_agent(dir: &Path) -> Hub {
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        kill_ux_delay: DELAY,
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

async fn until_ux_closed(mock: &MockLcu) {
    let closed = async {
        while mock.ux_open() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), closed)
        .await
        .expect("a UI oficial não fechou depois da espera");
}

#[tokio::test]
async fn ui_oficial_so_fecha_depois_da_espera() {
    let dir = temp_dir("espera");
    let mock = start_mock(&dir).await;
    let _hub = start_agent(&dir).await;

    // Logo depois de conectar a UI continua aberta para o Vanguard achá-la.
    assert!(mock.ux_open());
    until_ux_closed(&mock).await;

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn emergencia_durante_a_espera_cancela_o_fechamento() {
    let dir = temp_dir("emergencia");
    let mock = start_mock(&dir).await;
    let hub = start_agent(&dir).await;

    assert_eq!(hub.launch_ux().await, CommandResult::Ok);
    tokio::time::sleep(DELAY * 3).await;
    assert!(mock.ux_open());

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
