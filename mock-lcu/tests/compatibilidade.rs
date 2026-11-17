//! Modo de compatibilidade (#36) contra o mock-lcu: com um endpoint que
//! "sumiu" e outro que "mudou de formato", o agente desliga só as funções
//! deles, avisa a UI e recusa os comandos delas sem falar com a LCU.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::health::{self, EndpointProblem, Feature, UnavailableFeature, PROBES};
use easylobby_agent::lobby::Queue;
use easylobby_agent::lockfile::Credentials;
use easylobby_agent::rest::LcuClient;
use easylobby_agent::ws::protocol::{
    ClientMessage, Command, CommandResult, ErrorCode, ServerMessage,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::broken::Broken;
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-compatibilidade-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path, broken: Broken) -> MockLcu {
    start_mock_with(dir, "16.22", broken).await
}

async fn start_mock_with(dir: &Path, set: &str, broken: Broken) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent/tests/fixtures")
        .join(set);
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    config.broken = broken;
    MockLcu::start(config).await.unwrap()
}

async fn start_agent(dir: &Path) -> Ui {
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}{}", listener.local_addr().unwrap(), ws::WS_PATH);
    tokio::spawn(ws::run(lcu, listener, WsConfig::default()));
    tokio_tungstenite::connect_async(url.as_str())
        .await
        .unwrap()
        .0
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

async fn command(ui: &mut Ui, id: u32, command: Command) -> CommandResult {
    let json = serde_json::to_string(&ClientMessage { id, command }).unwrap();
    ui.send(Message::text(json)).await.unwrap();
    loop {
        if let ServerMessage::Response { id: got, result } = recv(ui).await {
            if got == id {
                return result;
            }
        }
    }
}

async fn unavailable_features(ui: &mut Ui) -> Vec<UnavailableFeature> {
    loop {
        if let ServerMessage::UnavailableFeatures {
            unavailable_features,
        } = recv(ui).await
        {
            return unavailable_features;
        }
    }
}

#[tokio::test]
async fn desliga_so_as_funcoes_quebradas() {
    let dir = temp_dir("quebrado");
    let mock = start_mock(
        &dir,
        Broken {
            missing: vec!["/lol-lobby/v2/lobby".into()],
            bad_format: vec!["/lol-chat/v1/friends".into()],
        },
    )
    .await;
    let mut ui = start_agent(&dir).await;

    let unavailable = unavailable_features(&mut ui).await;
    let features: Vec<_> = unavailable.iter().map(|u| u.feature).collect();
    assert_eq!(features, [Feature::Lobby, Feature::Social]);
    assert_eq!(unavailable[0].path, "/lol-lobby/v2/lobby");
    assert_eq!(unavailable[0].problem, EndpointProblem::Missing);
    assert!(unavailable[0].detail.contains("Invalid URI format"));
    assert_eq!(unavailable[1].path, "/lol-chat/v1/friends");
    assert_eq!(unavailable[1].problem, EndpointProblem::UnexpectedFormat);

    // Os comandos do lobby são recusados sem chegar na LCU.
    match command(
        &mut ui,
        1,
        Command::CreateLobby {
            queue: Queue::NormalDraft,
        },
    )
    .await
    {
        CommandResult::Error { code, message } => {
            assert_eq!(code, ErrorCode::FeatureUnavailable);
            assert!(message.contains("client oficial"), "{message}");
        }
        other => panic!("esperava featureUnavailable: {other:?}"),
    }
    // As funções que funcionam continuam ligadas.
    assert_eq!(
        command(&mut ui, 2, Command::LaunchUx).await,
        CommandResult::Ok
    );
    assert!(!matches!(
        command(&mut ui, 3, Command::OpenMatchDetails { game_id: 1 }).await,
        CommandResult::Error {
            code: ErrorCode::FeatureUnavailable,
            ..
        }
    ));

    // Um `hello` novo traz as funções desligadas no estado.
    let json = serde_json::to_string(&ClientMessage {
        id: 4,
        command: Command::GetState,
    })
    .unwrap();
    ui.send(Message::text(json)).await.unwrap();
    loop {
        if let ServerMessage::Hello { state, .. } = recv(&mut ui).await {
            assert_eq!(state.unavailable_features, unavailable);
            break;
        }
    }

    // O LoL fechou: nada fica desligado.
    mock.shutdown().await;
    assert!(unavailable_features(&mut ui).await.is_empty());
}

/// As gravações reais respondem bem a todos os endpoints checados: a
/// checagem não pode desligar nada à toa.
#[tokio::test]
async fn com_as_gravacoes_nada_e_desligado() {
    for set in ["16.22", "16.22-aram", "16.22-perfil", "16.22-pos-jogo"] {
        let dir = temp_dir(set);
        let mock = start_mock_with(&dir, set, Broken::default()).await;
        let creds = Credentials::from_lockfile_path(&dir.join("lockfile")).unwrap();
        let client =
            LcuClient::with_root_certificate(&creds, mock.ca_pem().unwrap().as_bytes()).unwrap();
        assert_eq!(health::check(&client, PROBES).await, [], "{set}");
        mock.shutdown().await;
    }
}
