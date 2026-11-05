//! Trocas na seleção pelo `/ws` (#25), com o agente conectado no mock-lcu
//! parado na vez do jogador escolher (gravação 16.22): a UI pede e cancela
//! uma troca de cargo, um aliado pede uma e a UI aceita ou recusa, e os
//! pedidos que não valem voltam `invalidCommand` sem chegar na LCU.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::champ_select::{ChampSelect, ChampSelectActionType};
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::swaps::{SwapKind, SwapState};
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
        "mock-lcu-trocas-champ-select-{name}-{}",
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

/// Espera a vez do jogador escolher, quando a gravação para.
async fn wait_local_pick(ui: &mut Ui) -> ChampSelect {
    loop {
        let cs = match recv(ui).await {
            ServerMessage::Hello { state, .. } => state.champ_select,
            ServerMessage::ChampSelect { champ_select } => champ_select,
            _ => continue,
        };
        if let Some(cs) = cs.filter(|cs| {
            cs.pending_action
                .as_ref()
                .is_some_and(|a| a.action_type == ChampSelectActionType::Pick)
        }) {
            return cs;
        }
    }
}

fn state_of(cs: &ChampSelect, kind: SwapKind, id: u64) -> SwapState {
    cs.swaps(kind)
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.state)
        .expect("a troca continua na lista")
}

#[tokio::test]
async fn pede_cancela_e_aceita_troca_de_cargo_pelo_ws() {
    let dir = temp_dir("cargo");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;

    let cs = wait_local_pick(&mut ui).await;
    let swap = cs
        .position_swaps
        .iter()
        .find(|s| s.state == SwapState::Available)
        .cloned()
        .expect("a gravação 16.22 tem troca de cargo disponível");
    let my_position = cs.local_player().unwrap().assigned_position;
    let ally_position = cs
        .my_team
        .iter()
        .find(|p| p.cell_id == swap.cell_id)
        .unwrap()
        .assigned_position;
    assert_ne!(my_position, ally_position);

    // Pedidos que não valem: troca que não existe, aceitar sem pedido do
    // aliado, cancelar sem pedido do jogador, id de outro tipo.
    let wrong = [
        Command::RequestSwap {
            kind: SwapKind::Position,
            swap_id: 9_999,
        },
        Command::AcceptSwap {
            kind: SwapKind::Position,
            swap_id: swap.id,
        },
        Command::CancelSwap {
            kind: SwapKind::Position,
            swap_id: swap.id,
        },
    ];
    for (id, command) in (1..).zip(wrong) {
        send(&mut ui, id, command).await;
        let (_, result) = until_response(&mut ui, id).await;
        assert!(invalid(&result), "{result:?}");
    }

    // Pede e cancela.
    send(
        &mut ui,
        10,
        Command::RequestSwap {
            kind: SwapKind::Position,
            swap_id: swap.id,
        },
    )
    .await;
    let (sent, result) = until_response(&mut ui, 10).await;
    assert_eq!(result, CommandResult::Ok);
    let sent = sent.expect("a seleção nova chega antes da resposta");
    assert_eq!(
        state_of(&sent, SwapKind::Position, swap.id),
        SwapState::Sent
    );

    send(
        &mut ui,
        11,
        Command::CancelSwap {
            kind: SwapKind::Position,
            swap_id: swap.id,
        },
    )
    .await;
    let (cancelled, result) = until_response(&mut ui, 11).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(
        state_of(&cancelled.unwrap(), SwapKind::Position, swap.id),
        SwapState::Available
    );

    // O aliado pede; o jogador aceita e os cargos trocam.
    let received = mock
        .receive_swap(SwapKind::Position)
        .expect("há troca de cargo disponível");
    loop {
        if let ServerMessage::ChampSelect {
            champ_select: Some(cs),
        } = recv(&mut ui).await
        {
            if state_of(&cs, SwapKind::Position, received) == SwapState::Received {
                break;
            }
        }
    }
    send(
        &mut ui,
        12,
        Command::AcceptSwap {
            kind: SwapKind::Position,
            swap_id: received,
        },
    )
    .await;
    let (accepted, result) = until_response(&mut ui, 12).await;
    assert_eq!(result, CommandResult::Ok);
    let accepted = accepted.expect("a seleção nova chega antes da resposta");
    let me = accepted.local_player().unwrap();
    let other = accepted
        .my_team
        .iter()
        .find(|p| p.cell_id == swap.cell_id)
        .unwrap();
    if received == swap.id {
        assert_eq!(me.assigned_position, ally_position);
        assert_eq!(other.assigned_position, my_position);
    }
    assert_ne!(me.assigned_position, my_position);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn cancela_troca_de_vez_gravada_e_recusa_troca_de_cargo() {
    let dir = temp_dir("recusa");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    let cs = wait_local_pick(&mut ui).await;

    // Na gravação, o jogador pediu uma troca de vez que ainda está aberta.
    let sent = cs
        .pick_order_swaps
        .iter()
        .find(|s| s.state == SwapState::Sent)
        .cloned()
        .expect("a gravação 16.22 tem uma troca de vez pedida");
    send(
        &mut ui,
        1,
        Command::DeclineSwap {
            kind: SwapKind::PickOrder,
            swap_id: sent.id,
        },
    )
    .await;
    assert!(invalid(&until_response(&mut ui, 1).await.1));
    send(
        &mut ui,
        2,
        Command::CancelSwap {
            kind: SwapKind::PickOrder,
            swap_id: sent.id,
        },
    )
    .await;
    let (cancelled, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let cancelled = cancelled.expect("a seleção nova chega antes da resposta");
    assert!(cancelled
        .pick_order_swaps
        .iter()
        .all(|s| s.state == SwapState::Available));
    assert_eq!(cancelled.actions, cs.actions);

    // Um aliado pede troca de cargo e o jogador recusa: nada muda.
    let received = mock
        .receive_swap(SwapKind::Position)
        .expect("há troca de cargo disponível");
    loop {
        if let ServerMessage::ChampSelect {
            champ_select: Some(cs),
        } = recv(&mut ui).await
        {
            if state_of(&cs, SwapKind::Position, received) == SwapState::Received {
                break;
            }
        }
    }
    send(
        &mut ui,
        3,
        Command::DeclineSwap {
            kind: SwapKind::Position,
            swap_id: received,
        },
    )
    .await;
    let (declined, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    let declined = declined.expect("a seleção nova chega antes da resposta");
    assert_eq!(
        state_of(&declined, SwapKind::Position, received),
        SwapState::Available
    );
    assert_eq!(declined.my_team, cs.my_team);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn fora_da_selecao_nao_fala_com_a_lcu() {
    let dir = temp_dir("fora");
    let mut config = Config::new(fixtures_until_local_pick());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
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
        Command::RequestSwap {
            kind: SwapKind::Champion,
            swap_id: 1,
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
