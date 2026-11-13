//! Seleção de campeões simulada (#440) pelo `/ws`: o agente conectado no
//! mock-lcu com `--selecao draft` acompanha o draft inteiro, com os bots
//! agindo sozinhos, sem nenhum evento da gravação.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::champ_select::{ChampSelect, ChampSelectActionType, ChampSelectPhase};
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ws::protocol::{
    ClientMessage, Command, CommandResult, ErrorCode, ServerMessage,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::scenario::{Mode, Options, Timings};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mock-lcu-selecao-simulada-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
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

#[tokio::test]
async fn draft_simulado_do_planejamento_a_partida() {
    let dir = temp_dir("draft");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    config.champ_select = Some(Options {
        mode: Mode::Draft,
        cell: Some(1),
    });
    // Etapas curtas; o planejamento e o pick do jogador esperam o `advance`
    // do teste, para o agente não perder o planejamento se conectar devagar.
    config.timings = Timings {
        planning: 60_000,
        ban: 600,
        reveal: 200,
        pick: 60_000,
        finalization: 300,
        bot: 100,
    };
    let mock = MockLcu::start(config).await.unwrap();

    let mut open: Vec<ChampSelect> = Vec::new();
    let mut planning_seen = false;
    loop {
        match recv(&mut ui).await {
            ServerMessage::ChampSelect {
                champ_select: Some(cs),
            } => {
                let my_pick = cs
                    .pending_action
                    .as_ref()
                    .is_some_and(|a| a.action_type == ChampSelectActionType::Pick);
                let planning = !planning_seen && cs.timer.phase == ChampSelectPhase::Planning;
                planning_seen |= planning;
                open.push(cs);
                if planning || my_pick {
                    // Planejamento visto: avança. No pick, o jogador "não
                    // escolhe": avançar dá um campeão a ele.
                    assert!(mock.advance_champ_select());
                }
            }
            ServerMessage::ChampSelect { champ_select: None } if !open.is_empty() => break,
            _ => {}
        }
    }

    let first = &open[0];
    assert_eq!(first.local_player_cell_id, 1);
    assert_eq!(first.my_team.len(), 5);
    assert_eq!(first.their_team.len(), 5);
    let mut phases: Vec<ChampSelectPhase> = open.iter().map(|cs| cs.timer.phase).collect();
    phases.dedup();
    assert_eq!(
        phases,
        [
            ChampSelectPhase::Planning,
            ChampSelectPhase::BanPick,
            ChampSelectPhase::Finalization,
            ChampSelectPhase::GameStarting,
        ]
    );
    let mut pending: Vec<ChampSelectActionType> = open
        .iter()
        .filter_map(|cs| cs.pending_action.as_ref())
        .inspect(|a| assert_eq!(a.actor_cell_id, 1))
        .map(|a| a.action_type)
        .collect();
    pending.dedup();
    assert_eq!(
        pending,
        [ChampSelectActionType::Ban, ChampSelectActionType::Pick]
    );
    // Todo mundo terminou com um campeão.
    let last = open.last().unwrap();
    assert!(last
        .my_team
        .iter()
        .chain(&last.their_team)
        .all(|p| p.champion_id.is_some()));
    assert!(!mock.champ_select_running());
    assert_eq!(mock.phase(), "InProgress");
}

async fn send(ui: &mut Ui, id: u32, command: Command) -> (Option<ChampSelect>, CommandResult) {
    let json = serde_json::to_string(&ClientMessage { id, command }).unwrap();
    ui.send(Message::text(json)).await.unwrap();
    let mut last = None;
    loop {
        match recv(ui).await {
            ServerMessage::Response { id: got, result } if got == id => return (last, result),
            ServerMessage::ChampSelect { champ_select } => last = champ_select,
            _ => {}
        }
    }
}

/// Lê mensagens até uma seleção que satisfaça `f`.
async fn until(ui: &mut Ui, f: impl Fn(&ChampSelect) -> bool) -> ChampSelect {
    loop {
        let cs = match recv(ui).await {
            ServerMessage::Hello { state, .. } => state.champ_select,
            ServerMessage::ChampSelect { champ_select } => champ_select,
            _ => continue,
        };
        if let Some(cs) = cs.filter(|cs| f(cs)) {
            return cs;
        }
    }
}

/// Ranqueada Solo/Duo (#311): no planejamento o ban do jogador já vem
/// `isInProgress`, mas não é a vez dele. O agente recusa o ban e aceita a
/// intenção de pick; em `BAN_PICK`, o ban confirma pelo `/ws`.
#[tokio::test]
async fn ban_na_solo_duo_so_confirma_depois_do_planejamento() {
    let dir = temp_dir("ban");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    config.champ_select = Some(Options {
        mode: Mode::Draft,
        cell: Some(1),
    });
    // O planejamento e os bans só acabam pelo `advance` do teste.
    config.timings = Timings {
        planning: 60_000,
        ban: 60_000,
        reveal: 200,
        pick: 60_000,
        finalization: 300,
        bot: 100,
    };
    let mock = MockLcu::start(config).await.unwrap();

    let planning = until(&mut ui, |cs| cs.timer.phase == ChampSelectPhase::Planning).await;
    assert_eq!(planning.pending_action, None);
    let mine = |cs: &ChampSelect, kind| {
        cs.actions
            .iter()
            .find(|a| a.actor_cell_id == 1 && a.action_type == kind)
            .cloned()
            .unwrap()
    };
    let ban = mine(&planning, ChampSelectActionType::Ban);
    assert!(
        ban.is_in_progress,
        "como na gravação, o ban já vem em andamento"
    );
    let pick = mine(&planning, ChampSelectActionType::Pick);

    // No planejamento: o ban não vale, nem marcar nem confirmar.
    for (id, command) in [
        (
            1,
            Command::HoverChampion {
                action_id: ban.id,
                champion_id: 1,
            },
        ),
        (
            2,
            Command::ConfirmAction {
                action_id: ban.id,
                champion_id: 1,
            },
        ),
    ] {
        let (_, result) = send(&mut ui, id, command).await;
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
    }
    // A intenção de pick vale.
    let (_, result) = send(
        &mut ui,
        3,
        Command::HoverChampion {
            action_id: pick.id,
            champion_id: 64,
        },
    )
    .await;
    assert!(matches!(result, CommandResult::Ok), "{result:?}");

    // Bans: agora é a vez do jogador, e o Banir confirma.
    assert!(mock.advance_champ_select());
    let bans = until(&mut ui, |cs| {
        cs.timer.phase == ChampSelectPhase::BanPick
            && cs.pending_action.as_ref().is_some_and(|a| a.id == ban.id)
    })
    .await;
    assert_eq!(
        bans.pending_action.unwrap().action_type,
        ChampSelectActionType::Ban
    );
    let (_, result) = send(
        &mut ui,
        4,
        Command::HoverChampion {
            action_id: ban.id,
            champion_id: 1,
        },
    )
    .await;
    assert!(matches!(result, CommandResult::Ok), "{result:?}");
    let (_, result) = send(
        &mut ui,
        5,
        Command::ConfirmAction {
            action_id: ban.id,
            champion_id: 1,
        },
    )
    .await;
    assert!(matches!(result, CommandResult::Ok), "{result:?}");
    let done = until(&mut ui, |cs| {
        cs.actions.iter().any(|a| a.id == ban.id && a.completed)
    })
    .await;
    let banned = done.actions.iter().find(|a| a.id == ban.id).unwrap();
    assert_eq!(banned.champion_id, Some(1));
}
