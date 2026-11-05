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
use easylobby_agent::ws::protocol::ServerMessage;
use easylobby_agent::ws::Hub;
use mock_lcu::{Config, MockLcu, ReplayStart};
use serde_json::json;
use tokio::sync::broadcast;

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

/// Conecta o `Hub` no mock como o agente faz (fase lida pelo REST no
/// `Connected`) e devolve o cliente REST da conexão.
async fn connect(conn: &mut LcuConnection, hub: &Hub) -> easylobby_agent::rest::LcuClient {
    loop {
        match next(conn).await {
            ConnectionEvent::Connected(session) => {
                let client = session.client.clone();
                hub.apply(ConnectionEvent::Connected(session)).await;
                return client;
            }
            other => hub.apply(other).await,
        }
    }
}

/// Passa para o `Hub` os eventos da conexão até chegar o evento WAMP da fase
/// `InProgress`, e devolve as fases que o `Hub` publicou desde `rx`.
///
/// O fim depende do evento consumido, não do estado do `Hub`: a fase lida
/// pelo REST na conexão pode já ser `InProgress` (a reprodução começa na
/// inscrição WAMP) com os eventos de antes ainda na fila (#72).
async fn phases_until_in_progress(
    conn: &mut LcuConnection,
    hub: &Hub,
    rx: &mut broadcast::Receiver<ServerMessage>,
) -> Vec<GameflowPhase> {
    loop {
        let event = next(conn).await;
        let done = matches!(
            &event,
            ConnectionEvent::Event(e)
                if e.uri == GAMEFLOW_PHASE_PATH && e.data == json!("InProgress")
        );
        hub.apply(event).await;
        if done {
            break;
        }
    }
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|m| match m {
            ServerMessage::GameflowPhase { phase } => phase,
            _ => None,
        })
        .collect()
}

/// A partida inteira, na ordem, no fim das fases publicadas, e nenhuma fase
/// desconhecida. O começo depende de onde a reprodução estava quando o REST
/// respondeu.
fn assert_partida(phases: &[GameflowPhase]) {
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
}

fn connection(dir: &Path) -> LcuConnection {
    LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    })
}

#[tokio::test]
async fn segue_a_fase_de_uma_ranqueada() {
    let dir = temp_dir("ranqueada");
    let mock = start_mock(&dir).await;
    let mut conn = connection(&dir);
    let hub = Hub::new();
    let (_, mut rx) = hub.subscribe();
    let client = connect(&mut conn, &hub).await;

    let phases = phases_until_in_progress(&mut conn, &hub, &mut rx).await;
    assert_partida(&phases);

    // O REST conta a mesma fase que os eventos.
    assert_eq!(hub.state().gameflow_phase, Some(GameflowPhase::InProgress));
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

/// O pior caso da corrida (#72): o REST da conexão já respondeu `InProgress`
/// e os eventos da partida inteira ainda estão na fila. O `Hub` segue os
/// eventos mesmo assim e termina em `InProgress`.
#[tokio::test]
async fn rest_ja_em_in_progress_na_conexao() {
    let dir = temp_dir("rest-adiantado");
    let mock = start_mock(&dir).await;
    let mut conn = connection(&dir);
    let hub = Hub::new();
    connect(&mut conn, &hub).await;
    // Simula a leitura REST adiantada, sem depender de tempo.
    hub.set_gameflow_phase(Some(GameflowPhase::InProgress));
    let (_, mut rx) = hub.subscribe();

    let phases = phases_until_in_progress(&mut conn, &hub, &mut rx).await;
    assert_partida(&phases);
    assert_eq!(hub.state().gameflow_phase, Some(GameflowPhase::InProgress));

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
