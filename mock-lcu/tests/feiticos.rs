//! Feitiços de invocador pelo `/ws` (#24), com o agente conectado no mock-lcu
//! parado na seleção de campeões da gravação 16.22 (Ranqueada Solo/Duo): a
//! UI recebe a lista de feitiços, troca os dois e inverte D e F; os pedidos
//! que não valem voltam `invalidCommand` sem chegar na LCU.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::champ_select::{ChampSelect, ChampSelectActionType};
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::lobby::Queue;
use easylobby_agent::summoner_spells::SummonerSpellCatalog;
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

const SESSION: &str = "/lol-champ-select/v1/session";

/// A gravação cortada na vez do jogador escolher: o mock fica parado na
/// seleção.
fn fixtures_in_champ_select() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut set = FixtureSet::load(&dir).unwrap();
    let turn = set
        .events
        .iter()
        .position(|e| {
            e.uri == SESSION
                && ChampSelect::from_json(&e.data)
                    .and_then(|cs| cs.pending_action)
                    .is_some_and(|a| a.action_type == ChampSelectActionType::Pick)
        })
        .expect("a gravação 16.22 tem a vez do jogador escolher");
    set.events.truncate(turn + 1);
    set
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-feiticos-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn config(dir: &Path, replay: ReplayStart) -> Config {
    let mut config = Config::new(fixtures_in_champ_select());
    config.speed = 500.0;
    config.replay_start = replay;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    config
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

/// Lê mensagens até a resposta do comando `id`, devolvendo a última seleção
/// que chegou antes dela e o resultado.
async fn until_response(ui: &mut Ui, id: u32) -> (Option<ChampSelect>, CommandResult) {
    let mut last = None;
    loop {
        match recv(ui).await {
            ServerMessage::Response { id: got, result } if got == id => return (last, result),
            ServerMessage::ChampSelect { champ_select } => last = champ_select,
            _ => {}
        }
    }
}

async fn set_spells(ui: &mut Ui, id: u32, spell1_id: u32, spell2_id: u32) {
    send(
        ui,
        id,
        Command::SetSummonerSpells {
            spell1_id,
            spell2_id,
        },
    )
    .await;
}

fn invalid(result: &CommandResult) -> bool {
    matches!(
        result,
        CommandResult::Error {
            code: ErrorCode::InvalidCommand,
            ..
        }
    )
}

fn spells(cs: &ChampSelect) -> (Option<u32>, Option<u32>) {
    let me = cs.local_player().expect("o jogador está no time");
    (me.spell1_id, me.spell2_id)
}

#[tokio::test]
async fn troca_os_feiticos_pelo_ws() {
    let dir = temp_dir("troca");
    let mock = MockLcu::start(config(&dir, ReplayStart::Immediately))
        .await
        .unwrap();
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;

    // Espera a lista de feitiços e o fim da reprodução (o último evento é a
    // vez do jogador escolher). Antes disso, um evento gravado com os
    // feitiços da gravação pode chegar depois da troca e desfazê-la.
    let at_local_pick = |cs: &ChampSelect| {
        cs.pending_action
            .as_ref()
            .is_some_and(|a| a.action_type == ChampSelectActionType::Pick)
    };
    let mut catalog: Option<SummonerSpellCatalog> = None;
    let mut champ_select: Option<ChampSelect> = None;
    while catalog.is_none() || champ_select.is_none() {
        match recv(&mut ui).await {
            ServerMessage::Hello { state, .. } => {
                catalog = state.summoner_spell_catalog;
                champ_select = state.champ_select.filter(at_local_pick);
            }
            ServerMessage::SummonerSpellCatalog {
                summoner_spell_catalog,
            } => catalog = summoner_spell_catalog,
            ServerMessage::ChampSelect {
                champ_select: Some(cs),
            } if at_local_pick(&cs) => champ_select = Some(cs),
            _ => {}
        }
    }
    let catalog = catalog.unwrap();
    let cs = champ_select.unwrap();
    assert_eq!(cs.queue, Some(Queue::RankedSolo));
    let flash = catalog.get(4).expect("Flash está na lista");
    assert_eq!(flash.name, "Flash");
    assert!(flash.allowed_in(Queue::RankedSolo));
    // Marcar é só do ARAM.
    assert_eq!(catalog.get(32).unwrap().queues, [Queue::Aram]);

    // Pedidos que não valem.
    set_spells(&mut ui, 1, 4, 4).await;
    assert!(invalid(&until_response(&mut ui, 1).await.1));
    set_spells(&mut ui, 2, 4, 0).await;
    assert!(invalid(&until_response(&mut ui, 2).await.1));
    set_spells(&mut ui, 3, 4, 32).await;
    match until_response(&mut ui, 3).await.1 {
        CommandResult::Error {
            code: ErrorCode::InvalidCommand,
            message,
        } => assert!(message.contains("fila"), "{message}"),
        other => panic!("esperava invalidCommand: {other:?}"),
    }
    set_spells(&mut ui, 4, 4, 9999).await;
    assert!(invalid(&until_response(&mut ui, 4).await.1));

    // Teleporte e Flash: a seleção nova chega antes da resposta.
    set_spells(&mut ui, 5, 12, 4).await;
    let (changed, result) = until_response(&mut ui, 5).await;
    assert_eq!(result, CommandResult::Ok);
    let changed = changed.expect("a seleção nova chega antes da resposta");
    assert_eq!(spells(&changed), (Some(12), Some(4)));

    // Inverter D e F é mandar os mesmos trocados.
    set_spells(&mut ui, 6, 4, 12).await;
    let (swapped, result) = until_response(&mut ui, 6).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(spells(&swapped.unwrap()), (Some(4), Some(12)));

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn fora_da_selecao_nao_fala_com_a_lcu() {
    let dir = temp_dir("fora");
    // Sem reprodução: o mock fica fora da seleção.
    let mock = MockLcu::start(config(&dir, ReplayStart::Never))
        .await
        .unwrap();
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    loop {
        let status = match recv(&mut ui).await {
            ServerMessage::Hello { state, .. } => state.lcu,
            ServerMessage::Lcu { status } => status,
            _ => continue,
        };
        if matches!(status, LcuStatus::Connected { .. }) {
            break;
        }
    }
    set_spells(&mut ui, 1, 4, 14).await;
    match until_response(&mut ui, 1).await.1 {
        CommandResult::Error {
            code: ErrorCode::InvalidCommand,
            message,
        } => assert!(message.contains("seleção"), "{message}"),
        other => panic!("esperava invalidCommand: {other:?}"),
    }

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
