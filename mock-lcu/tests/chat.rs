//! Chat 1:1 (#31) pelo `/ws`, com o agente conectado no mock-lcu: a UI manda
//! mensagem para um amigo, vê a conversa abrir com a mensagem antes da
//! resposta, marca a conversa ativa e recebe as conversas da gravação.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::chat::Conversation;
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ws::protocol::{
    ClientMessage, Command, CommandResult, ErrorCode, ServerMessage,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-chat-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path, replay: ReplayStart) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    config.replay_start = replay;
    // A gravação inteira em poucos segundos.
    config.speed = 50.0;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

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

/// Lê mensagens até a resposta do comando `id`, devolvendo as de antes e o
/// resultado.
async fn until_response(ui: &mut Ui, id: u32) -> (Vec<ServerMessage>, CommandResult) {
    let mut before = Vec::new();
    loop {
        match recv(ui).await {
            ServerMessage::Response { id: got, result } if got == id => return (before, result),
            other => before.push(other),
        }
    }
}

/// Espera o agente conectar e reler o estado pelo REST.
async fn wait_summoner(ui: &mut Ui) {
    loop {
        match recv(ui).await {
            ServerMessage::Hello { state, .. } if state.summoner.is_some() => break,
            ServerMessage::Summoner { summoner: Some(_) } => break,
            _ => {}
        }
    }
}

fn conversation(messages: &[ServerMessage]) -> Option<&Conversation> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::Conversation { conversation } => Some(conversation),
        _ => None,
    })
}

/// Amigo da lista gravada em `/lol-chat/v1/friends`.
const AMIGO: &str = "fake-pid-0002";

#[tokio::test]
async fn manda_mensagem_e_marca_a_conversa_ativa() {
    let dir = temp_dir("comandos");
    let mock = start_mock(&dir, ReplayStart::Never).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    // Texto vazio nem chega na LCU.
    send(
        &mut ui,
        1,
        Command::SendChatMessage {
            conversation_id: AMIGO.into(),
            body: "  ".into(),
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(
        matches!(
            result,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ),
        "{result:?}"
    );

    // A primeira mensagem abre a conversa, que chega antes da resposta.
    send(
        &mut ui,
        2,
        Command::SendChatMessage {
            conversation_id: AMIGO.into(),
            body: "bora uma?".into(),
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let conversa = conversation(&before)
        .expect("a conversa não chegou")
        .clone();
    assert_eq!(conversa.id, AMIGO);
    assert_eq!(conversa.game_name, "fake-name-0002");
    assert_eq!(conversa.messages.len(), 1);
    assert_eq!(conversa.messages[0].body, "bora uma?");
    assert!(conversa.messages[0].from_me);

    // A segunda entra no histórico, sem repetir a primeira.
    send(
        &mut ui,
        3,
        Command::SendChatMessage {
            conversation_id: AMIGO.into(),
            body: "vou criar o lobby".into(),
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    let bodies: Vec<String> = conversation(&before)
        .unwrap()
        .messages
        .iter()
        .map(|m| m.body.clone())
        .collect();
    assert_eq!(bodies, ["bora uma?", "vou criar o lobby"]);

    // Uma UI nova recebe a conversa e o histórico no hello.
    let mut late = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            assert_eq!(state.conversations.len(), 1);
            assert_eq!(state.conversations[0].messages.len(), 2);
        }
        other => panic!("esperava hello: {other:?}"),
    }

    // Abrir e fechar a conversa na tela.
    send(
        &mut ui,
        4,
        Command::SetActiveConversation {
            conversation_id: Some(AMIGO.into()),
        },
    )
    .await;
    assert_eq!(until_response(&mut ui, 4).await.1, CommandResult::Ok);
    send(
        &mut ui,
        5,
        Command::SetActiveConversation {
            conversation_id: None,
        },
    )
    .await;
    assert_eq!(until_response(&mut ui, 5).await.1, CommandResult::Ok);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn segue_as_conversas_da_gravacao() {
    let dir = temp_dir("gravacao");
    let mock = start_mock(&dir, ReplayStart::OnFirstSubscribe).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    // A gravação tem uma conversa 1:1 com mensagens dos dois lados e a sala
    // da seleção de campeões, que fica de fora.
    let mut ultima = None;
    loop {
        match recv(&mut ui).await {
            ServerMessage::Conversation { conversation } => {
                assert!(
                    !conversation.id.contains("champ-select"),
                    "{conversation:?}"
                );
                let ambos = conversation.messages.iter().any(|m| m.from_me)
                    && conversation.messages.iter().any(|m| !m.from_me);
                let completa = conversation.messages.len() >= 3;
                ultima = Some(conversation);
                if ambos && completa {
                    break;
                }
            }
            ServerMessage::GameflowPhase { phase: Some(phase) }
                if phase.as_str() == "InProgress" =>
            {
                panic!("a gravação acabou sem a conversa completa: {ultima:?}");
            }
            _ => {}
        }
    }
    let conversa = ultima.unwrap();
    // Sem repetidas e em ordem.
    let mut ids: Vec<&str> = conversa.messages.iter().map(|m| m.id.as_str()).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total);
    assert!(conversa
        .messages
        .windows(2)
        .all(|w| w[0].timestamp <= w[1].timestamp));

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
