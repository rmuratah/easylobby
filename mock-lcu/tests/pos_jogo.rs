//! Fim de partida simulado (#178) pelo `/ws`: o agente conectado no mock-lcu
//! com o pós-jogo de `16.22-pos-jogo` vê a fase ir de `InProgress` até
//! `EndOfGame`, honra dois jogadores e volta ao lobby pelo "jogar de novo".

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::gameflow::GameflowPhase;
use easylobby_agent::honor::HonorBallot;
use easylobby_agent::ws::protocol::{ClientMessage, Command, CommandResult, ServerMessage};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::post_game::Options;
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22-pos-jogo");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-pos-jogo-{name}-{}", std::process::id()));
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

/// O que a UI acompanha do pós-jogo.
#[derive(Default)]
struct Seen {
    phases: Vec<GameflowPhase>,
    ballot: Option<HonorBallot>,
}

impl Seen {
    fn read(&mut self, message: ServerMessage) -> Option<(u32, CommandResult)> {
        match message {
            ServerMessage::Hello { state, .. } => {
                self.phases.extend(state.gameflow_phase);
                self.ballot = state.honor_ballot;
            }
            ServerMessage::GameflowPhase { phase: Some(phase) } => {
                if self.phases.last() != Some(&phase) {
                    self.phases.push(phase);
                }
            }
            ServerMessage::HonorBallot { honor_ballot } => self.ballot = honor_ballot,
            ServerMessage::Response { id, result } => return Some((id, result)),
            _ => {}
        }
        None
    }

    fn phase(&self) -> Option<GameflowPhase> {
        self.phases.last().copied()
    }

    async fn until(&mut self, ui: &mut Ui, f: impl Fn(&Self) -> bool) {
        while !f(self) {
            let message = recv(ui).await;
            self.read(message);
        }
    }

    async fn send(&mut self, ui: &mut Ui, id: u32, command: Command) -> CommandResult {
        let json = serde_json::to_string(&ClientMessage { id, command }).unwrap();
        ui.send(Message::text(json)).await.unwrap();
        loop {
            let message = recv(ui).await;
            if let Some((got, result)) = self.read(message) {
                if got == id {
                    return result;
                }
            }
        }
    }
}

#[tokio::test]
async fn da_partida_ao_lobby_com_honra() {
    let dir = temp_dir("jogar-de-novo");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    config.speed = 10.0;
    // A partida só acaba quando o teste mandar.
    config.post_game = Some(Options { end_after_ms: None });
    let mock = MockLcu::start(config).await.unwrap();
    assert_eq!(mock.phase(), "InProgress");

    let mut seen = Seen::default();
    seen.until(&mut ui, |s| s.phase() == Some(GameflowPhase::InProgress))
        .await;
    // O agente fecha a UI oficial ao conectar; com ela fechada, é ele quem
    // conclui as telas do `PreEndOfGame` (#180).
    for _ in 0..100 {
        if !mock.ux_open() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!mock.ux_open());

    assert!(mock.end_game());
    seen.until(&mut ui, |s| s.phase() == Some(GameflowPhase::EndOfGame))
        .await;
    assert_eq!(
        seen.phases,
        [
            GameflowPhase::InProgress,
            GameflowPhase::WaitingForStats,
            GameflowPhase::PreEndOfGame,
            GameflowPhase::EndOfGame,
        ]
    );
    assert_eq!(mock.post_game_waiting(), None);

    // A cédula continua aberta no `EndOfGame` (#182): um aliado e um
    // adversário.
    seen.until(&mut ui, |s| s.ballot.is_some()).await;
    let ballot = seen.ballot.clone().unwrap();
    assert_eq!(ballot.game_id, 1124);
    let ally = ballot.eligible_allies[0].puuid.clone();
    let opponent = ballot.eligible_opponents[0].puuid.clone();
    for (id, puuid) in [(1, &ally), (2, &opponent)] {
        let result = seen
            .send(
                &mut ui,
                id,
                Command::HonorPlayer {
                    puuid: puuid.clone(),
                },
            )
            .await;
        assert!(matches!(result, CommandResult::Ok), "{result:?}");
    }
    seen.until(&mut ui, |s| {
        s.ballot
            .as_ref()
            .is_some_and(|b| b.honored_puuids.len() == 2)
    })
    .await;
    assert_eq!(
        seen.ballot.as_ref().unwrap().honored_puuids,
        [ally, opponent]
    );

    let result = seen.send(&mut ui, 3, Command::PlayAgain).await;
    assert!(matches!(result, CommandResult::Ok), "{result:?}");
    seen.until(&mut ui, |s| s.phase() == Some(GameflowPhase::Lobby))
        .await;
    seen.until(&mut ui, |s| s.ballot.is_none()).await;
    assert_eq!(mock.phase(), "Lobby");
}

/// "Sair" no pós-jogo (#180): o agente dispensa as estatísticas e a fase
/// vai para `None`, sem lobby; a cédula some.
#[tokio::test]
async fn sai_do_pos_jogo_para_o_inicio() {
    let dir = temp_dir("sair");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    config.speed = 10.0;
    config.post_game = Some(Options { end_after_ms: None });
    let mock = MockLcu::start(config).await.unwrap();

    let mut seen = Seen::default();
    seen.until(&mut ui, |s| s.phase() == Some(GameflowPhase::InProgress))
        .await;
    // Ainda na partida: a LCU recusa.
    let result = seen.send(&mut ui, 1, Command::LeavePostGame).await;
    assert!(matches!(result, CommandResult::Error { .. }), "{result:?}");

    assert!(mock.end_game());
    seen.until(&mut ui, |s| s.phase() == Some(GameflowPhase::EndOfGame))
        .await;
    seen.until(&mut ui, |s| s.ballot.is_some()).await;

    let result = seen.send(&mut ui, 2, Command::LeavePostGame).await;
    assert!(matches!(result, CommandResult::Ok), "{result:?}");
    seen.until(&mut ui, |s| s.phase() == Some(GameflowPhase::Idle))
        .await;
    seen.until(&mut ui, |s| s.ballot.is_none()).await;
    assert_eq!(mock.phase(), "None");
}

/// Sem ninguém concluir as telas (a UI oficial "aberta"), o pós-jogo para
/// na `honor-vote`, como no LoL real; o avanço do mock conclui cada uma.
#[tokio::test]
async fn telas_esperam_o_complete() {
    let mut config = Config::new(fixtures());
    config.replay_start = ReplayStart::Never;
    config.tls = false;
    config.speed = 10.0;
    config.post_game = Some(Options { end_after_ms: None });
    let mock = MockLcu::start(config).await.unwrap();

    assert!(!mock.advance_post_game());
    assert!(!mock.play_again());
    assert!(mock.end_game());
    let mut screens = Vec::new();
    for _ in 0..400 {
        if let Some(screen) = mock.post_game_waiting() {
            screens.push(screen);
            assert_eq!(mock.phase(), "PreEndOfGame");
            assert!(mock.advance_post_game());
        }
        if mock.phase() == "EndOfGame" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        screens,
        [
            "honor-vote",
            "challenge-level-up-celebration",
            "missions-celebration",
            "ranked-celebration",
        ]
    );
    assert_eq!(mock.phase(), "EndOfGame");

    // Volta para a partida e acaba de novo.
    assert!(mock.restart_game());
    assert_eq!(mock.phase(), "InProgress");
    assert!(mock.end_game());
    for _ in 0..200 {
        if mock.post_game_waiting().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(mock.post_game_waiting().as_deref(), Some("honor-vote"));
}

#[tokio::test]
async fn gravacao_sem_pos_jogo() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&dir).unwrap());
    config.tls = false;
    config.post_game = Some(Options::default());
    assert!(matches!(
        MockLcu::start(config).await,
        Err(mock_lcu::MockError::NoPostGame)
    ));
}
