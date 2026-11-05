//! Reconexão do agente (#10) contra o mock-lcu: o agente abre antes do LoL,
//! o LoL fecha e reabre com outra porta, outra senha e outra CA, e o agente
//! volta a receber eventos sem cair.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{
    ConnectionEvent, CredentialSource, DisconnectReason, LcuConnection, LcuSession, ReconnectConfig,
};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::wamp::{EventFilter, LcuEvent};
use mock_lcu::{Config, MockLcu, ReplayStart};
use serde_json::Value;

const FIXTURES: &str = "16.22";

const PHASE: &str = "/lol-gameflow/v1/gameflow-phase";

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent/tests/fixtures")
        .join(FIXTURES);
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mock-lcu-reconexao-{name}-{}", std::process::id()))
}

/// Sobe o mock gravando lockfile e CA em `dir`. Cada chamada tem porta,
/// senha e CA novas, como um LoL que reabriu.
async fn start_mock(dir: &Path, replay_start: ReplayStart) -> MockLcu {
    let mut config = Config::new(fixtures());
    config.speed = 2000.0;
    config.replay_start = replay_start;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

fn connection(dir: &Path) -> LcuConnection {
    LcuConnection::new(ReconnectConfig {
        filter: EventFilter::prefixes([PHASE]),
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    })
}

/// Próximo evento da conexão, com tempo máximo.
async fn next(conn: &mut LcuConnection) -> ConnectionEvent {
    tokio::time::timeout(Duration::from_secs(20), conn.next())
        .await
        .expect("a conexão não andou a tempo")
}

/// Espera conectar, contando as tentativas que falharam.
async fn until_connected(conn: &mut LcuConnection) -> (LcuSession, u32) {
    let mut failures = 0;
    loop {
        match next(conn).await {
            ConnectionEvent::Connected(session) => return (session, failures),
            ConnectionEvent::Retrying { attempt, .. } => {
                failures += 1;
                assert_eq!(attempt, failures);
            }
            other => panic!("esperava Connected ou Retrying: {other:?}"),
        }
    }
}

/// Lê eventos até chegar a fase `phase`.
async fn until_phase(conn: &mut LcuConnection, phase: &str) -> Vec<LcuEvent> {
    let mut seen = Vec::new();
    loop {
        match next(conn).await {
            ConnectionEvent::Event(event) => {
                let done = event.data == phase;
                seen.push(event);
                if done {
                    return seen;
                }
            }
            other => panic!("esperava eventos até {phase}: {other:?}"),
        }
    }
}

/// Espera a queda, ignorando eventos que ainda estavam a caminho.
async fn until_disconnected(conn: &mut LcuConnection) -> DisconnectReason {
    loop {
        match next(conn).await {
            ConnectionEvent::Event(_) => {}
            ConnectionEvent::Disconnected(reason) => return reason,
            other => panic!("esperava Disconnected: {other:?}"),
        }
    }
}

#[tokio::test]
async fn espera_o_lol_abrir() {
    let dir = temp_dir("espera");
    let _ = std::fs::remove_dir_all(&dir);
    let mut conn = connection(&dir);

    // Sem LoL: tenta de novo, com espera crescente, e diz que está fechado.
    let mut delays = Vec::new();
    for _ in 0..3 {
        match next(&mut conn).await {
            ConnectionEvent::Retrying { delay, error, .. } => {
                assert!(error.is_client_not_running(), "{error}");
                delays.push(delay);
            }
            other => panic!("esperava Retrying: {other:?}"),
        }
    }
    assert!(delays.windows(2).all(|w| w[0] <= w[1]), "{delays:?}");
    assert!(!conn.is_connected());

    let mock = start_mock(&dir, ReplayStart::OnFirstSubscribe).await;
    let (session, _) = until_connected(&mut conn).await;
    assert_eq!(session.id, 1);
    assert_eq!(session.credentials.port, mock.port());
    assert!(conn.is_connected());

    // REST e eventos da sessão funcionam.
    let phase: String = session.client.get(PHASE).await.unwrap();
    assert!(!phase.is_empty());
    let seen = until_phase(&mut conn, "InProgress").await;
    assert!(seen.iter().all(|e| e.uri == PHASE));

    conn.close().await;
    mock.shutdown().await;
}

#[tokio::test]
async fn reconecta_quando_o_lol_reinicia() {
    let dir = temp_dir("reinicia");
    let _ = std::fs::remove_dir_all(&dir);
    let mut conn = connection(&dir);

    let first = start_mock(&dir, ReplayStart::OnFirstSubscribe).await;
    let (session, failures) = until_connected(&mut conn).await;
    assert_eq!(failures, 0);
    assert_eq!(session.id, 1);
    let old = session.credentials.clone();
    until_phase(&mut conn, "ChampSelect").await;

    // O LoL fecha no meio do champ select.
    first.shutdown().await;
    until_disconnected(&mut conn).await;
    assert!(!conn.is_connected());
    assert!(conn.session().is_none());
    // O cliente REST antigo não serve mais.
    assert!(session.client.get::<String>(PHASE).await.is_err());

    // Enquanto está fechado, o agente continua tentando sem cair.
    match next(&mut conn).await {
        ConnectionEvent::Retrying { error, .. } => {
            assert!(error.is_client_not_running(), "{error}")
        }
        other => panic!("esperava Retrying: {other:?}"),
    }

    // Reabre com outra porta, outra senha e outra CA.
    let second = start_mock(&dir, ReplayStart::OnFirstSubscribe).await;
    let (session, _) = until_connected(&mut conn).await;
    assert_eq!(session.id, 2);
    assert_eq!(session.credentials.port, second.port());
    assert_ne!(session.credentials.password, old.password);

    // A partida inteira chega de novo pela conexão nova.
    let seen = until_phase(&mut conn, "InProgress").await;
    let phases: Vec<&str> = seen.iter().filter_map(|e| e.data.as_str()).collect();
    for phase in ["Lobby", "Matchmaking", "ReadyCheck", "ChampSelect"] {
        assert!(phases.contains(&phase), "faltou {phase}: {phases:?}");
    }
    let phase: String = session.client.get(PHASE).await.unwrap();
    assert_eq!(phase, second.phase());

    conn.close().await;
    second.shutdown().await;
}

#[tokio::test]
async fn lockfile_velho_nao_derruba() {
    // O LoL travou e deixou o lockfile para trás, apontando para uma porta
    // fechada.
    let dir = temp_dir("velho");
    let _ = std::fs::remove_dir_all(&dir);
    let stale = start_mock(&dir, ReplayStart::Never).await;
    let content = std::fs::read_to_string(dir.join("lockfile")).unwrap();
    let ca = std::fs::read(dir.join("mock-lcu-ca.pem")).unwrap();
    stale.shutdown().await;
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("lockfile"), &content).unwrap();
    std::fs::write(dir.join("mock-lcu-ca.pem"), &ca).unwrap();

    let mut conn = connection(&dir);
    for _ in 0..2 {
        match next(&mut conn).await {
            ConnectionEvent::Retrying { error, .. } => {
                assert!(!error.is_client_not_running(), "{error}")
            }
            other => panic!("esperava Retrying: {other:?}"),
        }
    }

    // O LoL abre de novo e sobrescreve o lockfile.
    let mock = start_mock(&dir, ReplayStart::Never).await;
    let (session, _) = until_connected(&mut conn).await;
    assert_eq!(session.credentials.port, mock.port());

    conn.close().await;
    mock.shutdown().await;
}

#[tokio::test]
async fn next_pode_ser_cancelado_sem_perder_eventos() {
    let dir = temp_dir("cancelado");
    let _ = std::fs::remove_dir_all(&dir);
    let mock = start_mock(&dir, ReplayStart::OnFirstSubscribe).await;
    let mut conn = connection(&dir);
    until_connected(&mut conn).await;

    // Um select! que cancela o next() a todo momento, como o loop do agente
    // com outros canais.
    let want: Vec<Value> = fixtures()
        .events
        .into_iter()
        .filter(|e| e.uri == PHASE)
        .map(|e| e.data)
        .collect();
    let mut seen = Vec::new();
    let mut tick = tokio::time::interval(Duration::from_micros(200));
    tokio::time::timeout(Duration::from_secs(20), async {
        while seen.len() < want.len() {
            tokio::select! {
                event = conn.next() => match event {
                    ConnectionEvent::Event(event) => seen.push(event.data),
                    other => panic!("esperava eventos: {other:?}"),
                },
                _ = tick.tick() => {}
            }
        }
    })
    .await
    .expect("os eventos não chegaram a tempo");
    assert_eq!(seen, want);

    conn.close().await;
    mock.shutdown().await;
}
