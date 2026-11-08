//! Estado da seleção de campeões (#22) contra o mock-lcu com a gravação
//! 16.22 (ranqueada Solo/Duo com 10 bans): a UI acompanha a seleção inteira
//! pelo `/ws`, e uma UI ou um agente que chega no meio recebe a seleção no
//! `hello`.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use easylobby_agent::champ_select::{ChampSelect, ChampSelectActionType, ChampSelectPhase};
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::lobby::{Position, Queue};
use easylobby_agent::ws::protocol::ServerMessage;
use easylobby_agent::ws::{self, WsConfig};
use futures_util::StreamExt;
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

const SESSION: &str = "/lol-champ-select/v1/session";

/// Na gravação 16.22 o jogador logado é a célula 2, de selva, e escolhe o
/// campeão 121.
const LOCAL_CELL: i32 = 2;
const LOCAL_CHAMPION: u32 = 121;

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

/// A gravação cortada no meio dos picks, com a vez do jogador aberta: o mock
/// fica parado na seleção.
fn fixtures_until_local_turn() -> FixtureSet {
    let mut set = fixtures();
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
        "mock-lcu-champ-select-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path, fixtures: FixtureSet, replay_start: ReplayStart) -> MockLcu {
    let mut config = Config::new(fixtures);
    config.speed = 500.0;
    config.replay_start = replay_start;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

/// Sobe o agente (conexão com o LoL + `/ws`) numa porta livre.
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

fn champ_selects(messages: &[ServerMessage]) -> Vec<Option<ChampSelect>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::ChampSelect { champ_select } => Some(champ_select.clone()),
            _ => None,
        })
        .collect()
}

fn now_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

#[tokio::test]
async fn acompanha_a_selecao_da_gravacao() {
    let dir = temp_dir("gravacao");
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    let mock = start_mock(&dir, fixtures(), ReplayStart::OnFirstSubscribe).await;

    // A seleção fecha (`Delete`) logo depois de a partida abrir.
    let seen = until(&mut ui, |m| {
        matches!(m, ServerMessage::ChampSelect { champ_select: None })
    })
    .await;
    let sessions = champ_selects(&seen);
    let open: Vec<&ChampSelect> = sessions.iter().flatten().collect();
    assert!(!open.is_empty(), "a seleção não apareceu");

    // A sessão é a mesma do começo ao fim, na Solo/Duo, com os dois times.
    let first = open[0];
    assert_eq!(first.queue, Some(Queue::RankedSolo));
    assert_eq!(first.local_player_cell_id, LOCAL_CELL);
    assert!(first.has_simultaneous_bans);
    assert_eq!(first.my_team.len(), 5);
    assert_eq!(first.their_team.len(), 5);
    assert!(open.iter().all(|cs| cs.id == first.id));
    let me = first.local_player().expect("o jogador está no time");
    assert_eq!(me.assigned_position, Position::Jungle);
    assert!(me.game_name.is_some());
    assert!(first.their_team.iter().all(|p| p.game_name.is_none()));

    // As etapas na ordem, sem repetir.
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
    assert!(open
        .iter()
        .all(|cs| cs.timer.time_left_ms <= cs.timer.total_time_ms));

    // A vez do jogador: um ban e depois um pick, da célula dele.
    let pending: Vec<ChampSelectActionType> = open
        .iter()
        .filter_map(|cs| cs.pending_action.as_ref())
        .inspect(|a| assert_eq!(a.actor_cell_id, LOCAL_CELL))
        .map(|a| a.action_type)
        .collect();
    assert_eq!(pending.first(), Some(&ChampSelectActionType::Ban));
    assert_eq!(pending.last(), Some(&ChampSelectActionType::Pick));

    // No fim: 10 bans, o campeão do jogador escolhido e nada pendente.
    let last = open.last().unwrap();
    assert_eq!(last.bans.my_team.len(), 5, "{:?}", last.bans);
    assert_eq!(last.bans.their_team.len(), 5, "{:?}", last.bans);
    assert_eq!(
        last.local_player().unwrap().champion_id,
        Some(LOCAL_CHAMPION)
    );
    assert_eq!(last.pending_action, None);
    assert!(last
        .actions
        .iter()
        .filter(|a| a.action_type == ChampSelectActionType::Pick && a.is_ally_action)
        .all(|a| a.completed && a.champion_id.is_some()));

    assert_eq!(sessions.last(), Some(&None));
    assert!(seen.iter().any(|m| matches!(
        m,
        ServerMessage::GameflowPhase { phase: Some(p) } if p.as_str() == "GameStart"
    )));

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn quem_chega_no_meio_recebe_a_selecao_no_hello() {
    let dir = temp_dir("no-meio");
    // O mock anda até a vez do jogador antes de o agente existir: o agente
    // tem que ler a seleção pelo REST ao conectar.
    let mock = start_mock(&dir, fixtures_until_local_turn(), ReplayStart::Immediately).await;
    mock.replay_finished().await;
    assert_eq!(mock.phase(), "ChampSelect");

    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    let seen = until(&mut ui, |m| {
        matches!(
            m,
            ServerMessage::ChampSelect {
                champ_select: Some(_)
            }
        )
    })
    .await;
    let from_rest = champ_selects(&seen).pop().flatten().unwrap();
    let pending = from_rest
        .pending_action
        .as_ref()
        .expect("a vez do jogador sumiu");
    assert_eq!(pending.action_type, ChampSelectActionType::Pick);
    assert_eq!(pending.actor_cell_id, LOCAL_CELL);

    // Uma segunda UI recebe a mesma seleção já no hello, com a hora do fim
    // da etapa pelo relógio do computador.
    let mut late = connect(&url).await;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            let cs = state.champ_select.expect("a seleção sumiu do estado");
            assert_eq!(cs, from_rest);
            assert_eq!(cs.timer.phase, ChampSelectPhase::BanPick);
            assert!(cs.timer.ends_at_ms <= now_ms() + u64::from(cs.timer.time_left_ms));
            assert!(cs.timer.ends_at_ms + 60_000 > now_ms());
        }
        other => panic!("esperava hello: {other:?}"),
    }

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
