//! Hover, ban e pick pelo `/ws` (#23), com o agente conectado no mock-lcu
//! parado na vez do jogador escolher (gravação 16.22): a UI marca um
//! campeão, confirma, e os pedidos que não valem voltam `invalidCommand` sem
//! chegar na LCU.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::champ_select::{ChampSelect, ChampSelectActionType};
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

const SESSION: &str = "/lol-champ-select/v1/session";

/// Na gravação 16.22 o jogador logado é a célula 2.
const LOCAL_CELL: i32 = 2;

/// A gravação cortada na vez do jogador escolher: o mock fica parado ali.
fn fixtures_until_local_pick() -> FixtureSet {
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
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-acoes-champ-select-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let mut config = Config::new(fixtures_until_local_pick());
    config.speed = 500.0;
    config.replay_start = ReplayStart::Immediately;
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

fn invalid(result: &CommandResult) -> bool {
    matches!(
        result,
        CommandResult::Error {
            code: ErrorCode::InvalidCommand,
            ..
        }
    )
}

#[tokio::test]
async fn marca_e_confirma_o_pick_pelo_ws() {
    let dir = temp_dir("pick");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;

    // Espera a vez do jogador escolher.
    let cs = loop {
        let cs = match recv(&mut ui).await {
            ServerMessage::Hello { state, .. } => state.champ_select,
            ServerMessage::ChampSelect { champ_select } => champ_select,
            _ => continue,
        };
        if let Some(cs) = cs.filter(|cs| {
            cs.pending_action
                .as_ref()
                .is_some_and(|a| a.action_type == ChampSelectActionType::Pick)
        }) {
            break cs;
        }
    };
    let pick = cs.pending_action.clone().unwrap();
    assert_eq!(pick.actor_cell_id, LOCAL_CELL);

    // Pedidos que não valem: sem campeão, ação de outro jogador, ban do
    // jogador já feito. Nada muda na seleção.
    send(
        &mut ui,
        1,
        Command::HoverChampion {
            action_id: pick.id,
            champion_id: 0,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(invalid(&result), "{result:?}");
    // Eventos da inicialização podem chegar entre o pedido inválido e sua
    // resposta; eles não foram causados pelo comando recusado.

    let other = cs
        .actions
        .iter()
        .find(|a| a.actor_cell_id != LOCAL_CELL && a.action_type == ChampSelectActionType::Pick)
        .unwrap();
    send(
        &mut ui,
        2,
        Command::ConfirmAction {
            action_id: other.id,
            champion_id: 1,
        },
    )
    .await;
    assert!(invalid(&until_response(&mut ui, 2).await.1));

    let my_ban = cs
        .actions
        .iter()
        .find(|a| a.actor_cell_id == LOCAL_CELL && a.action_type == ChampSelectActionType::Ban)
        .expect("o jogador baniu antes de escolher");
    assert!(my_ban.completed);
    send(
        &mut ui,
        3,
        Command::ConfirmAction {
            action_id: my_ban.id,
            champion_id: 1,
        },
    )
    .await;
    assert!(invalid(&until_response(&mut ui, 3).await.1));

    // Hover: o campeão aparece no jogador, sem confirmar.
    send(
        &mut ui,
        4,
        Command::HoverChampion {
            action_id: pick.id,
            champion_id: 64,
        },
    )
    .await;
    let (hovered, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    let hovered = hovered.expect("a seleção nova chega antes da resposta");
    assert_eq!(hovered.local_player().unwrap().champion_id, Some(64));
    let action = hovered.pending_action.as_ref().unwrap();
    assert_eq!(action.id, pick.id);
    assert_eq!(action.champion_id, Some(64));

    // Confirmar com outro campeão: vale o da confirmação.
    send(
        &mut ui,
        5,
        Command::ConfirmAction {
            action_id: pick.id,
            champion_id: 121,
        },
    )
    .await;
    let (locked, result) = until_response(&mut ui, 5).await;
    assert_eq!(result, CommandResult::Ok);
    let locked = locked.expect("a seleção nova chega antes da resposta");
    assert_eq!(locked.pending_action, None);
    assert_eq!(locked.local_player().unwrap().champion_id, Some(121));
    let done = locked.actions.iter().find(|a| a.id == pick.id).unwrap();
    assert!(done.completed);
    assert_eq!(done.champion_id, Some(121));

    // Confirmado, não dá para trocar.
    send(
        &mut ui,
        6,
        Command::HoverChampion {
            action_id: pick.id,
            champion_id: 64,
        },
    )
    .await;
    assert!(invalid(&until_response(&mut ui, 6).await.1));

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn fora_da_selecao_nao_fala_com_a_lcu() {
    let dir = temp_dir("fora");
    // Sem reprodução: o mock fica fora da seleção.
    let mut config = Config::new(fixtures_until_local_pick());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    // Espera conectar no LoL.
    loop {
        let connected = match recv(&mut ui).await {
            ServerMessage::Hello { state, .. } => state.lcu,
            ServerMessage::Lcu { status } => status,
            _ => continue,
        };
        if matches!(
            connected,
            easylobby_agent::ws::protocol::LcuStatus::Connected { .. }
        ) {
            break;
        }
    }
    send(
        &mut ui,
        1,
        Command::ConfirmAction {
            action_id: 1,
            champion_id: 1,
        },
    )
    .await;
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

async fn connect(url: &str) -> Ui {
    tokio_tungstenite::connect_async(url).await.unwrap().0
}
