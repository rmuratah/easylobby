//! Partida encontrada (#20) contra o mock-lcu com a gravação 16.22: a UI
//! acompanha o ready check pelo `/ws` e aceita ou recusa com os comandos
//! `acceptReadyCheck` e `declineReadyCheck`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ready_check::{ReadyCheck, ReadyCheckResponse, ReadyCheckState};
use easylobby_agent::ws::protocol::{
    ClientMessage, Command, CommandResult, ErrorCode, LcuStatus, ServerMessage,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

const READY_CHECK: &str = "/lol-matchmaking/v1/ready-check";

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

/// A gravação cortada logo depois de o ready check abrir: o mock fica parado
/// com a partida encontrada, esperando a resposta do jogador.
fn fixtures_until_ready_check() -> FixtureSet {
    let mut set = fixtures();
    let opened = set
        .events
        .iter()
        .position(|e| e.uri == READY_CHECK && e.data["state"] == "InProgress")
        .expect("a gravação 16.22 tem um ready check");
    set.events.truncate(opened + 1);
    set
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-ready-check-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path, fixtures: FixtureSet, replay_start: ReplayStart) -> MockLcu {
    let mut config = Config::new(fixtures);
    config.speed = 500.0;
    config.replay_start = replay_start;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

/// Sobe o agente (conexão com o LoL + `/ws`) numa porta livre.
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

async fn connect(url: &str) -> Ui {
    tokio_tungstenite::connect_async(url).await.unwrap().0
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

async fn send(ui: &mut Ui, id: u32, command: Command) {
    let json = serde_json::to_string(&ClientMessage { id, command }).unwrap();
    ui.send(Message::text(json)).await.unwrap();
}

async fn until(ui: &mut Ui, done: impl Fn(&ServerMessage) -> bool) -> Vec<ServerMessage> {
    let mut seen = Vec::new();
    loop {
        let message = recv(ui).await;
        let stop = done(&message);
        seen.push(message);
        if stop {
            return seen;
        }
    }
}

fn ready_checks(messages: &[ServerMessage]) -> Vec<Option<ReadyCheck>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::ReadyCheck { ready_check } => Some(ready_check.clone()),
            _ => None,
        })
        .collect()
}

fn is_response(message: &ServerMessage, expected: u32) -> bool {
    matches!(message, ServerMessage::Response { id, .. } if *id == expected)
}

fn response_result(messages: &[ServerMessage]) -> CommandResult {
    match messages.last() {
        Some(ServerMessage::Response { result, .. }) => result.clone(),
        other => panic!("esperava uma resposta: {other:?}"),
    }
}

/// Espera a resposta do comando `id` e o ready check que o evento da LCU
/// trouxe depois dele, em qualquer ordem.
async fn answer(ui: &mut Ui, id: u32) -> (CommandResult, ReadyCheck) {
    let mut result = None;
    let mut check = None;
    while result.is_none() || check.is_none() {
        match recv(ui).await {
            ServerMessage::Response { id: got, result: r } if got == id => result = Some(r),
            ServerMessage::ReadyCheck {
                ready_check: Some(c),
            } => check = Some(c),
            _ => {}
        }
    }
    (result.unwrap(), check.unwrap())
}

#[tokio::test]
async fn acompanha_o_ready_check_da_gravacao() {
    let dir = temp_dir("gravacao");
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    let mock = start_mock(&dir, fixtures(), ReplayStart::OnFirstSubscribe).await;

    let seen = until(&mut ui, |m| {
        matches!(m, ServerMessage::GameflowPhase { phase: Some(p) } if p.as_str() == "ChampSelect")
    })
    .await;
    let checks = ready_checks(&seen);

    // Abre sem resposta, o jogador aceita, todos aceitam e o ready check
    // fecha antes da seleção de campeões. O `Invalid` da fila não aparece.
    let first = checks
        .first()
        .and_then(Option::as_ref)
        .expect("o ready check não abriu");
    assert_eq!(first.state, ReadyCheckState::InProgress);
    assert_eq!(first.player_response, ReadyCheckResponse::Pending);
    assert!(first.duration > 0.0);
    assert!(checks
        .iter()
        .flatten()
        .any(|c| c.state == ReadyCheckState::InProgress
            && c.player_response == ReadyCheckResponse::Accepted
            && c.timer > 0.0));
    let last_open = checks.iter().rev().flatten().next().unwrap();
    assert_eq!(last_open.state, ReadyCheckState::EveryoneReady);
    assert_eq!(checks.last(), Some(&None), "{checks:?}");
    assert!(checks
        .iter()
        .flatten()
        .all(|c| c.state != ReadyCheckState::Invalid && c.state != ReadyCheckState::Unknown));

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn aceita_e_recusa_pelo_ws() {
    let dir = temp_dir("aceitar");
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    let mock = start_mock(
        &dir,
        fixtures_until_ready_check(),
        ReplayStart::OnFirstSubscribe,
    )
    .await;

    // Partida encontrada, esperando a resposta.
    until(&mut ui, |m| {
        matches!(
            m,
            ServerMessage::ReadyCheck {
                ready_check: Some(ReadyCheck {
                    state: ReadyCheckState::InProgress,
                    player_response: ReadyCheckResponse::Pending,
                    ..
                })
            }
        )
    })
    .await;

    // Aceitar: ok, e a mudança chega pelo evento da LCU.
    send(&mut ui, 1, Command::AcceptReadyCheck).await;
    let (result, check) = answer(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(check.player_response, ReadyCheckResponse::Accepted);
    assert_eq!(check.state, ReadyCheckState::InProgress);

    // Recusar depois de aceitar também vale, como no client oficial.
    send(&mut ui, 2, Command::DeclineReadyCheck).await;
    let (result, check) = answer(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(check.player_response, ReadyCheckResponse::Declined);

    // Uma UI que abre agora recebe o ready check no hello.
    let mut late = connect(&url).await;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            let check = state.ready_check.expect("o ready check sumiu do estado");
            assert_eq!(check.player_response, ReadyCheckResponse::Declined);
        }
        other => panic!("esperava hello: {other:?}"),
    }

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sem_partida_encontrada_aceitar_da_erro() {
    let dir = temp_dir("sem-partida");
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;

    // LoL fechado: notConnected.
    send(&mut ui, 1, Command::AcceptReadyCheck).await;
    let seen = until(&mut ui, |m| is_response(m, 1)).await;
    assert!(matches!(
        response_result(&seen),
        CommandResult::Error {
            code: ErrorCode::NotConnected,
            ..
        }
    ));

    // LoL aberto fora da fila: a LCU recusa e o agente responde `lcu`.
    let mock = start_mock(&dir, fixtures(), ReplayStart::Never).await;
    until(&mut ui, |m| {
        matches!(
            m,
            ServerMessage::Lcu {
                status: LcuStatus::Connected { .. }
            }
        )
    })
    .await;
    for (id, command) in [
        (2, Command::AcceptReadyCheck),
        (3, Command::DeclineReadyCheck),
    ] {
        send(&mut ui, id, command).await;
        let seen = until(&mut ui, |m| is_response(m, id)).await;
        assert!(
            matches!(
                response_result(&seen),
                CommandResult::Error {
                    code: ErrorCode::Lcu,
                    ..
                }
            ),
            "{seen:?}"
        );
    }

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
