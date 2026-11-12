//! Seleção de campeões controlável na LCU falsa (#440).
//!
//! Para testar as telas da seleção sem entrar em fila no LoL real. O mock
//! monta uma seleção a partir da sessão gravada e faz o draft andar sozinho,
//! como o LoL: planejamento, bans, revelação dos bans, picks, finalização e o
//! início da partida. Os outros 9 jogadores são bots: marcam e confirmam
//! campeões alguns segundos depois de a vez deles abrir (os aliados mostram o
//! hover antes de confirmar).
//!
//! Modos ([`Mode`]):
//!
//! - `draft`: ranqueada Solo/Duo (fila 420), 10 bans simultâneos e picks
//!   1-2-2-2-2-1, a mesma ordem da gravação 16.22.
//! - `torneio`: draft de torneio da personalizada (e do Clash), com os bans
//!   alternados um por vez em duas fases: B1 R1 B2 R2 B3 R3, picks B1 | R1
//!   R2 | B2 B3 | R3, bans R4 B4 R5 B5 e picks R4 | B4 B5 | R5.
//! - `blind`: escolha às cegas (fila 430), os 5 aliados escolhem ao mesmo
//!   tempo, sem bans e sem ver os adversários.
//! - `aram`: ARAM (fila 450), campeões sorteados, banco e reroll.
//!
//! Controle (rotas só do mock, ou as teclas do terminal do `--selecao`):
//!
//! - `POST /mock-lcu/champ-select` com `{"mode": "draft", "cell": 0}` começa
//!   (ou recomeça) uma seleção. `cell` é a vez do jogador na ordem de picks.
//! - `POST /mock-lcu/champ-select/pause` congela o tempo: o timer fica
//!   `isInfinite` e os bots param. `.../resume` volta com o tempo que faltava.
//! - `POST /mock-lcu/champ-select/advance` acaba a etapa atual na hora: os
//!   bots que faltam agem, e a próxima etapa começa (ainda pausada, se
//!   estava).
//! - `DELETE /mock-lcu/champ-select` é o dodge: a seleção some e a fase
//!   volta para `Lobby` (ou `None`, sem lobby).
//!
//! Tempo esgotado numa vez: o ban do jogador sem campeão fica vazio; o pick
//! com hover confirma o hover; o pick sem hover é dodge. No `advance`, o pick
//! sem hover recebe um campeão qualquer, para dar para seguir.
//!
//! O formato segue a gravação 16.22; o ARAM e a escolha às cegas são
//! suposições do mock, ainda não conferidas no LoL real.
//!
//! Como nos outros módulos, as funções daqui não emitem nada: devolvem os
//! eventos, e quem chama aplica no estado e manda pelo WAMP.

use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use easylobby_agent::fixtures::{EventFixture, FixtureSet};
use serde_json::{json, Value};

use crate::champ_select::SESSION;
use crate::champions;
use crate::lobby::LOBBY;
use crate::state::{LcuState, Response, GAMEFLOW_PHASE};

/// Começa (`POST`) ou desiste (`DELETE`, dodge) da seleção simulada.
pub const START: &str = "/mock-lcu/champ-select";
/// Congela o tempo.
pub const PAUSE: &str = "/mock-lcu/champ-select/pause";
/// Volta o tempo.
pub const RESUME: &str = "/mock-lcu/champ-select/resume";
/// Acaba a etapa atual na hora.
pub const ADVANCE: &str = "/mock-lcu/champ-select/advance";

/// Tipo de seleção simulada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Draft,
    Tournament,
    Blind,
    Aram,
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "draft" | "ranqueada" => Ok(Self::Draft),
            "torneio" | "tournament" => Ok(Self::Tournament),
            "blind" | "cegas" | "escolha-as-cegas" => Ok(Self::Blind),
            "aram" => Ok(Self::Aram),
            _ => Err(format!(
                "modo de seleção desconhecido: {s} (use draft, torneio, blind ou aram)"
            )),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Draft => "draft",
            Self::Tournament => "torneio",
            Self::Blind => "blind",
            Self::Aram => "aram",
        })
    }
}

/// Como começar a seleção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
    /// Vez do jogador na ordem de picks (0 a 4). `None` mantém a da
    /// gravação.
    pub cell: Option<u64>,
}

impl Options {
    /// Lê `{"mode": "draft", "cell": 0}`; sem `mode`, `draft`.
    pub fn from_json(body: &Value) -> Result<Self, String> {
        let mode = match body["mode"].as_str() {
            Some(mode) => mode.parse()?,
            None => Mode::Draft,
        };
        let cell = match &body["cell"] {
            Value::Null => None,
            cell => Some(
                cell.as_u64()
                    .filter(|c| *c < 5)
                    .ok_or("cell vai de 0 a 4")?,
            ),
        };
        Ok(Self { mode, cell })
    }
}

/// Duração de cada etapa e dos bots, em milissegundos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    pub planning: u64,
    pub ban: u64,
    pub reveal: u64,
    pub pick: u64,
    pub finalization: u64,
    /// Quanto um bot demora para confirmar depois de a vez abrir.
    pub bot: u64,
}

impl Default for Timings {
    /// Os tempos da gravação 16.22.
    fn default() -> Self {
        Self {
            planning: 12_000,
            ban: 25_000,
            reveal: 3_000,
            pick: 25_000,
            finalization: 30_000,
            bot: 3_000,
        }
    }
}

/// A primeira sessão da seleção gravada no conjunto, para servir de molde.
pub fn template(fixtures: &FixtureSet) -> Option<Value> {
    let from_events = fixtures
        .events
        .iter()
        .find(|e| e.uri == SESSION && e.data.is_object())
        .map(|e| e.data.clone());
    from_events.or_else(|| {
        fixtures
            .rest
            .iter()
            .find(|r| r.path == SESSION && r.status == 200 && r.body.is_object())
            .map(|r| r.body.clone())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Planning,
    /// Grupo de ações em andamento (índice em `actions`).
    Group(usize),
    Finalization,
}

/// O que um passo da seleção produziu.
#[derive(Debug, Default)]
pub struct Step {
    pub events: Vec<EventFixture>,
    /// Linhas para o terminal do mock.
    pub log: Vec<String>,
    /// A seleção acabou (partida iniciada, dodge ou a sessão sumiu).
    pub finished: bool,
}

/// Uma seleção simulada em andamento. O relógio é em milissegundos, num
/// marco qualquer que só precisa andar para a frente.
#[derive(Debug, Clone)]
pub struct Scenario {
    mode: Mode,
    timings: Timings,
    stage: Stage,
    started_at: u64,
    ends_at: u64,
    paused_at: Option<u64>,
    seed: u64,
}

impl Scenario {
    /// Começa uma seleção a partir do molde: a sessão nova e a fase
    /// `ChampSelect`.
    pub fn start(
        state: &LcuState,
        template: &Value,
        options: Options,
        timings: Timings,
        now: u64,
    ) -> (Self, Step) {
        let mut session = build_session(template, options);
        let mut scenario = Self {
            mode: options.mode,
            timings,
            stage: Stage::Planning,
            started_at: now,
            ends_at: now,
            paused_at: None,
            seed: 0x5eed,
        };
        let mut step = Step::default();
        match options.mode {
            Mode::Draft => {
                // Na gravação os bans já estão em andamento no planejamento.
                set_group_in_progress(&mut session, 0);
                scenario.enter(Stage::Planning, timings.planning, now);
            }
            Mode::Tournament => {
                set_group_in_progress(&mut session, 0);
                scenario.enter(Stage::Group(0), timings.ban, now);
            }
            Mode::Blind => {
                set_group_in_progress(&mut session, 0);
                scenario.enter(Stage::Group(0), timings.pick, now);
            }
            Mode::Aram => {
                scenario.roll_aram(state, &mut session);
                scenario.enter(Stage::Finalization, timings.finalization * 2, now);
            }
        }
        scenario.write_timer(&mut session, now);
        step.log.push(format!(
            "seleção {} começou; você é a célula {}. {}",
            options.mode,
            session["localPlayerCellId"],
            scenario.describe(&session)
        ));
        // A fase antes da sessão: trocar de fase volta as respostas gravadas
        // dela, e a sessão gravada taparia a nova.
        if state.phase() != "ChampSelect" {
            step.events
                .push(event(GAMEFLOW_PHASE, "Update", json!("ChampSelect")));
        }
        step.events.push(event(SESSION, "Create", session));
        (scenario, step)
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn paused(&self) -> bool {
        self.paused_at.is_some()
    }

    /// Anda o relógio até `now`: bots agem, etapas acabam.
    pub fn tick(&mut self, state: &LcuState, now: u64) -> Step {
        let Some(mut session) = self.current(state) else {
            return finished("a seleção simulada acabou fora do mock");
        };
        if self.paused() {
            return Step::default();
        }
        let before = session.clone();
        let mut step = Step::default();
        self.bots(state, &mut session, now);
        let group_done = matches!(self.stage, Stage::Group(g) if group_completed(&session, g));
        if group_done || now >= self.ends_at {
            if let Some(ended) = self.finish_stage(state, &mut session, now, false, &mut step) {
                return ended;
            }
        }
        if session != before {
            bump(&mut session);
            step.events.push(event(SESSION, "Update", session));
        }
        step
    }

    /// Congela o tempo.
    pub fn pause(&mut self, state: &LcuState, now: u64) -> Step {
        let Some(mut session) = self.current(state) else {
            return finished("sem seleção simulada");
        };
        if self.paused_at.is_none() {
            self.paused_at = Some(now);
        }
        self.write_timer(&mut session, now);
        bump(&mut session);
        Step {
            events: vec![event(SESSION, "Update", session)],
            log: vec!["tempo pausado".into()],
            finished: false,
        }
    }

    /// Volta o tempo de onde parou.
    pub fn resume(&mut self, state: &LcuState, now: u64) -> Step {
        let Some(mut session) = self.current(state) else {
            return finished("sem seleção simulada");
        };
        if let Some(paused_at) = self.paused_at.take() {
            let delta = now.saturating_sub(paused_at);
            self.started_at += delta;
            self.ends_at += delta;
        }
        self.write_timer(&mut session, now);
        bump(&mut session);
        let log = vec![format!("tempo continuando. {}", self.describe(&session))];
        Step {
            events: vec![event(SESSION, "Update", session)],
            log,
            finished: false,
        }
    }

    /// Acaba a etapa atual na hora.
    pub fn advance(&mut self, state: &LcuState, now: u64) -> Step {
        let Some(mut session) = self.current(state) else {
            return finished("sem seleção simulada");
        };
        let mut step = Step::default();
        if let Some(ended) = self.finish_stage(state, &mut session, now, true, &mut step) {
            return ended;
        }
        if self.paused_at.is_some() {
            self.paused_at = Some(now);
            self.write_timer(&mut session, now);
        }
        bump(&mut session);
        step.events.push(event(SESSION, "Update", session));
        step
    }

    /// Descrição da etapa atual, para o terminal.
    pub fn describe(&self, session: &Value) -> String {
        let stage = match self.stage {
            Stage::Planning => "planejamento".to_owned(),
            Stage::Finalization => "finalização".to_owned(),
            Stage::Group(g) => {
                let actions = group(session, g);
                let kind = actions
                    .first()
                    .and_then(|a| a["type"].as_str())
                    .unwrap_or("?");
                let mine = actions
                    .iter()
                    .any(|a| a["actorCellId"] == session["localPlayerCellId"]);
                let kind = match kind {
                    "ban" => "bans",
                    "pick" => "picks",
                    "ten_bans_reveal" => "revelação dos bans",
                    other => other,
                };
                if mine {
                    format!("{kind}, SUA VEZ")
                } else {
                    kind.to_owned()
                }
            }
        };
        let seconds = (self.ends_at.saturating_sub(self.started_at)) / 1000;
        format!("etapa: {stage} ({seconds}s)")
    }

    /// A sessão atual, se ainda é a seleção simulada.
    fn current(&self, state: &LcuState) -> Option<Value> {
        let session = state.get(SESSION);
        (session.status == 200 && state.phase() == "ChampSelect").then_some(session.body)
    }

    fn enter(&mut self, stage: Stage, duration: u64, now: u64) {
        self.stage = stage;
        self.started_at = now;
        self.ends_at = now + duration;
    }

    /// Fecha a etapa atual e abre a próxima. Devolve o passo final quando a
    /// seleção acaba (partida iniciada ou dodge).
    fn finish_stage(
        &mut self,
        state: &LcuState,
        session: &mut Value,
        now: u64,
        forced: bool,
        step: &mut Step,
    ) -> Option<Step> {
        let next = match self.stage {
            Stage::Planning => 0,
            Stage::Group(g) => {
                if self.expire_group(state, session, g, forced) {
                    let mut ended = dodge(state);
                    ended
                        .log
                        .insert(0, "tempo esgotado sem campeão: dodge".into());
                    return Some(ended);
                }
                set_group_done(session, g);
                g + 1
            }
            Stage::Finalization => return Some(game_start(session)),
        };
        let groups = session["actions"].as_array().map_or(0, Vec::len);
        if next >= groups {
            self.enter(Stage::Finalization, self.timings.finalization, now);
        } else {
            set_group_in_progress(session, next);
            let duration = match group(session, next)
                .first()
                .and_then(|a| a["type"].as_str())
            {
                Some("ban") => self.timings.ban,
                Some("pick") => self.timings.pick,
                _ => self.timings.reveal,
            };
            self.enter(Stage::Group(next), duration, now);
        }
        self.write_timer(session, now);
        step.log.push(self.describe(session));
        None
    }

    /// Fecha as ações que faltam no grupo `g`. Devolve `true` se o jogador
    /// ficou sem campeão no pick (dodge).
    fn expire_group(
        &mut self,
        state: &LcuState,
        session: &mut Value,
        g: usize,
        forced: bool,
    ) -> bool {
        let local = session["localPlayerCellId"].clone();
        for i in 0..group(session, g).len() {
            let action = &session["actions"][g][i];
            if action["completed"] == true {
                continue;
            }
            let kind = action["type"].as_str().unwrap_or_default().to_owned();
            let mine = action["actorCellId"] == local;
            let hovered = action["championId"].as_i64().unwrap_or(0);
            let champion = match kind.as_str() {
                "pick" if mine && hovered <= 0 && !forced => return true,
                "pick" | "ban" if hovered > 0 => hovered,
                "pick" => self.choose(state, session),
                "ban" if mine => 0,
                "ban" => self.choose(state, session),
                _ => 0,
            };
            complete(session, g, i, champion);
        }
        false
    }

    /// Os bots agem conforme o tempo passa na vez deles.
    fn bots(&mut self, state: &LcuState, session: &mut Value, now: u64) {
        let local = session["localPlayerCellId"].clone();
        let elapsed = now.saturating_sub(self.started_at);
        let bot = self.timings.bot.max(1);
        match self.stage {
            Stage::Planning => {
                // Aliados mostram a intenção no meio do planejamento.
                if elapsed < bot {
                    return;
                }
                let cells: Vec<usize> = (0..team_len(session, "myTeam"))
                    .filter(|&i| {
                        let p = &session["myTeam"][i];
                        p["cellId"] != local && p["championPickIntent"].as_i64().unwrap_or(0) == 0
                    })
                    .collect();
                for i in cells {
                    let champion = self.choose(state, session);
                    session["myTeam"][i]["championPickIntent"] = json!(champion);
                }
            }
            Stage::Group(g) => {
                let mut order = 0;
                for i in 0..group(session, g).len() {
                    let action = &session["actions"][g][i];
                    if action["completed"] == true
                        || action["actorCellId"] == local
                        || action["actorCellId"].as_i64().unwrap_or(-1) < 0
                    {
                        continue;
                    }
                    order += 1;
                    let due = bot * order;
                    let is_pick = action["type"] == "pick";
                    let ally = action["isAllyAction"] == true;
                    let hovered = action["championId"].as_i64().unwrap_or(0);
                    if elapsed >= due {
                        let champion = if hovered > 0 {
                            hovered
                        } else {
                            self.choose(state, session)
                        };
                        complete(session, g, i, champion);
                    } else if is_pick && ally && hovered == 0 && elapsed + bot / 2 >= due {
                        let champion = self.choose(state, session);
                        hover(session, g, i, champion);
                    }
                }
            }
            Stage::Finalization => {}
        }
    }

    /// Um campeão que ninguém pegou nem baniu, sorteado.
    fn choose(&mut self, state: &LcuState, session: &Value) -> i64 {
        let taken = taken_champions(session);
        let catalog: Vec<i64> = champions::game_data(state)
            .body
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["id"].as_i64())
            .filter(|id| *id > 0 && !taken.contains(id))
            .collect();
        if catalog.is_empty() {
            return 1;
        }
        // LCG simples: o mesmo cenário sorteia sempre os mesmos campeões.
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        catalog[((self.seed >> 33) % catalog.len() as u64) as usize]
    }

    /// ARAM: todo mundo com um campeão sorteado, 4 no banco.
    fn roll_aram(&mut self, state: &LcuState, session: &mut Value) {
        for i in 0..team_len(session, "myTeam") {
            let champion = self.choose(state, session);
            let player = &mut session["myTeam"][i];
            player["championId"] = json!(champion);
            player["selectedSkinId"] = json!(champion * 1000);
        }
        let mut bench = Vec::new();
        for _ in 0..4 {
            let champion = self.choose(state, session);
            bench.push(json!({"championId": champion, "isPriority": false}));
            session["benchChampions"] = Value::Array(bench.clone());
        }
    }

    fn write_timer(&self, session: &mut Value, now: u64) {
        let phase = match self.stage {
            Stage::Planning => "PLANNING",
            Stage::Group(_) => "BAN_PICK",
            Stage::Finalization => "FINALIZATION",
        };
        let reference = self.paused_at.unwrap_or(now);
        session["timer"] = json!({
            "adjustedTimeLeftInPhase": self.ends_at.saturating_sub(reference),
            "internalNowInEpochMs": epoch_ms(),
            "isInfinite": self.paused_at.is_some(),
            "phase": phase,
            "totalTimeInPhase": self.ends_at.saturating_sub(self.started_at),
        });
    }
}

/// Dodge: a seleção some e a fase volta para `Lobby` (ou `None`).
pub fn dodge(state: &LcuState) -> Step {
    let mut step = Step {
        finished: true,
        ..Step::default()
    };
    if state.get(SESSION).status == 200 {
        step.events.push(event(SESSION, "Delete", Value::Null));
    }
    let phase = if state.get(LOBBY).status == 200 {
        "Lobby"
    } else {
        "None"
    };
    step.events
        .push(event(GAMEFLOW_PHASE, "Update", json!(phase)));
    step.log.push(format!("dodge: fase {phase}"));
    step
}

/// A sessão nova: o molde com o jogador na célula pedida e as ações do
/// modo, sem nenhum campeão escolhido.
fn build_session(template: &Value, options: Options) -> Value {
    let mut session = template.clone();
    for team in ["myTeam", "theirTeam"] {
        if let Some(players) = session[team].as_array_mut() {
            for p in players {
                p["championId"] = json!(0);
                p["championPickIntent"] = json!(0);
                p["selectedSkinId"] = json!(0);
                p["isAutofilled"] = json!(false);
            }
        }
    }
    if let Some(cell) = options.cell {
        move_local_player(&mut session, cell);
    }
    session["bans"] = json!({"myTeamBans": [], "numBans": 0, "theirTeamBans": []});
    session["benchChampions"] = json!([]);
    session["benchEnabled"] = json!(false);
    session["allowRerolling"] = json!(false);
    session["rerollsRemaining"] = json!(0);
    session["counter"] = json!(0);
    for swaps in ["trades", "pickOrderSwaps", "positionSwaps"] {
        session[swaps] = json!([]);
    }
    let game_id = session["gameId"].as_u64().unwrap_or(0) + 1;
    session["gameId"] = json!(game_id);
    let (my_cells, their_cells) = (cells(&session, "myTeam"), cells(&session, "theirTeam"));
    match options.mode {
        Mode::Draft => {
            session["queueId"] = json!(420);
            session["hasSimultaneousBans"] = json!(true);
            session["hasSimultaneousPicks"] = json!(false);
            session["actions"] = draft_actions(&my_cells, &their_cells);
        }
        Mode::Tournament => {
            // Personalizada: a LCU manda `queueId` 0 (suposição do mock).
            session["queueId"] = json!(0);
            session["isCustomGame"] = json!(true);
            session["hasSimultaneousBans"] = json!(false);
            session["hasSimultaneousPicks"] = json!(false);
            session["actions"] = tournament_actions(&my_cells, &their_cells);
        }
        Mode::Blind => {
            session["queueId"] = json!(430);
            session["hasSimultaneousBans"] = json!(false);
            session["hasSimultaneousPicks"] = json!(true);
            let picks: Vec<Value> = my_cells
                .iter()
                .enumerate()
                .map(|(i, &cell)| action(10 + i as u64, "pick", cell, true))
                .collect();
            session["actions"] = json!([picks]);
            clear_positions(&mut session);
        }
        Mode::Aram => {
            session["queueId"] = json!(450);
            session["actions"] = json!([]);
            session["theirTeam"] = json!([]);
            session["benchEnabled"] = json!(true);
            session["allowRerolling"] = json!(true);
            session["rerollsRemaining"] = json!(2);
            clear_positions(&mut session);
        }
    }
    session
}

/// 10 bans ao mesmo tempo, a revelação e os picks 1-2-2-2-2-1 (ids e ordem
/// da gravação 16.22).
fn draft_actions(mine: &[i64], theirs: &[i64]) -> Value {
    let bans: Vec<Value> = mine
        .iter()
        .map(|&c| (c, true))
        .chain(theirs.iter().map(|&c| (c, false)))
        .enumerate()
        .map(|(i, (cell, ally))| action(i as u64, "ban", cell, ally))
        .collect();
    let mut groups = vec![
        Value::Array(bans),
        json!([action(100, "ten_bans_reveal", -1, true)]),
    ];
    // Ordem dos picks: A1 | B1 B2 | A2 A3 | B3 B4 | A4 A5 | B5.
    let order: [&[(bool, usize)]; 6] = [
        &[(true, 0)],
        &[(false, 0), (false, 1)],
        &[(true, 1), (true, 2)],
        &[(false, 2), (false, 3)],
        &[(true, 3), (true, 4)],
        &[(false, 4)],
    ];
    let mut id = 10;
    for turn in order {
        let picks: Vec<Value> = turn
            .iter()
            .filter_map(|&(ally, i)| {
                let cell = *if ally { mine } else { theirs }.get(i)?;
                let a = action(id, "pick", cell, ally);
                id += 1;
                Some(a)
            })
            .collect();
        if !picks.is_empty() {
            groups.push(Value::Array(picks));
        }
    }
    Value::Array(groups)
}

/// Draft de torneio: cada ban sozinho, os picks em pares, em duas fases.
/// `true` é o time do jogador (lado azul).
fn tournament_actions(mine: &[i64], theirs: &[i64]) -> Value {
    use Kind::{Ban, Pick};
    #[derive(Clone, Copy)]
    enum Kind {
        Ban,
        Pick,
    }
    let turns: &[&[(Kind, bool, usize)]] = &[
        &[(Ban, true, 0)],
        &[(Ban, false, 0)],
        &[(Ban, true, 1)],
        &[(Ban, false, 1)],
        &[(Ban, true, 2)],
        &[(Ban, false, 2)],
        &[(Pick, true, 0)],
        &[(Pick, false, 0), (Pick, false, 1)],
        &[(Pick, true, 1), (Pick, true, 2)],
        &[(Pick, false, 2)],
        &[(Ban, false, 3)],
        &[(Ban, true, 3)],
        &[(Ban, false, 4)],
        &[(Ban, true, 4)],
        &[(Pick, false, 3)],
        &[(Pick, true, 3), (Pick, true, 4)],
        &[(Pick, false, 4)],
    ];
    let mut id = 0;
    let groups: Vec<Value> = turns
        .iter()
        .filter_map(|turn| {
            let actions: Vec<Value> = turn
                .iter()
                .filter_map(|&(kind, ally, i)| {
                    let cell = *if ally { mine } else { theirs }.get(i)?;
                    let kind = match kind {
                        Ban => "ban",
                        Pick => "pick",
                    };
                    id += 1;
                    Some(action(id, kind, cell, ally))
                })
                .collect();
            (!actions.is_empty()).then_some(Value::Array(actions))
        })
        .collect();
    Value::Array(groups)
}

fn action(id: u64, kind: &str, cell: i64, ally: bool) -> Value {
    json!({
        "actorCellId": cell,
        "championId": 0,
        "completed": false,
        "duration": 0,
        "id": id,
        "isAllyAction": ally,
        "isInProgress": false,
        "pickTurn": 0,
        "type": kind,
    })
}

/// Troca o jogador logado de lugar com quem está na célula `cell`, mantendo
/// a ordem das células.
fn move_local_player(session: &mut Value, cell: u64) {
    let local = session["localPlayerCellId"].as_u64().unwrap_or(0);
    let Some(team) = session["myTeam"].as_array_mut() else {
        return;
    };
    let from = team
        .iter()
        .position(|p| p["cellId"].as_u64() == Some(local));
    let to = team.iter().position(|p| p["cellId"].as_u64() == Some(cell));
    let (Some(from), Some(to)) = (from, to) else {
        return;
    };
    team.swap(from, to);
    team[from]["cellId"] = json!(local);
    team[to]["cellId"] = json!(cell);
    session["localPlayerCellId"] = json!(cell);
}

fn clear_positions(session: &mut Value) {
    if let Some(players) = session["myTeam"].as_array_mut() {
        for p in players {
            p["assignedPosition"] = json!("");
        }
    }
}

fn cells(session: &Value, team: &str) -> Vec<i64> {
    session[team]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["cellId"].as_i64())
        .collect()
}

fn team_len(session: &Value, team: &str) -> usize {
    session[team].as_array().map_or(0, Vec::len)
}

fn group(session: &Value, g: usize) -> Vec<Value> {
    session["actions"][g]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn group_completed(session: &Value, g: usize) -> bool {
    let actions = group(session, g);
    !actions.is_empty() && actions.iter().all(|a| a["completed"] == true)
}

fn set_group_in_progress(session: &mut Value, g: usize) {
    if let Some(actions) = session["actions"][g].as_array_mut() {
        for a in actions {
            a["isInProgress"] = json!(true);
        }
    }
}

fn set_group_done(session: &mut Value, g: usize) {
    if let Some(actions) = session["actions"][g].as_array_mut() {
        for a in actions {
            a["isInProgress"] = json!(false);
            a["completed"] = json!(true);
        }
    }
}

/// O jogador da célula `cell` em qualquer time.
fn player_mut<'a>(session: &'a mut Value, cell: &Value) -> Option<&'a mut Value> {
    let in_mine = session["myTeam"]
        .as_array()
        .is_some_and(|t| t.iter().any(|p| p["cellId"] == *cell));
    let team = if in_mine { "myTeam" } else { "theirTeam" };
    session[team]
        .as_array_mut()?
        .iter_mut()
        .find(|p| p["cellId"] == *cell)
}

fn hover(session: &mut Value, g: usize, i: usize, champion: i64) {
    session["actions"][g][i]["championId"] = json!(champion);
    let cell = session["actions"][g][i]["actorCellId"].clone();
    if let Some(player) = player_mut(session, &cell) {
        player["championId"] = json!(champion);
    }
}

fn complete(session: &mut Value, g: usize, i: usize, champion: i64) {
    let action = &mut session["actions"][g][i];
    action["championId"] = json!(champion);
    action["completed"] = json!(true);
    action["isInProgress"] = json!(false);
    if action["type"] == "pick" {
        let cell = action["actorCellId"].clone();
        if let Some(player) = player_mut(session, &cell) {
            player["championId"] = json!(champion);
            player["championPickIntent"] = json!(0);
            player["selectedSkinId"] = json!(champion * 1000);
        }
    }
}

/// Campeões já marcados em alguma ação, nos times, nas intenções ou no
/// banco.
fn taken_champions(session: &Value) -> Vec<i64> {
    let actions = session["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|a| a["championId"].as_i64());
    let players = ["myTeam", "theirTeam"].into_iter().flat_map(|team| {
        session[team]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|p| [p["championId"].as_i64(), p["championPickIntent"].as_i64()])
            .flatten()
    });
    let bench = session["benchChampions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["championId"].as_i64());
    actions
        .chain(players)
        .chain(bench)
        .filter(|id| *id > 0)
        .collect()
}

/// A finalização acabou: `GAME_STARTING`, `GameStart`, `InProgress` e a
/// sessão some, na ordem da gravação.
fn game_start(session: &mut Value) -> Step {
    session["timer"] = json!({
        "adjustedTimeLeftInPhase": 0,
        "internalNowInEpochMs": epoch_ms(),
        "isInfinite": false,
        "phase": "GAME_STARTING",
        "totalTimeInPhase": 0,
    });
    bump(session);
    Step {
        events: vec![
            event(SESSION, "Update", session.clone()),
            event(GAMEFLOW_PHASE, "Update", json!("GameStart")),
            event(GAMEFLOW_PHASE, "Update", json!("InProgress")),
            event(SESSION, "Delete", Value::Null),
        ],
        log: vec!["partida iniciada (fase InProgress)".into()],
        finished: true,
    }
}

fn finished(message: &str) -> Step {
    Step {
        events: Vec::new(),
        log: vec![message.to_owned()],
        finished: true,
    }
}

fn bump(session: &mut Value) {
    let counter = session["counter"].as_i64().unwrap_or(0) + 1;
    session["counter"] = json!(counter);
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn event(uri: &str, event_type: &str, data: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.into(),
        event_type: event_type.into(),
        data,
    }
}

/// Erro das rotas de controle sem seleção simulada.
pub fn not_running() -> Response {
    Response::error(404, "mock-lcu: nenhuma seleção simulada em andamento")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::champ_select;

    fn fixtures() -> FixtureSet {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
        FixtureSet::load(&dir).unwrap()
    }

    fn apply(state: &mut LcuState, step: &Step) {
        for e in &step.events {
            state.apply(e);
        }
    }

    fn session(state: &LcuState) -> Value {
        state.get(SESSION).body
    }

    fn start(mode: Mode, cell: Option<u64>) -> (LcuState, Scenario) {
        let set = fixtures();
        let mut state = LcuState::new(&set);
        let template = template(&set).unwrap();
        let (scenario, step) = Scenario::start(
            &state,
            &template,
            Options { mode, cell },
            Timings::default(),
            0,
        );
        apply(&mut state, &step);
        (state, scenario)
    }

    fn in_progress(session: &Value) -> Vec<Value> {
        session["actions"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g.as_array().unwrap().clone())
            .filter(|a| a["isInProgress"] == true)
            .collect()
    }

    fn my_action(session: &Value) -> Option<Value> {
        in_progress(session)
            .into_iter()
            .find(|a| a["actorCellId"] == session["localPlayerCellId"] && a["completed"] != true)
    }

    /// Anda o relógio de 100 em 100 ms até `until`, como o timer do mock.
    fn run(state: &mut LcuState, scenario: &mut Scenario, from: u64, until: u64) -> bool {
        let mut now = from;
        while now <= until {
            let step = scenario.tick(state, now);
            apply(state, &step);
            if step.finished {
                return true;
            }
            now += 100;
        }
        false
    }

    #[test]
    fn le_as_opcoes() {
        assert_eq!(
            Options::from_json(&json!({"mode": "blind", "cell": 4})),
            Ok(Options {
                mode: Mode::Blind,
                cell: Some(4)
            })
        );
        assert_eq!(Options::from_json(&Value::Null).unwrap().mode, Mode::Draft);
        assert!(Options::from_json(&json!({"cell": 5})).is_err());
        assert!(Options::from_json(&json!({"mode": "urf"})).is_err());
    }

    #[test]
    fn comeca_no_planejamento_com_o_draft_da_gravacao() {
        let (state, _) = start(Mode::Draft, None);
        assert_eq!(state.phase(), "ChampSelect");
        let s = session(&state);
        assert_eq!(s["timer"]["phase"], "PLANNING");
        assert_eq!(s["timer"]["adjustedTimeLeftInPhase"], 12_000);
        assert_eq!(s["queueId"], 420);
        let groups = s["actions"].as_array().unwrap();
        let sizes: Vec<usize> = groups.iter().map(|g| g.as_array().unwrap().len()).collect();
        assert_eq!(sizes, [10, 1, 1, 2, 2, 2, 2, 1]);
        assert!(s["myTeam"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["championId"] == 0));
    }

    #[test]
    fn celula_troca_o_jogador_de_lugar() {
        let (state, _) = start(Mode::Draft, Some(0));
        let s = session(&state);
        assert_eq!(s["localPlayerCellId"], 0);
        let template = template(&fixtures()).unwrap();
        let original = template["localPlayerCellId"].as_u64().unwrap() as usize;
        assert_eq!(
            s["myTeam"][0]["puuid"],
            template["myTeam"][original]["puuid"]
        );
        assert_eq!(s["myTeam"][0]["cellId"], 0);
        // O primeiro pick é dele.
        assert_eq!(s["actions"][2][0]["actorCellId"], 0);
    }

    #[test]
    fn draft_inteiro_com_os_bots_ate_a_partida() {
        let (mut state, mut scenario) = start(Mode::Draft, None);
        let mut now = 0;
        let mut acted = Vec::new();
        loop {
            let step = scenario.tick(&state, now);
            apply(&mut state, &step);
            if step.finished {
                break;
            }
            let s = session(&state);
            if s["timer"]["phase"] == "BAN_PICK" {
                if let Some(action) = my_action(&s) {
                    let id = action["id"].as_u64().unwrap();
                    let champion = if action["type"] == "ban" { 99 } else { 777 };
                    let (r, events) =
                        champ_select::hover(&state, id, &json!({"championId": champion}));
                    assert_eq!(r.status, 204);
                    for e in &events {
                        state.apply(e);
                    }
                    let (r, events) = champ_select::complete(&state, id);
                    assert_eq!(r.status, 204);
                    for e in &events {
                        state.apply(e);
                    }
                    acted.push(action["type"].as_str().unwrap().to_owned());
                }
            }
            now += 100;
            assert!(now < 600_000, "a seleção não acabou");
        }
        assert_eq!(acted, ["ban", "pick"]);
        assert_eq!(state.phase(), "InProgress");
        assert_eq!(state.get(SESSION).status, 404);
    }

    #[test]
    fn bots_confirmam_e_aliados_mostram_hover() {
        let (mut state, mut scenario) = start(Mode::Draft, Some(4));
        // Planejamento, bans e revelação passam com a avançada.
        for _ in 0..3 {
            let step = scenario.advance(&state, 0);
            apply(&mut state, &step);
        }
        // Agora o primeiro pick (célula 0, aliado bot).
        let s = session(&state);
        assert_eq!(in_progress(&s)[0]["actorCellId"], 0);
        assert!(s["actions"][0]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["completed"] == true));
        run(&mut state, &mut scenario, 0, 2_000);
        let s = session(&state);
        let hovered = s["actions"][2][0]["championId"].as_i64().unwrap();
        assert!(hovered > 0, "o aliado mostra o hover antes de confirmar");
        assert_eq!(s["actions"][2][0]["completed"], false);
        run(&mut state, &mut scenario, 2_000, 3_100);
        let s = session(&state);
        assert_eq!(s["actions"][2][0]["completed"], true);
        assert_eq!(s["myTeam"][0]["championId"], hovered);
        // Os picks dos adversários (células 5 e 6) estão abertos.
        let open: Vec<_> = in_progress(&s)
            .iter()
            .map(|a| a["actorCellId"].clone())
            .collect();
        assert_eq!(open, [json!(5), json!(6)]);
    }

    #[test]
    fn pausar_congela_o_tempo_e_os_bots() {
        let (mut state, mut scenario) = start(Mode::Draft, None);
        let step = scenario.advance(&state, 1_000); // bans
        apply(&mut state, &step);
        let step = scenario.pause(&state, 2_000);
        apply(&mut state, &step);
        let s = session(&state);
        assert_eq!(s["timer"]["isInfinite"], true);
        assert!(!run(&mut state, &mut scenario, 2_000, 60_000));
        let s = session(&state);
        assert_eq!(s["timer"]["phase"], "BAN_PICK");
        assert!(s["actions"][0]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["completed"] == false));
        let step = scenario.resume(&state, 60_000);
        apply(&mut state, &step);
        let s = session(&state);
        assert_eq!(s["timer"]["isInfinite"], false);
        assert_eq!(s["timer"]["adjustedTimeLeftInPhase"], 24_000);
    }

    #[test]
    fn avancar_pausado_continua_pausado_na_etapa_seguinte() {
        let (mut state, mut scenario) = start(Mode::Draft, None);
        let step = scenario.pause(&state, 0);
        apply(&mut state, &step);
        let step = scenario.advance(&state, 500);
        apply(&mut state, &step);
        let s = session(&state);
        assert_eq!(s["timer"]["phase"], "BAN_PICK");
        assert_eq!(s["timer"]["isInfinite"], true);
        assert_eq!(s["timer"]["adjustedTimeLeftInPhase"], 25_000);
        // Avançar os bans: o ban do jogador sem campeão fica vazio.
        let step = scenario.advance(&state, 600);
        apply(&mut state, &step);
        let s = session(&state);
        let mine = s["actions"][0]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["actorCellId"] == s["localPlayerCellId"])
            .unwrap()
            .clone();
        assert_eq!(mine["completed"], true);
        assert_eq!(mine["championId"], 0);
        assert_eq!(s["actions"][1][0]["isInProgress"], true);
    }

    #[test]
    fn tempo_esgotado_no_pick_sem_hover_e_dodge() {
        let (mut state, mut scenario) = start(Mode::Draft, Some(0));
        for _ in 0..3 {
            let step = scenario.advance(&state, 0);
            apply(&mut state, &step);
        }
        // Vez do jogador (primeiro pick), sem hover, até o tempo acabar.
        assert!(run(&mut state, &mut scenario, 0, 26_000));
        assert_eq!(state.get(SESSION).status, 404);
        assert_ne!(state.phase(), "ChampSelect");
    }

    #[test]
    fn tempo_esgotado_com_hover_confirma() {
        let (mut state, mut scenario) = start(Mode::Draft, Some(0));
        for _ in 0..3 {
            let step = scenario.advance(&state, 0);
            apply(&mut state, &step);
        }
        let (_, events) = champ_select::hover(&state, 10, &json!({"championId": 777}));
        for e in &events {
            state.apply(e);
        }
        assert!(!run(&mut state, &mut scenario, 0, 26_000));
        let s = session(&state);
        assert_eq!(s["actions"][2][0]["completed"], true);
        assert_eq!(s["myTeam"][0]["championId"], 777);
    }

    #[test]
    fn dodge_volta_para_fora_da_selecao() {
        let (mut state, _) = start(Mode::Blind, None);
        let step = dodge(&state);
        assert!(step.finished);
        apply(&mut state, &step);
        assert_eq!(state.get(SESSION).status, 404);
        assert_ne!(state.phase(), "ChampSelect");
    }

    #[test]
    fn torneio_alterna_os_bans_em_duas_fases() {
        let (mut state, mut scenario) = start(Mode::Tournament, None);
        let s = session(&state);
        assert_eq!(s["timer"]["phase"], "BAN_PICK");
        let kinds: Vec<String> = s["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| {
                g.as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a["type"].as_str().unwrap()[..1].to_owned())
                    .collect()
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "b", "b", "b", "b", "b", "b", "p", "pp", "pp", "p", "b", "b", "b", "b", "p", "pp",
                "p"
            ]
        );
        // Um ban por vez: só o primeiro está aberto.
        assert_eq!(in_progress(&s).len(), 1);
        // Até o fim, com o jogador confirmando quando é a vez dele.
        let mut now = 0;
        loop {
            let step = scenario.advance(&state, now);
            apply(&mut state, &step);
            if step.finished {
                break;
            }
            now += 1;
            assert!(now < 100);
        }
        assert_eq!(state.phase(), "InProgress");
    }

    #[test]
    fn blind_todos_escolhem_juntos() {
        let (state, _) = start(Mode::Blind, None);
        let s = session(&state);
        assert_eq!(s["queueId"], 430);
        assert_eq!(s["timer"]["phase"], "BAN_PICK");
        assert_eq!(in_progress(&s).len(), 5);
    }

    #[test]
    fn aram_sorteia_campeoes_e_banco() {
        let (state, _) = start(Mode::Aram, None);
        let s = session(&state);
        assert_eq!(s["queueId"], 450);
        assert_eq!(s["timer"]["phase"], "FINALIZATION");
        assert_eq!(s["benchChampions"].as_array().unwrap().len(), 4);
        let champions: Vec<i64> = s["myTeam"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["championId"].as_i64().unwrap())
            .collect();
        assert!(champions.iter().all(|c| *c > 0));
        let mut unique = champions.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), champions.len());
        assert_eq!(s["allowRerolling"], true);
    }
}
