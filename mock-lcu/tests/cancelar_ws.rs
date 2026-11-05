//! Cancelar o serviço `/ws` do agente (#58) contra o mock-lcu, com o runtime
//! Tokio ainda vivo: a conexão com o LoL tem de fechar junto, o LoL
//! reabrindo não pode receber a conexão de um serviço já cancelado, e
//! reiniciar o serviço não acumula conexões.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ws::{self, WsConfig};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-cancelar-ws-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Sobe o mock gravando lockfile e CA em `dir`. Cada chamada tem porta,
/// senha e CA novas, como um LoL que reabriu.
async fn start_mock(dir: &Path) -> MockLcu {
    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

/// Sobe o serviço como o app desktop: `ws::run` numa tarefa própria.
async fn start_service(dir: &Path) -> JoinHandle<std::io::Result<()>> {
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(50),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    tokio::spawn(ws::run(lcu, listener, WsConfig::default()))
}

/// Cancela a tarefa do serviço e espera ela terminar.
async fn cancel(service: JoinHandle<std::io::Result<()>>) {
    service.abort();
    let error = service
        .await
        .expect_err("o serviço não devia terminar sozinho");
    assert!(error.is_cancelled());
}

/// Espera o mock ter `expected` conexões WAMP abertas.
async fn until_wamp_clients(mock: &MockLcu, expected: usize, why: &str) {
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        while mock.wamp_clients() != expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "{why}: esperava {expected} conexões WAMP, o mock tem {}",
        mock.wamp_clients()
    );
}

/// Dá tempo para várias tentativas de reconexão (o intervalo máximo é 50 ms)
/// e confere que o número de conexões WAMP não mudou.
async fn stays_at(mock: &MockLcu, expected: usize, why: &str) {
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(mock.wamp_clients(), expected, "{why}");
}

#[tokio::test]
async fn cancelar_o_servico_fecha_a_conexao_com_o_lol() {
    let dir = temp_dir("fecha");
    let mock = start_mock(&dir).await;

    let service = start_service(&dir).await;
    until_wamp_clients(&mock, 1, "o serviço devia conectar").await;

    cancel(service).await;
    until_wamp_clients(&mock, 0, "a conexão do serviço cancelado ficou aberta").await;

    // O LoL reabre: o serviço cancelado não pode voltar a conectar.
    mock.shutdown().await;
    let mock = start_mock(&dir).await;
    stays_at(&mock, 0, "o serviço cancelado reconectou no LoL novo").await;
    assert!(mock.ux_open(), "o serviço cancelado fechou a UI oficial");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn reiniciar_o_servico_nao_acumula_conexoes() {
    let dir = temp_dir("reinicia");
    let mock = start_mock(&dir).await;

    let mut service = start_service(&dir).await;
    until_wamp_clients(&mock, 1, "o serviço devia conectar").await;

    for round in 1..=3 {
        cancel(service).await;
        service = start_service(&dir).await;
        until_wamp_clients(&mock, 1, &format!("reinício {round}")).await;
        stays_at(
            &mock,
            1,
            &format!("reinício {round}: sobrou conexão antiga"),
        )
        .await;
    }

    // O LoL reabre: só o serviço atual reconecta.
    mock.shutdown().await;
    let mock = start_mock(&dir).await;
    until_wamp_clients(&mock, 1, "o serviço atual devia reconectar").await;
    stays_at(&mock, 1, "um serviço cancelado reconectou no LoL novo").await;

    cancel(service).await;
    until_wamp_clients(&mock, 0, "a conexão do último serviço ficou aberta").await;

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
