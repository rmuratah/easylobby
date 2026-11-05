//! Fase do gameflow (#16) contra o mock-lcu com a gravação 16.22: o agente lê
//! a fase pelo REST, segue cada mudança pelo WAMP como [`GameflowPhase`] e,
//! no fim, o REST e os eventos concordam.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{
    ConnectionEvent, CredentialSource, LcuConnection, ReconnectConfig,
};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::gameflow::{self, GameflowPhase, GAMEFLOW_PHASE_PATH};
use easylobby_agent::ws::Hub;
use mock_lcu::{Config, MockLcu, ReplayStart};

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-gameflow-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let mut config = Config::new(fixtures());
    config.speed = 500.0;
    config.replay_start = ReplayStart::OnFirstSubscribe;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

async fn next(conn: &mut LcuConnection) -> ConnectionEvent {
    tokio::time::timeout(Duration::from_secs(20), conn.next())
        .await
        .expect("a conexão não andou a tempo")
}

#[tokio::test]
async fn segue_a_fase_de_uma_ranqueada() {
    let dir = temp_dir("ranqueada");
    let mock = start_mock(&dir).await;
    let mut conn = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });

    // O Hub faz o que o agente faz de verdade: lê a fase pelo REST ao
    // conectar e depois segue os eventos.
    let hub = Hub::new();
    let (_, mut rx) = hub.subscribe();
    let client = loop {
        match next(&mut conn).await {
            ConnectionEvent::Connected(session) => {
                let client = session.client.clone();
                hub.apply(ConnectionEvent::Connected(session)).await;
                break client;
            }
            other => hub.apply(other).await,
        }
    };

    let mut phases = Vec::new();
    while hub.state().gameflow_phase != Some(GameflowPhase::InProgress) {
        hub.apply(next(&mut conn).await).await;
        while let Ok(message) = rx.try_recv() {
            if let easylobby_agent::ws::protocol::ServerMessage::GameflowPhase {
                phase: Some(phase),
            } = message
            {
                phases.push(phase);
            }
        }
    }

    // A reprodução começa quando o agente se inscreve no WAMP, então a fase
    // lida pelo REST na conexão pode já estar adiantada em relação aos
    // eventos que ainda estão na fila. O que importa: a partida inteira, na
    // ordem, no fim, e nenhuma fase desconhecida.
    let partida = [
        GameflowPhase::Lobby,
        GameflowPhase::Matchmaking,
        GameflowPhase::ReadyCheck,
        GameflowPhase::ChampSelect,
        GameflowPhase::GameStart,
        GameflowPhase::InProgress,
    ];
    assert!(phases.len() >= partida.len(), "fases: {phases:?}");
    assert_eq!(
        phases[phases.len() - partida.len()..],
        partida,
        "fases: {phases:?}"
    );
    assert!(
        !phases.contains(&GameflowPhase::Unknown),
        "fases: {phases:?}"
    );

    // O REST conta a mesma fase que os eventos.
    assert_eq!(
        gameflow::current_phase(&client).await.unwrap(),
        GameflowPhase::InProgress
    );
    assert_eq!(mock.phase(), "InProgress");
    let raw: String = client.get(GAMEFLOW_PHASE_PATH).await.unwrap();
    assert_eq!(raw, "InProgress");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
