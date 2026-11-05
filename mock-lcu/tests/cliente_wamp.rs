//! O cliente WAMP do agente (#9) contra o mock-lcu: lockfile, WebSocket com o
//! certificado fixado e Basic Auth, inscrição em `OnJsonApiEvent` e filtro dos
//! tópicos.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::lockfile::Credentials;
use easylobby_agent::rest::RIOT_ROOT_PEM;
use easylobby_agent::wamp::{EventFilter, EventType, LcuEvent, LcuEvents, WampError};
use mock_lcu::{Config, MockLcu, ReplayStart};
use serde_json::Value;

/// Senha de teste, obviamente falsa.
const PASSWORD: &str = "senha-falsa-do-cliente-wamp";

const FIXTURES: &str = "16.22";

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent/tests/fixtures")
        .join(FIXTURES);
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mock-lcu-wamp-{name}-{}", std::process::id()))
}

/// Sobe o mock com lockfile (e CA, com TLS) em disco e devolve as
/// credenciais lidas do lockfile, como o agente vai ler.
async fn start(
    name: &str,
    tls: bool,
    replay_start: ReplayStart,
) -> (MockLcu, Credentials, Vec<u8>) {
    let dir = temp_dir(name);
    let mut config = Config::new(fixtures());
    config.password = PASSWORD.into();
    config.tls = tls;
    // A gravação de ~4 minutos em ~0,1 s.
    config.speed = 2000.0;
    config.replay_start = replay_start;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();
    let creds = Credentials::from_lockfile_path(&dir.join("lockfile")).unwrap();
    let ca_pem = if tls {
        std::fs::read(dir.join("mock-lcu-ca.pem")).unwrap()
    } else {
        Vec::new()
    };
    (mock, creds, ca_pem)
}

/// Os eventos da gravação que passam no filtro, na ordem.
fn expected(filter: &EventFilter) -> Vec<LcuEvent> {
    fixtures()
        .events
        .into_iter()
        .filter(|e| filter.matches(&e.uri))
        .map(|e| LcuEvent {
            uri: e.uri,
            event_type: match e.event_type.as_str() {
                "Create" => EventType::Create,
                "Update" => EventType::Update,
                "Delete" => EventType::Delete,
                other => panic!("eventType inesperado na fixture: {other}"),
            },
            data: e.data,
        })
        .collect()
}

/// Lê `n` eventos, com tempo máximo.
async fn take(events: &mut LcuEvents, n: usize) -> Vec<LcuEvent> {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut seen = Vec::with_capacity(n);
        while seen.len() < n {
            match events.next().await {
                Some(event) => seen.push(event.unwrap()),
                None => panic!("a conexão fechou depois de {} eventos", seen.len()),
            }
        }
        seen
    })
    .await
    .expect("os eventos não chegaram a tempo")
}

#[tokio::test]
async fn recebe_os_eventos_do_easylobby_na_ordem() {
    let (mock, creds, ca_pem) = start("easylobby", true, ReplayStart::OnFirstSubscribe).await;
    let filter = EventFilter::easylobby();
    let want = expected(&filter);
    assert!(want.len() > 500, "{} eventos", want.len());

    let mut events = LcuEvents::with_root_certificate(&creds, &ca_pem, filter)
        .await
        .unwrap();
    assert_eq!(events.url(), format!("wss://127.0.0.1:{}/", mock.port()));
    let seen = take(&mut events, want.len()).await;
    assert_eq!(seen, want);

    // A partida inteira passou: fila, ready check e champ select.
    let phases: Vec<&str> = seen
        .iter()
        .filter(|e| e.uri == "/lol-gameflow/v1/gameflow-phase")
        .filter_map(|e| e.data.as_str())
        .collect();
    for phase in [
        "Lobby",
        "Matchmaking",
        "ReadyCheck",
        "ChampSelect",
        "InProgress",
    ] {
        assert!(phases.contains(&phase), "faltou {phase}: {phases:?}");
    }
    for uri in [
        "/lol-matchmaking/v1/ready-check",
        "/lol-champ-select/v1/session",
        "/lol-lobby/v2/lobby",
    ] {
        assert!(seen.iter().any(|e| e.uri == uri), "faltou {uri}");
    }

    events.close().await;
    mock.shutdown().await;
}

#[tokio::test]
async fn filtra_so_os_topicos_pedidos() {
    let (mock, creds, ca_pem) = start("filtro", true, ReplayStart::OnFirstSubscribe).await;
    let filter = EventFilter::prefixes(["/lol-gameflow/v1/gameflow-phase"]);
    let want = expected(&filter);
    assert!(!want.is_empty());

    let mut events = LcuEvents::with_root_certificate(&creds, &ca_pem, filter)
        .await
        .unwrap();
    let seen = take(&mut events, want.len()).await;
    assert_eq!(seen, want);
    assert!(seen
        .iter()
        .all(|e| e.uri == "/lol-gameflow/v1/gameflow-phase"));
    assert_eq!(
        seen.last().map(|e| e.data.clone()),
        Some(Value::from("InProgress"))
    );

    events.close().await;
    mock.shutdown().await;
}

#[tokio::test]
async fn conecta_sem_tls_quando_o_lockfile_diz_http() {
    let (mock, creds, _) = start("http", false, ReplayStart::OnFirstSubscribe).await;
    let filter = EventFilter::prefixes(["/lol-gameflow/v1/gameflow-phase"]);
    let want = expected(&filter);
    let mut events = LcuEvents::connect(&creds, filter).await.unwrap();
    assert_eq!(events.url(), format!("ws://127.0.0.1:{}/", mock.port()));
    assert_eq!(take(&mut events, want.len()).await, want);
    events.close().await;
    mock.shutdown().await;
}

#[tokio::test]
async fn senha_errada_vira_unauthorized() {
    let (mock, mut creds, ca_pem) = start("401", true, ReplayStart::Never).await;
    creds.password = "outra-senha-falsa".into();
    let err = LcuEvents::with_root_certificate(&creds, &ca_pem, EventFilter::easylobby())
        .await
        .unwrap_err();
    assert!(matches!(err, WampError::Unauthorized), "{err}");
    mock.shutdown().await;
}

#[tokio::test]
async fn recusa_certificado_que_nao_bate_com_o_fixado() {
    // O mock assina com a CA dele; com o riotgames.pem fixado, o TLS falha.
    let (mock, creds, _) = start("cert", true, ReplayStart::Never).await;
    let err = LcuEvents::with_root_certificate(&creds, RIOT_ROOT_PEM, EventFilter::easylobby())
        .await
        .unwrap_err();
    assert!(err.is_connect(), "{err}");
    mock.shutdown().await;
}

#[tokio::test]
async fn lol_fechado_nao_conecta() {
    let (mock, creds, ca_pem) = start("fechado", true, ReplayStart::Never).await;
    mock.shutdown().await;
    let err = LcuEvents::with_root_certificate(&creds, &ca_pem, EventFilter::easylobby())
        .await
        .unwrap_err();
    assert!(err.is_connect(), "{err}");
}

#[tokio::test]
async fn next_devolve_none_quando_a_lcu_fecha() {
    let (mock, creds, ca_pem) = start("queda", true, ReplayStart::Never).await;
    let mut events = LcuEvents::with_root_certificate(&creds, &ca_pem, EventFilter::easylobby())
        .await
        .unwrap();
    mock.shutdown().await;

    let end = tokio::time::timeout(Duration::from_secs(10), async {
        let mut errors = 0;
        loop {
            match events.next().await {
                None => return errors,
                Some(Err(_)) => errors += 1,
                Some(Ok(event)) => panic!("evento inesperado: {event:?}"),
            }
        }
    })
    .await
    .expect("next() não percebeu a queda");
    // No máximo um erro antes do `None`.
    assert!(end <= 1, "{end} erros");
    // Depois do fim, continua `None`.
    assert!(events.next().await.is_none());
}
