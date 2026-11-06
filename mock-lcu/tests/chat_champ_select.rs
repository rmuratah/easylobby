//! Chat da seleção de campeões (#27) pelo `/ws`, com o agente conectado no
//! mock-lcu parado no meio da seleção da gravação 16.22: a sala abre, um
//! aliado escondido escreve, a UI manda mensagem e a sala fecha com a
//! seleção.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::champ_select_chat::ChampSelectChat;
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::{EventFixture, FixtureSet};
use easylobby_agent::ws::protocol::{
    ClientMessage, Command, CommandResult, ErrorCode, ServerMessage,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

const SESSION: &str = "/lol-champ-select/v1/session";

/// A sala da seleção na gravação 16.22.
const ROOM_URI: &str =
    "/lol-chat/v1/conversations/6923ea3f-b5e8-4310-80f9-54235f3c22b0%40lol-champ-select.br1.pvp.net";

/// Na gravação 16.22 o jogador logado é a célula 2. O aliado com o nome
/// escondido `fake-puuid-0023` começa na célula 0 e troca de lugar com o da
/// célula 4 no meio da seleção; no fim da gravação ele está na 4.
const LOCAL_CELL: i32 = 2;
const HIDDEN_ALLY: &str = "fake-puuid-0023";
const HIDDEN_ALLY_CELL: i32 = 4;

/// A gravação cortada antes de a seleção fechar: o mock fica parado com a
/// sala aberta.
fn fixtures_in_champ_select() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut set = FixtureSet::load(&dir).unwrap();
    let end = set
        .events
        .iter()
        .position(|e| e.uri == SESSION && e.event_type == "Delete")
        .expect("a gravação 16.22 tem o fim da seleção");
    set.events.truncate(end);
    set
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-chat-champ-select-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let mut config = Config::new(fixtures_in_champ_select());
    config.speed = 500.0;
    config.replay_start = ReplayStart::OnFirstSubscribe;
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

/// Espera a próxima `champSelectChat` (ou o `hello` com a sala).
async fn next_room(ui: &mut Ui) -> Option<ChampSelectChat> {
    loop {
        match recv(ui).await {
            ServerMessage::Hello { state, .. } if state.champ_select_chat.is_some() => {
                return state.champ_select_chat
            }
            ServerMessage::ChampSelectChat { champ_select_chat } => return champ_select_chat,
            _ => {}
        }
    }
}

fn room(messages: &[ServerMessage]) -> Option<&ChampSelectChat> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::ChampSelectChat {
            champ_select_chat: Some(room),
        } => Some(room),
        _ => None,
    })
}

fn event(uri: &str, event_type: &str, data: serde_json::Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.into(),
        event_type: event_type.into(),
        data,
    }
}

#[tokio::test]
async fn conversa_na_sala_da_selecao() {
    let dir = temp_dir("sala");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    // A sala abre vazia: na gravação só há avisos de entrada (`system`).
    let sala = next_room(&mut ui).await.expect("a sala não abriu");
    assert!(sala.messages.is_empty(), "{sala:?}");
    // A sessão da seleção chegou antes da sala, com o time.

    // O aliado escondido escreve.
    mock.emit(event(
        &format!("{ROOM_URI}/messages/m_aliado"),
        "Create",
        json!({
            "body": "vou top", "fromId": "0", "fromObfuscatedPuuid": HIDDEN_ALLY,
            "fromObfuscatedSummonerId": 0, "fromPid": "", "fromPuuid": "",
            "fromSummonerId": 0, "id": "m_aliado", "isHistorical": false,
            "timestamp": "2026-11-05T01:15:00.000Z", "type": "chat",
        }),
    ));
    let sala = loop {
        let sala = next_room(&mut ui).await.expect("a sala fechou");
        if !sala.messages.is_empty() {
            break sala;
        }
    };
    assert_eq!(sala.messages.len(), 1);
    assert_eq!(sala.messages[0].body, "vou top");
    assert_eq!(sala.messages[0].from_cell_id, Some(HIDDEN_ALLY_CELL));
    assert!(!sala.messages[0].from_me);

    // Texto vazio nem chega na LCU.
    send(
        &mut ui,
        1,
        Command::SendChampSelectMessage { body: " ".into() },
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

    // A mensagem do jogador chega na sala antes da resposta.
    send(
        &mut ui,
        2,
        Command::SendChampSelectMessage {
            body: "vou mid".into(),
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let sala = room(&before).expect("a mensagem não chegou").clone();
    let resumo: Vec<(&str, Option<i32>, bool)> = sala
        .messages
        .iter()
        .map(|m| (m.body.as_str(), m.from_cell_id, m.from_me))
        .collect();
    assert_eq!(
        resumo,
        [
            ("vou top", Some(HIDDEN_ALLY_CELL), false),
            ("vou mid", Some(LOCAL_CELL), true),
        ]
    );
    // A sala não vira conversa 1:1.
    assert!(!before
        .iter()
        .any(|m| matches!(m, ServerMessage::Conversation { .. })));

    // Uma UI nova recebe a sala no hello.
    let mut late = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            assert_eq!(state.champ_select_chat.unwrap().messages.len(), 2);
            assert!(state
                .conversations
                .iter()
                .all(|c| !c.id.contains("champ-select")));
        }
        other => panic!("esperava hello: {other:?}"),
    }

    // A seleção acaba e a sala fecha; mandar agora é inválido.
    mock.emit(event(ROOM_URI, "Delete", json!(null)));
    assert_eq!(next_room(&mut ui).await, None);
    send(
        &mut ui,
        3,
        Command::SendChampSelectMessage { body: "gg".into() },
    )
    .await;
    let (_, result) = until_response(&mut ui, 3).await;
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

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
