//! Canal `/ws` do agente (#15) contra o mock-lcu com a gravação 16.22: a UI
//! conecta, recebe o estado, acompanha uma partida inteira pelas mudanças,
//! manda comandos e vê o LoL fechar.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::launcher::LaunchOptions;
use easylobby_agent::ws::protocol::{
    ClientMessage, Command, CommandResult, ErrorCode, LcuStatus, ServerMessage, PROTOCOL_VERSION,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-canal-ws-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path, replay_start: ReplayStart) -> MockLcu {
    let mut config = Config::new(fixtures());
    config.speed = 500.0;
    config.replay_start = replay_start;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

/// Sobe o agente (conexão com o LoL + `/ws`) numa porta livre.
async fn start_agent(dir: &Path) -> String {
    start_agent_with(dir, WsConfig::default()).await
}

async fn start_agent_with(dir: &Path, config: WsConfig) -> String {
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}{}", listener.local_addr().unwrap(), ws::WS_PATH);
    tokio::spawn(ws::run(lcu, listener, config));
    url
}

async fn connect(url: &str) -> Ui {
    tokio_tungstenite::connect_async(url).await.unwrap().0
}

/// Próxima mensagem do agente, com tempo máximo.
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

/// Lê mensagens até `done` aceitar uma, devolvendo todas as lidas.
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

fn phases(messages: &[ServerMessage]) -> Vec<Option<String>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::GameflowPhase { phase } => Some(phase.clone()),
            _ => None,
        })
        .collect()
}

fn is_phase(message: &ServerMessage, expected: &str) -> bool {
    matches!(message, ServerMessage::GameflowPhase { phase: Some(p) } if p == expected)
}

#[tokio::test]
async fn acompanha_uma_partida_e_o_lol_fechando() {
    let dir = temp_dir("partida");
    let url = start_agent(&dir).await;

    // Antes do LoL abrir: hello e depois "não está aberto".
    let mut ui = connect(&url).await;
    match recv(&mut ui).await {
        ServerMessage::Hello {
            protocol, state, ..
        } => {
            assert_eq!(protocol, PROTOCOL_VERSION);
            assert_eq!(state.summoner, None);
            assert_eq!(state.gameflow_phase, None);
        }
        other => panic!("esperava hello: {other:?}"),
    }
    until(&mut ui, |m| {
        matches!(
            m,
            ServerMessage::Lcu {
                status: LcuStatus::Retrying {
                    client_not_running: true,
                    ..
                }
            }
        )
    })
    .await;

    // O LoL abre: conectado, invocador e fase relidos pelo REST, e a partida
    // inteira pelos eventos.
    let mock = start_mock(&dir, ReplayStart::OnFirstSubscribe).await;
    let seen = until(&mut ui, |m| is_phase(m, "InProgress")).await;
    assert!(seen.iter().any(|m| matches!(
        m,
        ServerMessage::Lcu {
            status: LcuStatus::Connected { session: 1 }
        }
    )));
    let summoner = seen
        .iter()
        .find_map(|m| match m {
            ServerMessage::Summoner { summoner } => summoner.clone(),
            _ => None,
        })
        .expect("o invocador não chegou");
    assert!(!summoner.puuid.is_empty());
    assert!(summoner.summoner_level > 0);

    let phases = phases(&seen);
    let partida = [
        "Lobby",
        "Matchmaking",
        "ReadyCheck",
        "ChampSelect",
        "GameStart",
        "InProgress",
    ];
    let tail: Vec<&str> = phases[phases.len() - partida.len()..]
        .iter()
        .map(|p| p.as_deref().unwrap())
        .collect();
    assert_eq!(tail, partida, "fases: {phases:?}");

    // Uma UI que abre agora recebe tudo no hello.
    let mut late = connect(&url).await;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            assert_eq!(state.lcu, LcuStatus::Connected { session: 1 });
            assert_eq!(state.summoner.as_ref(), Some(&summoner));
            assert_eq!(state.gameflow_phase.as_deref(), Some("InProgress"));
        }
        other => panic!("esperava hello: {other:?}"),
    }

    // Comandos. Eventos atrasados da gravação (como o lobby sendo apagado)
    // podem chegar antes de cada resposta.
    send(&mut late, 1, Command::Ping).await;
    let seen = until(&mut late, |m| matches!(m, ServerMessage::Response { .. })).await;
    assert_eq!(
        seen.last(),
        Some(&ServerMessage::Response {
            id: 1,
            result: CommandResult::Ok
        })
    );
    send(&mut late, 2, Command::GetState).await;
    let seen = until(&mut late, |m| matches!(m, ServerMessage::Response { .. })).await;
    assert!(seen
        .iter()
        .any(|m| matches!(m, ServerMessage::Hello { .. })));
    assert_eq!(
        seen.last(),
        Some(&ServerMessage::Response {
            id: 2,
            result: CommandResult::Ok
        })
    );
    late.send(Message::text(
        "{\"id\": 3, \"command\": {\"type\": \"comprarSkin\"}}",
    ))
    .await
    .unwrap();
    until(&mut late, |m| {
        matches!(m, ServerMessage::InvalidMessage { .. })
    })
    .await;

    // O LoL fecha: o estado que dependia dele some e o agente continua de pé.
    mock.shutdown().await;
    let seen = until(&mut ui, |m| {
        matches!(
            m,
            ServerMessage::Lcu {
                status: LcuStatus::Retrying { .. }
            }
        )
    })
    .await;
    assert!(seen.contains(&ServerMessage::Summoner { summoner: None }));
    assert!(seen.contains(&ServerMessage::GameflowPhase { phase: None }));
    assert!(seen.iter().any(|m| matches!(
        m,
        ServerMessage::Lcu {
            status: LcuStatus::Disconnected { .. }
        }
    )));

    // E volta quando o LoL reabre.
    let mock = start_mock(&dir, ReplayStart::Never).await;
    until(&mut ui, |m| {
        matches!(
            m,
            ServerMessage::Lcu {
                status: LcuStatus::Connected { session: 2 }
            }
        )
    })
    .await;
    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn recusa_origem_de_fora() {
    let dir = temp_dir("origem");
    let url = start_agent(&dir).await;

    let mut request = url.as_str().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", HeaderValue::from_static("https://exemplo.com"));
    match tokio_tungstenite::connect_async(request).await {
        Err(tungstenite::Error::Http(response)) => assert_eq!(response.status(), 403),
        other => panic!("esperava 403: {other:?}"),
    }

    let mut request = url.as_str().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", HeaderValue::from_static("http://localhost:5173"));
    let (mut ui, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    assert!(matches!(recv(&mut ui).await, ServerMessage::Hello { .. }));
}

/// Resposta ao comando com este `id`, pulando as outras mensagens.
async fn response(ui: &mut Ui, id: u32) -> CommandResult {
    let seen = until(
        ui,
        |m| matches!(m, ServerMessage::Response { id: i, .. } if *i == id),
    )
    .await;
    match seen.into_iter().last() {
        Some(ServerMessage::Response { result, .. }) => result,
        other => panic!("esperava response: {other:?}"),
    }
}

#[tokio::test]
async fn launch_league_pede_o_lol_ao_riot_client() {
    // O LoL está fechado; um segundo mock faz o papel do Riot Client aberto.
    let dir = temp_dir("abrir-lol");
    let rc_dir = temp_dir("abrir-lol-rc");
    let riot_client = start_mock(&rc_dir, ReplayStart::Never).await;
    let config = WsConfig {
        launch: LaunchOptions {
            riot_client_lockfile: Some(rc_dir.join("lockfile")),
            ..LaunchOptions::default()
        },
        ..WsConfig::default()
    };
    let url = start_agent_with(&dir, config).await;

    let mut ui = connect(&url).await;
    send(&mut ui, 1, Command::LaunchLeague).await;
    assert_eq!(response(&mut ui, 1).await, CommandResult::Ok);
    assert_eq!(riot_client.launches(), ["league_of_legends/live"]);

    riot_client.shutdown().await;
    let _ = std::fs::remove_dir_all(&rc_dir);
}

#[tokio::test]
async fn launch_league_sem_riot_client_da_launch_failed() {
    let dir = temp_dir("abrir-lol-falha");
    let nao_existe = dir.join("nao-existe");
    let config = WsConfig {
        launch: LaunchOptions {
            riot_client: Some(nao_existe.join("RiotClientServices.exe")),
            riot_client_lockfile: Some(nao_existe.join("lockfile")),
            ..LaunchOptions::default()
        },
        ..WsConfig::default()
    };
    let url = start_agent_with(&dir, config).await;

    let mut ui = connect(&url).await;
    send(&mut ui, 2, Command::LaunchLeague).await;
    match response(&mut ui, 2).await {
        CommandResult::Error { code, message } => {
            assert_eq!(code, ErrorCode::LaunchFailed);
            assert!(message.contains("Riot Client"), "{message}");
        }
        other => panic!("esperava erro: {other:?}"),
    }
}

/// Botão de emergência (#13): o agente fecha a UI oficial ao conectar e o
/// comando `launchUx` reabre.
#[tokio::test]
async fn launch_ux_reabre_a_ui_oficial() {
    let dir = temp_dir("launch-ux");
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    assert!(matches!(recv(&mut ui).await, ServerMessage::Hello { .. }));

    // Sem o LoL aberto, não tem o que reabrir.
    send(&mut ui, 1, Command::LaunchUx).await;
    let seen = until(&mut ui, |m| {
        matches!(m, ServerMessage::Response { id: 1, .. })
    })
    .await;
    assert!(matches!(
        seen.last().unwrap(),
        ServerMessage::Response {
            result: CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            },
            ..
        }
    ));

    let mock = start_mock(&dir, ReplayStart::Never).await;
    until(&mut ui, |m| {
        matches!(
            m,
            ServerMessage::Lcu {
                status: LcuStatus::Connected { .. }
            }
        )
    })
    .await;
    // O kill-ux sai antes de a conexão ser anunciada.
    assert!(!mock.ux_open());

    send(&mut ui, 2, Command::LaunchUx).await;
    let seen = until(&mut ui, |m| {
        matches!(m, ServerMessage::Response { id: 2, .. })
    })
    .await;
    assert_eq!(
        seen.last().unwrap(),
        &ServerMessage::Response {
            id: 2,
            result: CommandResult::Ok
        }
    );
    assert!(mock.ux_open());

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
