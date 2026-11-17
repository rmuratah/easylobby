//! A LCU que não responde a tempo ao conectar (#580).
//!
//! Com o LoL subindo ou esperando os servidores da Riot, a fase e as filas
//! às vezes ficam sem resposta na releitura do `Connected`. Antes o estado
//! ficava vazio até um evento da LCU, e num lobby parado o evento não vinha:
//! a UI ficava sem fase e sem filas, sem ter como iniciar a fila. O agente
//! agora lê de novo o que ficou sem resposta.

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::http::Method;
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::gameflow::{GameflowPhase, GAMEFLOW_PHASE_PATH};
use easylobby_agent::queues::GAME_QUEUES_PATH;
use easylobby_agent::ws::protocol::ServerMessage;
use easylobby_agent::ws::{self, WsConfig};
use futures_util::StreamExt;
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-sem-resposta-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[tokio::test]
async fn le_de_novo_a_fase_e_as_filas_que_ficaram_sem_resposta() {
    let dir = temp_dir("fase-filas");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    // Sem reprodução: nenhum evento da fase chega para corrigir o estado.
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();

    // A primeira leitura de cada uma fica sem resposta: o agente desiste
    // pelo tempo máximo. As seguintes respondem normalmente.
    let _phase = mock.hold(Method::GET, GAMEFLOW_PHASE_PATH);
    let _queues = mock.hold(Method::GET, GAME_QUEUES_PATH);

    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        request_timeout: Duration::from_secs(2),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}{}", listener.local_addr().unwrap(), ws::WS_PATH);
    let config = WsConfig {
        compatibility_check: false,
        ..WsConfig::default()
    };
    tokio::spawn(ws::run(lcu, listener, config));
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    let mut phase = None;
    let mut queues = false;
    tokio::time::timeout(Duration::from_secs(60), async {
        while phase.is_none() || !queues {
            let Some(Ok(Message::Text(text))) = ui.next().await else {
                continue;
            };
            match serde_json::from_str::<ServerMessage>(&text).unwrap() {
                ServerMessage::Hello { state, .. } => {
                    phase = phase.or(state.gameflow_phase);
                    queues |= !state.queues.is_empty();
                }
                ServerMessage::GameflowPhase { phase: Some(read) } => phase = Some(read),
                ServerMessage::Queues { queues: list } => queues |= !list.is_empty(),
                _ => {}
            }
        }
    })
    .await
    .expect("o agente não leu de novo a fase e as filas");
    assert_eq!(phase, Some(GameflowPhase::Idle));

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
