//! Releituras atrasadas da seleção de campeões e das runas não trazem de
//! volta um estado que já acabou (#87).
//!
//! Como em `resposta_atrasada.rs` (#73): cada teste segura no mock-lcu a
//! releitura que o comando faz depois de dar certo, muda o estado por fora
//! (evento WAMP, queda da conexão ou um LoL novo), espera a UI observadora
//! ver a mudança e só então solta a resposta. O agente tem que descartá-la.

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::http::Method;
use easylobby_agent::champ_select::{
    ChampSelect, ChampSelectActionType, CHAMP_SELECT_SESSION_PATH,
};
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::{EventFixture, FixtureSet};
use easylobby_agent::runes::PAGES_PATH;
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

/// Páginas da gravação 16.22: a em uso no começo e outra.
const CURRENT: u64 = 994_528_921;
const OTHER: u64 = 156_827_123;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22")
}

/// A gravação cortada na vez do jogador escolher: o mock fica parado ali.
fn fixtures_until_local_pick() -> FixtureSet {
    let mut set = FixtureSet::load(&fixtures_dir()).unwrap();
    let turn = set
        .events
        .iter()
        .position(|e| {
            e.uri == CHAMP_SELECT_SESSION_PATH
                && ChampSelect::from_json(&e.data)
                    .and_then(|cs| cs.pending_action)
                    .is_some_and(|a| a.action_type == ChampSelectActionType::Pick)
        })
        .expect("a gravação 16.22 tem a vez do jogador escolher");
    set.events.truncate(turn + 1);
    set
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-atrasada-selecao-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Mock parado na vez do jogador escolher.
async fn start_mock_pick(dir: &Path) -> MockLcu {
    let mut config = Config::new(fixtures_until_local_pick());
    config.speed = 500.0;
    config.replay_start = ReplayStart::Immediately;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();
    // O agente só conecta com a gravação já reproduzida: no meio dela, os
    // eventos atrasados do WAMP chegam depois da leitura pelo REST e voltam
    // a seleção para um estado mais velho (#241).
    mock.replay_finished().await;
    mock
}

/// Mock sem reprodução: as runas só mudam pelos comandos.
async fn start_mock_still(dir: &Path) -> MockLcu {
    let mut config = Config::new(FixtureSet::load(&fixtures_dir()).unwrap());
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
    let ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
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

fn event(uri: &str, event_type: &str, data: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.to_owned(),
        event_type: event_type.to_owned(),
        data,
    }
}

fn is_champ_select(m: &ServerMessage, some: bool) -> bool {
    matches!(m, ServerMessage::ChampSelect { champ_select } if champ_select.is_some() == some)
}

fn is_rune_pages(m: &ServerMessage, some: bool) -> bool {
    matches!(m, ServerMessage::RunePages { rune_pages } if rune_pages.is_some() == some)
}

/// A última seleção publicada em `messages`, se alguma.
fn last_champ_select(messages: &[ServerMessage]) -> Option<Option<ChampSelect>> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::ChampSelect { champ_select } => Some(champ_select.clone()),
        _ => None,
    })
}

fn is_disconnected(m: &ServerMessage) -> bool {
    matches!(
        m,
        ServerMessage::Lcu {
            status: LcuStatus::Disconnected { .. }
        }
    )
}

/// Espera a vez do jogador escolher e devolve o id da ação.
async fn wait_local_pick(ui: &mut Ui) -> u32 {
    loop {
        let cs = match recv(ui).await {
            ServerMessage::Hello { state, .. } => state.champ_select,
            ServerMessage::ChampSelect { champ_select } => champ_select,
            _ => continue,
        };
        if let Some(action) = cs
            .and_then(|cs| cs.pending_action)
            .filter(|a| a.action_type == ChampSelectActionType::Pick)
        {
            return action.id;
        }
    }
}

/// Espera o agente ler as páginas de runas.
async fn wait_rune_pages(ui: &mut Ui) {
    until(ui, |m| match m {
        ServerMessage::Hello { state, .. } => state.rune_pages.is_some(),
        m => is_rune_pages(m, true),
    })
    .await;
}

#[tokio::test]
async fn releitura_da_selecao_depois_do_delete_nao_volta() {
    let dir = temp_dir("selecao-delete");
    let mock = start_mock_pick(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    // A seleção chegou ou pela releitura da conexão ou por um evento; nos
    // dois casos a releitura da conexão já terminou.
    let pick = wait_local_pick(&mut ui).await;

    let mut hold = mock.hold(Method::GET, CHAMP_SELECT_SESSION_PATH);
    send(
        &mut ui,
        1,
        Command::HoverChampion {
            action_id: pick,
            champion_id: 64,
        },
    )
    .await;
    hold.arrived().await;
    // A seleção acaba pelo WAMP (alguém saiu) enquanto a releitura está a
    // caminho.
    mock.emit(event(CHAMP_SELECT_SESSION_PATH, "Delete", json!({})));
    until(&mut observer, |m| is_champ_select(m, false)).await;
    hold.release();

    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(last_champ_select(&before), Some(None), "{before:?}");
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_champ_select(m, true)), "{after:?}");
    assert_eq!(state(&mut ui, 2).await.champ_select, None);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn releitura_da_selecao_depois_da_queda_nao_volta() {
    let dir = temp_dir("selecao-queda");
    let mock = start_mock_pick(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    let pick = wait_local_pick(&mut ui).await;

    let mut hold = mock.hold(Method::GET, CHAMP_SELECT_SESSION_PATH);
    send(
        &mut ui,
        1,
        Command::ConfirmAction {
            action_id: pick,
            champion_id: 121,
        },
    )
    .await;
    hold.arrived().await;
    // O LoL "fecha": sem lockfile o agente não reconecta, mas o REST ainda
    // entrega a resposta segurada.
    std::fs::remove_file(dir.join("lockfile")).unwrap();
    mock.close_websockets();
    until(&mut observer, is_disconnected).await;
    hold.release();

    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(last_champ_select(&before), Some(None), "{before:?}");
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_champ_select(m, true)), "{after:?}");
    assert_eq!(state(&mut ui, 2).await.champ_select, None);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn releitura_das_runas_depois_da_queda_nao_volta() {
    let dir = temp_dir("runas-queda");
    let mock = start_mock_still(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    wait_rune_pages(&mut ui).await;

    let mut hold = mock.hold(Method::GET, PAGES_PATH);
    send(&mut ui, 1, Command::SelectRunePage { page_id: OTHER }).await;
    hold.arrived().await;
    std::fs::remove_file(dir.join("lockfile")).unwrap();
    mock.close_websockets();
    until(&mut observer, is_disconnected).await;
    hold.release();

    let (_, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_rune_pages(m, true)), "{after:?}");
    assert_eq!(state(&mut ui, 2).await.rune_pages, None);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn releitura_das_runas_da_sessao_antiga_nao_entra_na_nova() {
    let dir = temp_dir("runas-sessao");
    let old = start_mock_still(&dir).await;
    let (mut ui, mut observer) = start_agent(&dir).await;
    wait_rune_pages(&mut ui).await;

    // No LoL antigo a página em uso passa a ser OTHER; o novo começa com
    // CURRENT, como na gravação.
    let mut hold = old.hold(Method::GET, PAGES_PATH);
    send(&mut ui, 1, Command::SelectRunePage { page_id: OTHER }).await;
    hold.arrived().await;
    let new = start_mock_still(&dir).await;
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
    // Espera a releitura da sessão nova trazer as páginas.
    until(&mut observer, |m| is_rune_pages(m, true)).await;
    hold.release();

    let (_, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    let after = drain(&mut observer).await;
    assert!(!after.iter().any(|m| is_rune_pages(m, true)), "{after:?}");
    let state = state(&mut ui, 2).await;
    let pages = state.rune_pages.expect("as páginas do LoL novo");
    assert_eq!(pages.current_page_id, Some(CURRENT));
    assert!(matches!(state.lcu, LcuStatus::Connected { session: 2 }));

    new.shutdown().await;
    drop(old);
    let _ = std::fs::remove_dir_all(&dir);
}
