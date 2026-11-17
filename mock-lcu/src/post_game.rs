//! Fim de partida simulado e controlável (#178).
//!
//! Para desenvolver o pós-jogo (resultado e placar, votação de honra e
//! "Jogar de novo") sem jogar uma partida inteira a cada teste. O mock
//! começa no meio da partida gravada (`InProgress`) e, quando a partida
//! acaba, reproduz o fim de partida da gravação (#177) na ordem gravada:
//! `WaitingForStats`, o `eog-stats-block`, `PreEndOfGame` com as telas da
//! sequência e `EndOfGame`.
//!
//! Como no LoL real com a UI oficial fechada, cada tela do `PreEndOfGame`
//! (`honor-vote`, `challenge-level-up-celebration`...) fica esperando o
//! `POST /lol-pre-end-of-game/v1/complete/{nome}` dela, e a fase só vai para
//! `EndOfGame` depois da última. `POST /lol-lobby/v2/play-again` reproduz a
//! volta ao lobby gravada, e `POST /lol-end-of-game/v1/state/dismiss-stats`
//! (ou `DELETE /lol-lobby/v2/lobby`) sai do pós-jogo para o início. `POST /lol-honor/v1/honor` honra um jogador da cédula.
//!
//! O roteiro sai da própria gravação ([`Script::from_fixtures`]): nada de
//! nomes de tela ou índices fixos, então uma gravação nova de pós-jogo serve
//! do mesmo jeito.

use std::collections::VecDeque;

use easylobby_agent::end_of_game::DISMISS_STATS_PATH;
use easylobby_agent::fixtures::{EventFixture, FixtureSet};
use easylobby_agent::honor::{BALLOT_PATH, HONOR_PATH, HONOR_TYPE};
use easylobby_agent::lobby::{PLAY_AGAIN_DECLINE_PATH, PLAY_AGAIN_PATH};
use easylobby_agent::pre_end_of_game::{COMPLETE_PREFIX, CURRENT_SEQUENCE_EVENT_PATH};
use serde_json::{json, Value};

use crate::state::{LcuState, Response, GAMEFLOW_PHASE};

/// Rota só do mock: volta para a partida (`InProgress`) e acaba ela agora.
pub const END_GAME: &str = "/mock-lcu/end-of-game";

/// Rota só do mock: conclui a tela do `PreEndOfGame` que está esperando,
/// como a UI oficial faria.
pub const ADVANCE: &str = "/mock-lcu/end-of-game/advance";

/// Rota só do mock: volta para a partida (`InProgress`) sem acabar ela.
pub const RESTART: &str = "/mock-lcu/end-of-game/restart";

/// O "jogar de novo" da LCU.
pub const PLAY_AGAIN: &str = PLAY_AGAIN_PATH;

/// O "dispensar estatísticas" da LCU: sai do pós-jogo para o início (#180).
pub const DISMISS_STATS: &str = DISMISS_STATS_PATH;

/// A recusa do "jogar de novo" do grupo (#180). O mock só responde 204.
pub const PLAY_AGAIN_DECLINE: &str = PLAY_AGAIN_DECLINE_PATH;

/// O voto de honra da LCU.
pub const HONOR: &str = HONOR_PATH;

/// Cédula de honra.
pub const BALLOT: &str = BALLOT_PATH;

const SEQUENCE: &str = CURRENT_SEQUENCE_EVENT_PATH;
const SESSION: &str = "/lol-gameflow/v1/session";
const EOG_STATUS: &str = "/lol-lobby/v2/party/eog-status";
const EOG_STATS: &str = "/lol-end-of-game/v1/eog-stats-block";
const SEARCH: &str = "/lol-matchmaking/v1/search";
const LOBBY: &str = "/lol-lobby/v2/lobby";
const CHAMP_SELECT: &str = "/lol-champ-select/v1/session";

/// Maior pausa entre dois eventos do roteiro, em milissegundos. Na
/// gravação há minutos de chat entre o `EndOfGame` e o "jogar de novo".
const MAX_GAP_MS: u64 = 1_000;

/// Opções do fim de partida simulado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Acaba a partida sozinho este tanto de milissegundos depois de um
    /// cliente abrir o WebSocket WAMP. `None` espera [`END_GAME`] (ou a
    /// tecla `f` no terminal).
    pub end_after_ms: Option<u64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            end_after_ms: Some(3_000),
        }
    }
}

/// Um pedaço do fim de partida que sai de uma vez, e a tela que ele deixa
/// esperando.
#[derive(Debug, Clone, PartialEq)]
struct Chunk {
    events: Vec<EventFixture>,
    /// Nome da tela do `PreEndOfGame` que fica esperando o `complete`
    /// depois deste pedaço. `None` no último, que vai até o `EndOfGame`.
    waits_for: Option<String>,
}

/// O fim de partida tirado de uma gravação.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// Da gravação até a partida em andamento, aplicado sem mandar nada.
    game: Vec<EventFixture>,
    /// Do fim da partida até o `EndOfGame` (e o que vem depois dele, até o
    /// "jogar de novo"), cortado nas telas do `PreEndOfGame`.
    chunks: Vec<Chunk>,
    /// Do "jogar de novo" até o lobby, antes de entrar na fila de novo.
    play_again: Vec<EventFixture>,
}

impl Script {
    /// Monta o roteiro a partir de uma gravação com uma partida inteira e o
    /// pós-jogo, como `16.22-pos-jogo`. `None` se faltar alguma parte.
    pub fn from_fixtures(fixtures: &FixtureSet) -> Option<Self> {
        let events = &fixtures.events;
        let phase_at = |i: usize| -> Option<&str> {
            let e = &events[i];
            (e.uri == GAMEFLOW_PHASE).then(|| e.data.as_str()).flatten()
        };
        let find_phase = |from: usize, wanted: &dyn Fn(&str) -> bool| {
            (from..events.len()).find(|&i| phase_at(i).is_some_and(wanted))
        };

        let in_progress = find_phase(0, &|p| p == "InProgress")?;
        // A primeira troca de fase depois da partida: `WaitingForStats` ou,
        // como em 16.22-pos-jogo, um `Reconnect` antes dele.
        let game_over = find_phase(in_progress + 1, &|p| p != "InProgress")?;
        let end_of_game = find_phase(game_over, &|p| p == "EndOfGame")?;
        let lobby = find_phase(end_of_game + 1, &|p| p != "EndOfGame")?;
        if phase_at(lobby) != Some("Lobby") {
            return None;
        }
        // O "jogar de novo" começa quando o jogador fica pronto no
        // `eog-status`, logo antes de a cédula e as estatísticas sumirem.
        let play_again = (end_of_game..lobby)
            .find(|&i| events[i].uri == EOG_STATUS)
            .or_else(|| {
                (end_of_game..lobby)
                    .find(|&i| events[i].uri == EOG_STATS && events[i].event_type == "Delete")
            })
            .unwrap_or(lobby);
        // Acaba antes de entrar na fila de novo.
        let back_in_lobby = (lobby + 1..events.len())
            .find(|&i| events[i].uri == SEARCH || phase_at(i).is_some())
            .unwrap_or(events.len());

        let mut chunks: Vec<Chunk> = Vec::new();
        let mut current = Chunk {
            events: Vec::new(),
            waits_for: None,
        };
        let mut has_screen = false;
        for event in &events[game_over..play_again] {
            // O `Reconnect` da gravação foi o jogo fechando devagar; não
            // faz parte do fim de partida.
            if event.uri == GAMEFLOW_PHASE && event.data == "Reconnect" {
                continue;
            }
            if event.uri == SEQUENCE {
                if has_screen {
                    chunks.push(std::mem::replace(
                        &mut current,
                        Chunk {
                            events: Vec::new(),
                            waits_for: None,
                        },
                    ));
                }
                has_screen = true;
                current.waits_for = screen_name(&event.data);
            }
            current.events.push(event.clone());
        }
        chunks.push(current);
        // Sem tela nenhuma o último pedaço não espera nada; com telas, só o
        // último (o da sequência vazia) segue direto.
        if let Some(last) = chunks.last_mut() {
            if last.waits_for.is_some() {
                // A gravação acabou a sequência numa tela: o mock não sabe
                // quando a fase muda.
                return None;
            }
        }
        if !chunks
            .iter()
            .flat_map(|c| &c.events)
            .any(|e| e.uri == GAMEFLOW_PHASE && e.data == "EndOfGame")
        {
            return None;
        }

        Some(Self {
            game: events[..game_over].to_vec(),
            chunks,
            play_again: events[play_again..back_in_lobby].to_vec(),
        })
    }

    /// Nomes das telas do `PreEndOfGame`, na ordem da gravação.
    pub fn screens(&self) -> Vec<String> {
        self.chunks
            .iter()
            .filter_map(|c| c.waits_for.clone())
            .collect()
    }
}

fn screen_name(data: &Value) -> Option<String> {
    let name = data["name"].as_str().unwrap_or("").trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Em que ponto o fim de partida simulado está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Partida em andamento, esperando acabar.
    InGame,
    /// A partida acabou: o fim de partida está saindo (ou esperando uma
    /// tela). `next` é o próximo pedaço do roteiro.
    PostGame { next: usize },
    /// O jogador voltou ao lobby ou saiu dele.
    Over,
}

/// O fim de partida simulado em andamento. O relógio é em milissegundos,
/// num marco qualquer que só precisa andar para a frente.
#[derive(Debug, Clone)]
pub struct PostGame {
    script: Script,
    options: Options,
    speed: f64,
    stage: Stage,
    /// Tela do `PreEndOfGame` esperando o `complete`: a última que já saiu
    /// pelo WAMP.
    waiting: Option<String>,
    /// Eventos a mandar e quando.
    queue: VecDeque<(u64, EventFixture)>,
    /// Quando a partida acaba sozinha, depois que alguém conectou.
    end_at: Option<u64>,
}

/// O que um passo do fim de partida produziu.
#[derive(Debug, Default)]
pub struct Step {
    /// Eventos a aplicar e mandar pelo WAMP, em ordem.
    pub events: Vec<EventFixture>,
    /// Linhas para o terminal do mock.
    pub log: Vec<String>,
}

impl PostGame {
    /// Prepara o fim de partida. Chame [`PostGame::restart`] em seguida para
    /// pôr o estado na partida.
    pub fn new(script: Script, options: Options, speed: f64) -> Self {
        Self {
            script,
            options,
            speed: if speed.is_finite() && speed > 0.0 {
                speed
            } else {
                1.0
            },
            stage: Stage::InGame,
            waiting: None,
            queue: VecDeque::new(),
            end_at: None,
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Tela do `PreEndOfGame` esperando o `complete`, se houver.
    pub fn waiting(&self) -> Option<&str> {
        self.waiting.as_deref()
    }

    /// Se ainda há eventos do roteiro para sair.
    pub fn pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Volta o estado para a partida em andamento da gravação. Os eventos
    /// devolvidos já foram aplicados; só falta mandar pelo WAMP, para quem
    /// está conectado ver a volta.
    pub fn restart(&mut self, state: &mut LcuState) -> Step {
        state.reset();
        for event in &self.script.game {
            state.apply(event);
        }
        self.stage = Stage::InGame;
        self.waiting = None;
        self.queue.clear();
        self.end_at = None;
        // A fase antes dos outros: o agente relê o estado na troca de fase.
        let mut events = vec![event(GAMEFLOW_PHASE, "Update", json!(state.phase()))];
        for path in [
            SESSION,
            LOBBY,
            SEARCH,
            CHAMP_SELECT,
            EOG_STATS,
            BALLOT,
            SEQUENCE,
        ] {
            let current = state.get(path);
            events.push(if current.status == 200 {
                event(path, "Update", current.body)
            } else {
                event(path, "Delete", Value::Null)
            });
        }
        Step {
            events,
            log: vec!["partida em andamento (fase InProgress)".into()],
        }
    }

    /// Acaba a partida agora. `false` se ela já acabou.
    pub fn end_game(&mut self, now: u64) -> bool {
        if self.stage != Stage::InGame {
            return false;
        }
        self.stage = Stage::PostGame { next: 0 };
        self.end_at = None;
        self.enqueue_next(now);
        true
    }

    /// `POST /lol-pre-end-of-game/v1/complete/{name}`: a tela `name`
    /// acabou. Com outra tela esperando (ou nenhuma) não muda nada, e a LCU
    /// responde 204 do mesmo jeito (suposição do mock).
    pub fn complete(&mut self, name: &str, now: u64) -> Step {
        if self.waiting.as_deref() != Some(name) {
            return Step::default();
        }
        self.waiting = None;
        self.enqueue_next(now);
        Step {
            events: Vec::new(),
            log: vec![format!("pós-jogo: tela {name} concluída")],
        }
    }

    /// Conclui a tela esperando, como [`ADVANCE`]. `false` sem tela.
    pub fn advance(&mut self, now: u64) -> Option<Step> {
        let name = self.waiting.clone()?;
        Some(self.complete(&name, now))
    }

    /// `POST /lol-lobby/v2/play-again`: do fim de partida de volta ao lobby,
    /// pelos eventos gravados. O que faltava do fim de partida não sai mais.
    pub fn play_again(&mut self, now: u64) -> Result<Step, Response> {
        if !matches!(self.stage, Stage::PostGame { .. }) {
            return Err(Response::error(400, "mock-lcu: fora do pós-jogo"));
        }
        self.queue.clear();
        let mut step = Step {
            events: Vec::new(),
            log: vec!["pós-jogo: jogar de novo, voltando ao lobby".into()],
        };
        if self.waiting.take().is_some() {
            step.events.push(event(
                SEQUENCE,
                "Update",
                json!({"name": "", "priority": -1}),
            ));
        }
        self.stage = Stage::Over;
        self.schedule(&self.script.play_again.clone(), now);
        Ok(step)
    }

    /// `DELETE /lol-lobby/v2/lobby` no pós-jogo: a cédula e as estatísticas
    /// somem e a fase vai para `None`. Os eventos do lobby em si vêm de
    /// [`crate::lobby::leave`]. A ordem é suposição do mock.
    pub fn leave(&mut self, state: &LcuState) -> Step {
        self.queue.clear();
        self.stage = Stage::Over;
        let mut events = Vec::new();
        if self.waiting.take().is_some() {
            events.push(event(
                SEQUENCE,
                "Update",
                json!({"name": "", "priority": -1}),
            ));
        }
        for path in [EOG_STATS, BALLOT] {
            if state.get(path).status == 200 {
                events.push(event(path, "Delete", Value::Null));
            }
        }
        Step {
            events,
            log: vec!["pós-jogo: saiu do lobby".into()],
        }
    }

    /// `POST /lol-end-of-game/v1/state/dismiss-stats` (#180): sai do
    /// pós-jogo para o início, sem lobby. A cédula e as estatísticas somem e
    /// a fase vai para `None`. A ordem dos eventos é suposição do mock.
    pub fn dismiss(&mut self, state: &LcuState) -> Result<Step, Response> {
        if !matches!(self.stage, Stage::PostGame { .. }) {
            return Err(Response::error(400, "mock-lcu: fora do pós-jogo"));
        }
        let mut step = self.leave(state);
        if state.phase() != "None" {
            step.events
                .push(event(GAMEFLOW_PHASE, "Update", json!("None")));
        }
        step.log = vec!["pós-jogo: dispensou as estatísticas, voltando ao início".into()];
        Ok(step)
    }

    /// Anda o relógio: acaba a partida na hora marcada e solta os eventos
    /// do roteiro que já venceram. `connected` diz se algum cliente está com
    /// o WAMP aberto, para a partida acabar sozinha só com alguém olhando.
    pub fn tick(&mut self, now: u64, connected: bool) -> Step {
        let mut step = Step::default();
        if self.stage == Stage::InGame {
            if let (None, Some(delay), true) = (self.end_at, self.options.end_after_ms, connected) {
                self.end_at = Some(now + delay);
            }
            if self.end_at.is_some_and(|at| now >= at) {
                self.end_game(now);
            }
        }
        while self.queue.front().is_some_and(|(at, _)| *at <= now) {
            let Some((_, event)) = self.queue.pop_front() else {
                break;
            };
            if event.uri == SEQUENCE {
                self.waiting = screen_name(&event.data);
            }
            log_event(&event, &mut step.log);
            step.events.push(event);
        }
        step
    }

    /// Põe na fila o próximo pedaço do roteiro e marca a tela que ele deixa
    /// esperando.
    fn enqueue_next(&mut self, now: u64) {
        let Stage::PostGame { next } = self.stage else {
            return;
        };
        let Some(chunk) = self.script.chunks.get(next).cloned() else {
            return;
        };
        self.stage = Stage::PostGame { next: next + 1 };
        self.schedule(&chunk.events, now);
    }

    /// Agenda `events` depois do que já está na fila, com as pausas da
    /// gravação (no máximo [`MAX_GAP_MS`], divididas pela velocidade).
    fn schedule(&mut self, events: &[EventFixture], now: u64) {
        let mut at = self.queue.back().map_or(now, |(t, _)| (*t).max(now));
        let mut previous: Option<u64> = None;
        for event in events {
            if let Some(previous) = previous {
                let gap = event.t_ms.saturating_sub(previous).min(MAX_GAP_MS);
                at += (gap as f64 / self.speed) as u64;
            }
            previous = Some(event.t_ms);
            self.queue.push_back((at, event.clone()));
        }
    }
}

fn log_event(event: &EventFixture, log: &mut Vec<String>) {
    if event.uri == GAMEFLOW_PHASE {
        if let Some(phase) = event.data.as_str() {
            log.push(format!("pós-jogo: fase {phase}"));
        }
    } else if event.uri == SEQUENCE {
        match screen_name(&event.data) {
            Some(name) => log.push(format!(
                "pós-jogo: tela {name} esperando (a avança, ou o complete do agente)"
            )),
            None => log.push("pós-jogo: fim das telas".into()),
        }
    }
}

/// A cédula do roteiro chegando por cima de uma com votos da mesma partida
/// (o agente lê a cédula gravada pelo REST antes do evento dela): mantém os
/// votos, como a LCU faria.
pub fn keep_votes(state: &LcuState, event: &mut EventFixture) {
    if event.uri != BALLOT || event.event_type == "Delete" {
        return;
    }
    let current = state.get(BALLOT);
    let honored = &current.body["honoredPlayers"];
    let has_votes = honored.as_array().is_some_and(|h| !h.is_empty());
    if current.status == 200 && has_votes && current.body["gameId"] == event.data["gameId"] {
        event.data["honoredPlayers"] = honored.clone();
    }
}

/// Nome da tela em `POST /lol-pre-end-of-game/v1/complete/{nome}`.
pub fn complete_route(path: &str) -> Option<String> {
    let name = path.strip_prefix(COMPLETE_PREFIX)?;
    (!name.is_empty() && !name.contains('/')).then(|| percent_decode(name))
}

fn percent_decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&segment[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `POST /lol-honor/v1/honor` com `{"honorType": "HEART", "recipientPuuid":
/// "..."}`: põe o jogador em `honoredPlayers` da cédula aberta e manda a
/// cédula pelo WAMP. O `votePool` fica como está (não se sabe se a LCU
/// desconta; o agente aceita os dois jeitos). Sem cédula aberta, 404; quem
/// não está na cédula, já honrado ou sem honra sobrando, 400. Os erros são
/// suposições do mock.
pub fn vote(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let current = state.get(BALLOT);
    let open = current.status == 200
        && current.body["gameId"].as_u64().unwrap_or(0) != 0
        && current.body["eligibleAllies"]
            .as_array()
            .is_some_and(|a| !a.is_empty());
    if !open {
        return (
            Response::error(404, "mock-lcu: nenhuma cédula de honra aberta"),
            Vec::new(),
        );
    }
    let mut ballot = current.body;
    let Some(puuid) = body["recipientPuuid"].as_str().filter(|p| !p.is_empty()) else {
        return (
            Response::error(400, "mock-lcu: falta recipientPuuid"),
            Vec::new(),
        );
    };
    let eligible = ["eligibleAllies", "eligibleOpponents"]
        .iter()
        .filter_map(|k| ballot[*k].as_array())
        .flatten()
        .any(|c| c["puuid"] == puuid);
    if !eligible {
        return (
            Response::error(400, "mock-lcu: jogador fora da cédula"),
            Vec::new(),
        );
    }
    let honored = ballot["honoredPlayers"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if honored.iter().any(|h| h["recipientPuuid"] == puuid) {
        return (
            Response::error(400, "mock-lcu: jogador já honrado"),
            Vec::new(),
        );
    }
    let votes = ballot["votePool"]["votes"].as_u64().unwrap_or(1);
    if honored.len() as u64 >= votes {
        return (
            Response::error(400, "mock-lcu: sem honras sobrando"),
            Vec::new(),
        );
    }
    let honor_type = body["honorType"].as_str().unwrap_or(HONOR_TYPE);
    let mut honored = honored;
    honored.push(json!({"honorType": honor_type, "recipientPuuid": puuid}));
    ballot["honoredPlayers"] = Value::Array(honored);
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        vec![event(BALLOT, "Update", ballot)],
    )
}

fn event(uri: &str, event_type: &str, data: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.into(),
        event_type: event_type.into(),
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures(name: &str) -> FixtureSet {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../agent/tests/fixtures")
            .join(name);
        FixtureSet::load(&dir).unwrap()
    }

    fn start() -> (LcuState, PostGame) {
        let set = fixtures("16.22-pos-jogo");
        let script = Script::from_fixtures(&set).expect("a gravação tem pós-jogo");
        let mut state = LcuState::new(&set);
        let mut post_game = PostGame::new(script, Options { end_after_ms: None }, 1.0);
        let step = post_game.restart(&mut state);
        apply(&mut state, &step);
        (state, post_game)
    }

    fn apply(state: &mut LcuState, step: &Step) {
        for e in &step.events {
            state.apply(e);
        }
    }

    /// Anda o relógio até `until`, de 100 em 100 ms.
    fn run(state: &mut LcuState, post_game: &mut PostGame, from: u64, until: u64) {
        let mut now = from;
        while now <= until {
            let step = post_game.tick(now, true);
            apply(state, &step);
            now += 100;
        }
    }

    #[test]
    fn roteiro_da_gravacao() {
        let script = Script::from_fixtures(&fixtures("16.22-pos-jogo")).unwrap();
        assert_eq!(
            script.screens(),
            [
                "honor-vote",
                "challenge-level-up-celebration",
                "missions-celebration",
                "ranked-celebration",
            ]
        );
        assert!(!script
            .chunks
            .iter()
            .flat_map(|c| &c.events)
            .any(|e| e.data == "Reconnect"));
        assert_eq!(script.play_again.first().unwrap().uri, EOG_STATUS);
        assert!(script
            .play_again
            .iter()
            .any(|e| e.uri == GAMEFLOW_PHASE && e.data == "Lobby"));
        assert!(!script.play_again.iter().any(|e| e.uri == SEARCH));
    }

    #[test]
    fn gravacao_sem_pos_jogo() {
        assert!(Script::from_fixtures(&fixtures("16.22")).is_none());
        assert!(Script::from_fixtures(&fixtures("exemplo")).is_none());
    }

    #[test]
    fn da_partida_ao_lobby_pelo_jogar_de_novo() {
        let (mut state, mut pg) = start();
        assert_eq!(state.phase(), "InProgress");
        assert_eq!(state.get(EOG_STATS).status, 404);
        // Sem `end_after_ms`, a partida não acaba sozinha.
        run(&mut state, &mut pg, 0, 10_000);
        assert_eq!(state.phase(), "InProgress");

        assert!(pg.end_game(10_000));
        assert!(!pg.end_game(10_000));
        run(&mut state, &mut pg, 10_000, 30_000);
        assert_eq!(state.phase(), "PreEndOfGame");
        assert_eq!(pg.waiting(), Some("honor-vote"));
        assert_eq!(state.get(SEQUENCE).body["name"], "honor-vote");
        assert_eq!(state.get(EOG_STATS).status, 200);
        assert_eq!(state.get(BALLOT).body["gameId"], 1124);

        // Concluir outra tela não muda nada.
        assert!(pg.complete("ranked-celebration", 30_000).log.is_empty());
        run(&mut state, &mut pg, 30_000, 40_000);
        assert_eq!(pg.waiting(), Some("honor-vote"));

        let mut now = 40_000;
        for screen in [
            "honor-vote",
            "challenge-level-up-celebration",
            "missions-celebration",
            "ranked-celebration",
        ] {
            assert_eq!(pg.waiting(), Some(screen));
            assert_eq!(state.phase(), "PreEndOfGame");
            pg.complete(screen, now);
            run(&mut state, &mut pg, now, now + 5_000);
            now += 5_000;
        }
        assert_eq!(pg.waiting(), None);
        assert_eq!(state.phase(), "EndOfGame");
        assert_eq!(state.get(EOG_STATS).status, 200);

        let step = pg.play_again(now).unwrap();
        apply(&mut state, &step);
        run(&mut state, &mut pg, now, now + 60_000);
        assert_eq!(state.phase(), "Lobby");
        assert_eq!(state.get(LOBBY).status, 200);
        assert_eq!(state.get(EOG_STATS).status, 404);
        // No lobby a gravação tem a cédula vazia, de fora do pós-jogo.
        assert_eq!(state.get(BALLOT).body["gameId"], 0);
        assert_eq!(pg.stage(), Stage::Over);
        assert!(pg.play_again(now).is_err());
    }

    #[test]
    fn acaba_sozinha_depois_que_alguem_conecta() {
        let set = fixtures("16.22-pos-jogo");
        let script = Script::from_fixtures(&set).unwrap();
        let mut state = LcuState::new(&set);
        let mut pg = PostGame::new(
            script,
            Options {
                end_after_ms: Some(2_000),
            },
            1.0,
        );
        pg.restart(&mut state);
        // Ninguém conectado: continua na partida.
        for now in (0..10_000).step_by(100) {
            apply(&mut state, &pg.tick(now, false));
        }
        assert_eq!(pg.stage(), Stage::InGame);
        apply(&mut state, &pg.tick(10_000, true));
        assert_eq!(pg.stage(), Stage::InGame);
        apply(&mut state, &pg.tick(12_000, true));
        assert_eq!(pg.stage(), Stage::PostGame { next: 1 });
    }

    #[test]
    fn jogar_de_novo_no_meio_das_telas() {
        let (mut state, mut pg) = start();
        assert!(pg.play_again(0).is_err());
        pg.end_game(0);
        run(&mut state, &mut pg, 0, 20_000);
        assert_eq!(pg.waiting(), Some("honor-vote"));
        let step = pg.play_again(20_000).unwrap();
        apply(&mut state, &step);
        run(&mut state, &mut pg, 20_000, 80_000);
        assert_eq!(state.phase(), "Lobby");
        assert_eq!(state.get(SEQUENCE).body["name"], "");
        assert_eq!(pg.waiting(), None);
    }

    #[test]
    fn sair_no_pos_jogo() {
        let (mut state, mut pg) = start();
        pg.end_game(0);
        run(&mut state, &mut pg, 0, 20_000);
        let step = pg.leave(&state);
        apply(&mut state, &step);
        let (_, events) = crate::lobby::leave(&state);
        for e in &events {
            state.apply(e);
        }
        assert_eq!(state.phase(), "None");
        assert_eq!(state.get(EOG_STATS).status, 404);
        assert_eq!(state.get(BALLOT).status, 404);
        // Nada mais do roteiro sai.
        run(&mut state, &mut pg, 20_000, 80_000);
        assert_eq!(state.phase(), "None");
    }

    #[test]
    fn dispensar_as_estatisticas() {
        let (mut state, mut pg) = start();
        assert!(pg.dismiss(&state).is_err(), "ainda na partida");
        pg.end_game(0);
        run(&mut state, &mut pg, 0, 20_000);
        let step = pg.dismiss(&state).unwrap();
        apply(&mut state, &step);
        assert_eq!(state.phase(), "None");
        assert_eq!(state.get(EOG_STATS).status, 404);
        assert_eq!(state.get(BALLOT).status, 404);
        assert_eq!(state.get(SEQUENCE).body["name"], "");
        // Nada mais do roteiro sai, e não dá para dispensar de novo.
        run(&mut state, &mut pg, 20_000, 80_000);
        assert_eq!(state.phase(), "None");
        assert!(pg.dismiss(&state).is_err());
    }

    #[test]
    fn recomecar_volta_para_a_partida() {
        let (mut state, mut pg) = start();
        pg.end_game(0);
        run(&mut state, &mut pg, 0, 20_000);
        let step = pg.restart(&mut state);
        assert_eq!(step.events[0].data, "InProgress");
        assert!(step
            .events
            .iter()
            .any(|e| e.uri == EOG_STATS && e.event_type == "Delete"));
        apply(&mut state, &step);
        assert_eq!(state.phase(), "InProgress");
        assert_eq!(state.get(EOG_STATS).status, 404);
        assert_eq!(pg.stage(), Stage::InGame);
        assert!(!pg.pending());
    }

    #[test]
    fn honra_varios_jogadores() {
        let (mut state, mut pg) = start();
        let body = |puuid: &str| json!({"honorType": "HEART", "recipientPuuid": puuid});
        assert_eq!(vote(&state, &body("fake-puuid-0092")).0.status, 404);

        pg.end_game(0);
        run(&mut state, &mut pg, 0, 20_000);
        let ballot = state.get(BALLOT).body;
        assert_eq!(ballot["votePool"]["votes"], 4);

        let (response, events) = vote(&state, &body("fake-puuid-0092"));
        assert_eq!(response.status, 204);
        for e in &events {
            state.apply(e);
        }
        state.pin(BALLOT);
        // Um adversário também.
        let (response, events) = vote(&state, &body("fake-puuid-0091"));
        assert_eq!(response.status, 204);
        for e in &events {
            state.apply(e);
        }
        let honored = state.get(BALLOT).body["honoredPlayers"].clone();
        assert_eq!(
            honored,
            json!([
                {"honorType": "HEART", "recipientPuuid": "fake-puuid-0092"},
                {"honorType": "HEART", "recipientPuuid": "fake-puuid-0091"},
            ])
        );
        assert_eq!(vote(&state, &body("fake-puuid-0092")).0.status, 400);
        assert_eq!(vote(&state, &body("fake-puuid-9999")).0.status, 400);
        assert_eq!(vote(&state, &json!({})).0.status, 400);
        for puuid in ["fake-puuid-0093", "fake-puuid-0095"] {
            let (_, events) = vote(&state, &body(puuid));
            for e in &events {
                state.apply(e);
            }
        }
        // As 4 honras foram usadas.
        assert_eq!(vote(&state, &body("fake-puuid-0094")).0.status, 400);

        // A troca de fase não apaga os votos.
        let mut now = 20_000;
        while let Some(screen) = pg.waiting().map(str::to_owned) {
            pg.complete(&screen, now);
            run(&mut state, &mut pg, now, now + 5_000);
            now += 5_000;
        }
        assert_eq!(state.phase(), "EndOfGame");
        assert_eq!(
            state.get(BALLOT).body["honoredPlayers"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn voto_antes_da_cedula_do_roteiro_chegar() {
        let (mut state, mut pg) = start();
        pg.end_game(0);
        // Só até a fase `PreEndOfGame`: a cédula vem da resposta gravada.
        let mut now = 0;
        while state.phase() != "PreEndOfGame" {
            apply(&mut state, &pg.tick(now, true));
            now += 50;
        }
        let (response, events) = vote(
            &state,
            &json!({"honorType": "HEART", "recipientPuuid": "fake-puuid-0092"}),
        );
        assert_eq!(response.status, 204);
        for e in &events {
            state.apply(e);
        }
        // O resto do roteiro, com a cédula gravada por cima.
        while now < 20_000 {
            let mut step = pg.tick(now, true);
            for e in &mut step.events {
                keep_votes(&state, e);
                state.apply(e);
            }
            now += 50;
        }
        assert_eq!(
            state.get(BALLOT).body["honoredPlayers"][0]["recipientPuuid"],
            "fake-puuid-0092"
        );
    }

    #[test]
    fn so_conclui_a_tela_que_ja_apareceu() {
        let (mut state, mut pg) = start();
        pg.end_game(0);
        // Nada saiu ainda: a honor-vote não está esperando.
        assert!(pg.complete("honor-vote", 0).log.is_empty());
        run(&mut state, &mut pg, 0, 20_000);
        assert_eq!(pg.waiting(), Some("honor-vote"));
        assert!(!pg.complete("honor-vote", 20_000).log.is_empty());
        assert_eq!(pg.waiting(), None);
    }

    #[test]
    fn nome_da_tela_no_caminho() {
        assert_eq!(
            complete_route("/lol-pre-end-of-game/v1/complete/honor-vote").as_deref(),
            Some("honor-vote")
        );
        assert_eq!(
            complete_route("/lol-pre-end-of-game/v1/complete/a%20b").as_deref(),
            Some("a b")
        );
        assert_eq!(complete_route("/lol-pre-end-of-game/v1/complete/"), None);
        assert_eq!(complete_route("/lol-pre-end-of-game/v1/outra"), None);
    }
}
