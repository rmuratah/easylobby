//! kill-ux automático (#12) contra o mock-lcu: ao conectar, o agente fecha a
//! UI oficial, de novo quando o LoL reabre, e deixa a UI aberta quando a opção
//! está desligada.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{
    ConnectionEvent, CredentialSource, LcuConnection, LcuSession, ReconnectConfig,
};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ux::{self, KillUx};
use easylobby_agent::wamp::EventFilter;
use mock_lcu::{Config, MockLcu, ReplayStart};

const FIXTURES: &str = "16.22";

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent/tests/fixtures")
        .join(FIXTURES);
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mock-lcu-kill-ux-{name}-{}", std::process::id()))
}

/// Sobe o mock com lockfile e CA em `dir`. A UI oficial começa aberta.
async fn start_mock(dir: &Path) -> MockLcu {
    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

fn connection(dir: &Path, kill_ux: bool) -> LcuConnection {
    LcuConnection::new(ReconnectConfig {
        filter: EventFilter::prefixes(["/lol-gameflow/v1/gameflow-phase"]),
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        kill_ux,
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    })
}

async fn next(conn: &mut LcuConnection) -> ConnectionEvent {
    tokio::time::timeout(Duration::from_secs(20), conn.next())
        .await
        .expect("a conexão não andou a tempo")
}

async fn until_connected(conn: &mut LcuConnection) -> LcuSession {
    loop {
        match next(conn).await {
            ConnectionEvent::Connected(session) => return session,
            ConnectionEvent::Retrying { .. } => {}
            other => panic!("esperava Connected ou Retrying: {other:?}"),
        }
    }
}

#[tokio::test]
async fn fecha_a_ui_ao_conectar_e_reabre_pelo_launch_ux() {
    let dir = temp_dir("conectar");
    let _ = std::fs::remove_dir_all(&dir);
    let mock = start_mock(&dir).await;
    assert!(mock.ux_open());

    let mut conn = connection(&dir, true);
    let session = until_connected(&mut conn).await;
    assert!(session.kill_ux.is_killed(), "{:?}", session.kill_ux);
    assert!(!mock.ux_open());
    // A LCU continua respondendo sem a UI oficial.
    let phase: String = session
        .client
        .get("/lol-gameflow/v1/gameflow-phase")
        .await
        .unwrap();
    assert!(!phase.is_empty());

    // Botão de emergência.
    ux::launch_ux(&session.client).await.unwrap();
    assert!(mock.ux_open());
    // kill-ux com a UI já fechada não dá erro.
    ux::kill_ux(&session.client).await.unwrap();
    ux::kill_ux(&session.client).await.unwrap();
    assert!(!mock.ux_open());

    conn.close().await;
    mock.shutdown().await;
}

#[tokio::test]
async fn desligado_mantem_a_ui_aberta() {
    let dir = temp_dir("desligado");
    let _ = std::fs::remove_dir_all(&dir);
    let mock = start_mock(&dir).await;

    let mut conn = connection(&dir, false);
    let session = until_connected(&mut conn).await;
    assert!(matches!(session.kill_ux, KillUx::Disabled));
    assert!(mock.ux_open());

    conn.close().await;
    mock.shutdown().await;
}

#[tokio::test]
async fn fecha_de_novo_quando_o_lol_reabre() {
    let dir = temp_dir("reabre");
    let _ = std::fs::remove_dir_all(&dir);
    let mut conn = connection(&dir, true);

    let first = start_mock(&dir).await;
    let session = until_connected(&mut conn).await;
    assert_eq!(session.id, 1);
    assert!(session.kill_ux.is_killed());
    assert!(!first.ux_open());

    // O LoL fecha e reabre com a UI oficial aberta.
    first.shutdown().await;
    match next(&mut conn).await {
        ConnectionEvent::Disconnected(_) => {}
        other => panic!("esperava Disconnected: {other:?}"),
    }
    let second = start_mock(&dir).await;
    assert!(second.ux_open());
    let session = until_connected(&mut conn).await;
    assert_eq!(session.id, 2);
    assert!(session.kill_ux.is_killed());
    assert!(!second.ux_open());

    conn.close().await;
    second.shutdown().await;
}

#[tokio::test]
async fn desligar_vale_na_proxima_conexao() {
    let dir = temp_dir("alternar");
    let _ = std::fs::remove_dir_all(&dir);
    let mut conn = connection(&dir, true);

    let first = start_mock(&dir).await;
    until_connected(&mut conn).await;
    assert!(!first.ux_open());

    // Desligar não reabre a UI da conexão atual.
    conn.set_kill_ux(false);
    assert!(!first.ux_open());

    first.shutdown().await;
    match next(&mut conn).await {
        ConnectionEvent::Disconnected(_) => {}
        other => panic!("esperava Disconnected: {other:?}"),
    }
    let second = start_mock(&dir).await;
    let session = until_connected(&mut conn).await;
    assert!(matches!(session.kill_ux, KillUx::Disabled));
    assert!(second.ux_open());

    conn.close().await;
    second.shutdown().await;
}
