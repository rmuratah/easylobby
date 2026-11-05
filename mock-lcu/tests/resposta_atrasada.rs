//! Respostas REST atrasadas não sobrescrevem um estado mais novo (#73).
//!
//! Cada teste segura no mock-lcu a resposta de que um comando depende, muda o
//! estado por fora (evento WAMP, queda da conexão ou um LoL novo), espera
//! uma segunda UI (a observadora) ver a mudança e só então solta a resposta.
//! A UI que mandou o comando não serve para isso: ela só recebe as mudanças
//! junto com a resposta. O agente tem que descartar a resposta velha: o
//! estado mais novo continua valendo e nada de antigo é publicado no `/ws`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::http::Method;
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::{EventFixture, FixtureSet};
use easylobby_agent::lobby::{Lobby, Position, Queue, LOBBY_PATH};
use easylobby_agent::matchmaking::{Matchmaking, SearchState, LOBBY_SEARCH_PATH, SEARCH_PATH};
use easylobby_agent::ws::protocol::{
    AgentState, ClientMessage, Command, CommandResult, LcuStatus, ServerMessage,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-atrasada-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    // Sem reprodução: o estado só muda pelos comandos e pelo teste.
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

/// Sobe o agente e conecta duas UIs: a que manda os comandos e a
/// observadora.
async fn start_agent(dir: &Path) -> (Ui, Ui) {
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}{}", listener.local_addr().unwrap(), ws::WS_PATH);
    tokio::spawn(ws::run(lcu, listener, WsConfig::default()));
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    // Espera conectar e reler o estado pelo REST.
    until(&mut ui, |m| match m {
        ServerMessage::Hello { state, .. } => state.summoner.is_some(),
        ServerMessage::Summoner { summoner } => summoner.is_some(),
        _ => false,
    })
    .await;
    let mut observer = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    until(&mut observer, |m| matches!(m, ServerMessage::Hello { .. })).await;
    (ui, observer)
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

/// Lê mensagens até uma que satisfaça `done`, devolvendo todas, ela inclusa.
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

async fn send(ui: &mut Ui, id: u32, command: Command) {
    let json = serde_json::to_string(&ClientMessage { id, command }).unwrap();
    ui.send(Message::text(json)).await.unwrap();
}

/// Lê até a resposta do comando `id`, devolvendo as mensagens de antes e o
/// resultado.
async fn until_response(ui: &mut Ui, id: u32) -> (Vec<ServerMessage>, CommandResult) {
    let mut seen = until(
        ui,
        |m| matches!(m, ServerMessage::Response { id: got, .. } if *got == id),
    )
    .await;
    match seen.pop() {
        Some(ServerMessage::Response { result, .. }) => (seen, result),
        other => panic!("esperava a resposta: {other:?}"),
    }
}

/// Estado atual do agente, pelo `getState`.
async fn state(ui: &mut Ui, id: u32) -> AgentState {
    send(ui, id, Command::GetState).await;
    let (before, result) = until_response(ui, id).await;
    assert_eq!(result, CommandResult::Ok);
    match before
        .into_iter()
        .rev()
        .find(|m| matches!(m, ServerMessage::Hello { .. }))
    {
        // `AgentState::clone` aceita o estado com ou sem `Box`.
        Some(ServerMessage::Hello { state, .. }) => AgentState::clone(&state),
        _ => panic!("esperava hello"),
    }
}

/// O que a observadora recebeu até agora: um `ping` devolve antes da
/// resposta tudo o que já foi publicado.
async fn drain(observer: &mut Ui) -> Vec<ServerMessage> {
    send(observer, 1000, Command::Ping).await;
    until_response(observer, 1000).await.0
}

async fn command_ok(ui: &mut Ui, id: u32, command: Command) {
    send(ui, id, command).await;
    let (_, result) = until_response(ui, id).await;
    assert_eq!(result, CommandResult::Ok);
}

fn event(uri: &str, event_type: &str, data: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.to_owned(),
        event_type: event_type.to_owned(),
        data,
    }
}

/// A última busca publicada em `messages`, se alguma.
fn last_search(messages: &[ServerMessage]) -> Option<Option<Matchmaking>> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::Matchmaking { matchmaking } => Some(matchmaking.clone()),
        _ => None,
    })
}

/// O último lobby publicado em `messages`, se algum.
fn last_lobby(messages: &[ServerMessage]) -> Option<Option<Lobby>> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::Lobby { lobby } => Some(lobby.clone()),
        _ => None,
    })
}

fn is_search(m: &ServerMessage, some: bool) -> bool {
    matches!(m, ServerMessage::Matchmaking { matchmaking } if matchmaking.is_some() == some)
}

fn is_lobby(m: &ServerMessage, some: bool) -> bool {
    matches!(m, ServerMessage::Lobby { lobby } if lobby.is_some() == some)
}

/// Uma busca nova, diferente da que o mock cria.
fn new_search() -> Value {
    json!({
        "searchState": "Searching",
        "queueId": 420,
        "timeInQueue": 99.0,
        "estimatedQueueTime": 10.0,
        "errors": [],
    })
}

/// Um lobby novo, diferente do que o comando criou.
fn new_lobby() -> Value {
    json!({
        "canStartActivity": true,
        "gameConfig": {"queueId": 450, "maxLobbySize": 5},
        "localMember": {"puuid": "p", "isLeader": true},
        "members": [{"puuid": "p", "summonerId": 1, "isLeader": true}],
    })
}

#[tokio::test]
async fn releitura_da_busca_depois_do_delete_nao_volta() {
    let dir = temp_dir("start-delete");
    let mock = start_mock(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    command_ok(&mut ui, 1, Command::CreateLobby { queue: Queue::Aram }).await;

    let mut hold = mock.hold(Method::GET, SEARCH_PATH);
    send(&mut ui, 2, Command::StartQueue).await;
    hold.arrived().await;
    // A busca acaba pelo WAMP enquanto a releitura está a caminho.
    mock.emit(event(SEARCH_PATH, "Delete", Value::Null));
    until(&mut observer, |m| is_search(m, false)).await;
    hold.release();

    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    // O estado de antes da resposta é o novo, e nada velho saiu depois.
    assert_eq!(last_search(&before), Some(None), "{before:?}");
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_search(m, true)), "{after:?}");
    assert_eq!(state(&mut ui, 3).await.matchmaking, None);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sair_da_fila_atrasado_nao_apaga_a_busca_nova() {
    let dir = temp_dir("stop-nova");
    let mock = start_mock(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    command_ok(&mut ui, 1, Command::CreateLobby { queue: Queue::Aram }).await;
    command_ok(&mut ui, 2, Command::StartQueue).await;

    let mut hold = mock.hold(Method::DELETE, LOBBY_SEARCH_PATH);
    send(&mut ui, 3, Command::StopQueue).await;
    hold.arrived().await;
    // O mock já avisou a saída; uma busca nova começa antes da resposta.
    until(&mut observer, |m| is_search(m, false)).await;
    mock.emit(event(SEARCH_PATH, "Create", new_search()));
    until(&mut observer, |m| is_search(m, true)).await;
    hold.release();

    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    let last = last_search(&before).flatten().expect("a busca nova");
    assert_eq!(last.time_in_queue, 99.0);
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_search(m, false)), "{after:?}");
    let search = state(&mut ui, 4).await.matchmaking.expect("a busca nova");
    assert_eq!(search.search_state, SearchState::Searching);
    assert_eq!(search.time_in_queue, 99.0);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn criar_lobby_atrasado_nao_volta_depois_do_delete() {
    let dir = temp_dir("create-delete");
    let mock = start_mock(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;

    let mut hold = mock.hold(Method::POST, LOBBY_PATH);
    send(&mut ui, 1, Command::CreateLobby { queue: Queue::Aram }).await;
    hold.arrived().await;
    until(&mut observer, |m| is_lobby(m, true)).await;
    mock.emit(event(LOBBY_PATH, "Delete", Value::Null));
    until(&mut observer, |m| is_lobby(m, false)).await;
    hold.release();

    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(last_lobby(&before), Some(None), "{before:?}");
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_lobby(m, true)), "{after:?}");
    assert_eq!(state(&mut ui, 2).await.lobby, None);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sair_do_lobby_atrasado_nao_apaga_o_lobby_novo() {
    let dir = temp_dir("leave-novo");
    let mock = start_mock(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    command_ok(&mut ui, 1, Command::CreateLobby { queue: Queue::Aram }).await;

    let mut hold = mock.hold(Method::DELETE, LOBBY_PATH);
    send(&mut ui, 2, Command::LeaveLobby).await;
    hold.arrived().await;
    until(&mut observer, |m| is_lobby(m, false)).await;
    mock.emit(event(LOBBY_PATH, "Create", new_lobby()));
    until(&mut observer, |m| is_lobby(m, true)).await;
    hold.release();

    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    assert!(matches!(last_lobby(&before), Some(Some(_))), "{before:?}");
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_lobby(m, false)), "{after:?}");
    let lobby = state(&mut ui, 3).await.lobby.expect("o lobby novo");
    assert_eq!(lobby.queue, Some(Queue::Aram));

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn releitura_dos_cargos_depois_do_delete_nao_volta() {
    let dir = temp_dir("cargos-delete");
    let mock = start_mock(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    command_ok(
        &mut ui,
        1,
        Command::CreateLobby {
            queue: Queue::RankedSolo,
        },
    )
    .await;

    let mut hold = mock.hold(Method::GET, LOBBY_PATH);
    send(
        &mut ui,
        2,
        Command::SetPositions {
            first: Position::Top,
            second: Position::Jungle,
        },
    )
    .await;
    hold.arrived().await;
    mock.emit(event(LOBBY_PATH, "Delete", Value::Null));
    until(&mut observer, |m| is_lobby(m, false)).await;
    hold.release();

    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(last_lobby(&before), Some(None), "{before:?}");
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_lobby(m, true)), "{after:?}");
    assert_eq!(state(&mut ui, 3).await.lobby, None);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn releitura_da_busca_depois_da_queda_nao_volta() {
    let dir = temp_dir("start-queda");
    let mock = start_mock(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    command_ok(&mut ui, 1, Command::CreateLobby { queue: Queue::Aram }).await;

    let mut hold = mock.hold(Method::GET, SEARCH_PATH);
    send(&mut ui, 2, Command::StartQueue).await;
    hold.arrived().await;
    // O LoL "fecha": sem lockfile o agente não reconecta, mas o REST ainda
    // entrega a resposta segurada.
    std::fs::remove_file(dir.join("lockfile")).unwrap();
    mock.close_websockets();
    until(&mut observer, |m| {
        matches!(
            m,
            ServerMessage::Lcu {
                status: LcuStatus::Disconnected { .. }
            }
        )
    })
    .await;
    hold.release();

    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(last_search(&before), Some(None), "{before:?}");
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_search(m, true)), "{after:?}");
    let state = state(&mut ui, 3).await;
    assert_eq!(state.matchmaking, None);
    assert_eq!(state.lobby, None);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn releitura_da_sessao_antiga_nao_entra_na_nova() {
    let dir = temp_dir("start-sessao");
    let old = start_mock(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    command_ok(&mut ui, 1, Command::CreateLobby { queue: Queue::Aram }).await;

    let mut hold = old.hold(Method::GET, SEARCH_PATH);
    send(&mut ui, 2, Command::StartQueue).await;
    hold.arrived().await;
    // Um LoL novo sobe (lockfile e CA novos) e o antigo cai.
    let new = start_mock(&dir).await;
    old.close_websockets();
    until(&mut observer, |m| {
        matches!(
            m,
            ServerMessage::Lcu {
                status: LcuStatus::Connected { session: 2 }
            }
        )
    })
    .await;
    // Espera a releitura da sessão nova terminar.
    until(
        &mut observer,
        |m| matches!(m, ServerMessage::Summoner { summoner } if summoner.is_some()),
    )
    .await;
    hold.release();

    let (_, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_search(m, true)), "{after:?}");
    let state = state(&mut ui, 3).await;
    assert_eq!(state.matchmaking, None);
    assert!(matches!(state.lcu, LcuStatus::Connected { session: 2 }));

    new.shutdown().await;
    drop(old);
    let _ = std::fs::remove_dir_all(&dir);
}
