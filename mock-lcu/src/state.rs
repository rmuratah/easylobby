//! Estado da LCU falsa: que resposta cada `GET` recebe agora.
//!
//! As respostas gravadas (`rest.json`) são marcadas com a fase do gameflow em
//! que foram tiradas. O estado guarda a fase atual e as fases já visitadas;
//! um `GET` devolve a resposta gravada na fase atual ou, se não houver, na
//! fase visitada mais recente que tenha uma.
//!
//! Os eventos reproduzidos mudam o estado por cima disso: um `Create` ou
//! `Update` vira a resposta do recurso, um `Delete` faz o recurso responder
//! 404. Quando a fase muda, as respostas gravadas na nova fase voltam a valer
//! para os recursos que elas cobrem.

use std::collections::HashMap;

use easylobby_agent::fixtures::{EventFixture, FixtureSet};
use serde_json::{json, Value};

/// Recurso com a fase do gameflow.
pub const GAMEFLOW_PHASE: &str = "/lol-gameflow/v1/gameflow-phase";

/// Recurso do ready check (partida encontrada).
pub const READY_CHECK: &str = "/lol-matchmaking/v1/ready-check";

/// Fase quando não há resposta gravada para dizer outra.
const DEFAULT_PHASE: &str = "None";

/// Uma resposta HTTP: status e corpo JSON (`Value::Null` para corpo vazio).
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: Value,
}

impl Response {
    /// Erro no formato da LCU (`{"errorCode": "RPC_ERROR", ...}`).
    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            body: json!({
                "errorCode": "RPC_ERROR",
                "httpStatus": status,
                "implementationDetails": {},
                "message": message.into(),
            }),
        }
    }
}

/// Estado da LCU falsa.
#[derive(Debug, Clone)]
pub struct LcuState {
    /// Caminho -> (fase, resposta), na ordem do `rest.json`.
    recorded: HashMap<String, Vec<(String, Response)>>,
    initial_phase: String,
    /// Fases visitadas, da primeira à atual.
    phases: Vec<String>,
    /// Mudanças vindas de eventos: `Some` é o novo corpo, `None` é `Delete`.
    overlay: HashMap<String, Option<Value>>,
    /// Se a UI oficial está aberta (`kill-ux` fecha, `launch-ux` abre).
    ux_open: bool,
    /// Pedidos ao `/product-launcher` do Riot Client, como
    /// `league_of_legends/live`.
    launches: Vec<String>,
}

impl LcuState {
    pub fn new(set: &FixtureSet) -> Self {
        let mut recorded: HashMap<String, Vec<(String, Response)>> = HashMap::new();
        for r in &set.rest {
            if !r.method.eq_ignore_ascii_case("GET") {
                continue;
            }
            recorded.entry(r.path.clone()).or_default().push((
                r.phase.clone(),
                Response {
                    status: r.status,
                    body: r.body.clone(),
                },
            ));
        }
        let initial_phase = set
            .rest
            .first()
            .map(|r| r.phase.clone())
            .unwrap_or_else(|| DEFAULT_PHASE.to_owned());
        Self {
            recorded,
            phases: vec![initial_phase.clone()],
            initial_phase,
            overlay: HashMap::new(),
            ux_open: true,
            launches: Vec::new(),
        }
    }

    /// Fase atual do gameflow.
    pub fn phase(&self) -> &str {
        self.phases.last().map_or(DEFAULT_PHASE, String::as_str)
    }

    pub fn ux_open(&self) -> bool {
        self.ux_open
    }

    pub fn set_ux_open(&mut self, open: bool) {
        self.ux_open = open;
    }

    /// Pedidos ao `/product-launcher`, do primeiro ao último.
    pub fn launches(&self) -> &[String] {
        &self.launches
    }

    pub fn record_launch(&mut self, product_patchline: String) {
        self.launches.push(product_patchline);
    }

    /// Resposta de `GET path`. `path` pode ter query; tenta primeiro o caminho
    /// inteiro e depois sem a query.
    pub fn get(&self, path: &str) -> Response {
        let without_query = path.split('?').next().unwrap_or(path);
        for candidate in [path, without_query] {
            if let Some(response) = self.lookup(candidate) {
                return response;
            }
        }
        Response::error(
            404,
            format!("mock-lcu: nenhuma resposta gravada para {without_query}"),
        )
    }

    fn lookup(&self, path: &str) -> Option<Response> {
        if let Some(change) = self.overlay.get(path) {
            return Some(match change {
                Some(body) => Response {
                    status: 200,
                    body: body.clone(),
                },
                None => Response::error(404, format!("{path} foi apagado (evento Delete)")),
            });
        }
        let recorded = self.recorded.get(path)?;
        self.phases.iter().rev().find_map(|phase| {
            recorded
                .iter()
                .rev()
                .find(|(p, _)| p == phase)
                .map(|(_, r)| r.clone())
        })
    }

    /// Primeira resposta `200` gravada para `path`, em qualquer fase. Serve de
    /// modelo para respostas que o mock monta, como o lobby de um `POST`.
    pub fn recorded_body(&self, path: &str) -> Option<&Value> {
        self.recorded
            .get(path)?
            .iter()
            .find(|(_, r)| r.status == 200)
            .map(|(_, r)| &r.body)
    }

    /// Aplica um evento reproduzido.
    pub fn apply(&mut self, event: &EventFixture) {
        if event.uri == GAMEFLOW_PHASE {
            if let Some(phase) = event.data.as_str() {
                if phase != self.phase() {
                    self.enter_phase(phase);
                }
            }
        }
        let change = match event.event_type.as_str() {
            "Delete" => None,
            _ => Some(event.data.clone()),
        };
        self.overlay.insert(event.uri.clone(), change);
    }

    /// Entra numa fase: as respostas gravadas nela passam a valer de novo para
    /// os recursos que cobrem.
    fn enter_phase(&mut self, phase: &str) {
        let recorded = &self.recorded;
        self.overlay.retain(|path, _| {
            !recorded
                .get(path)
                .is_some_and(|rs| rs.iter().any(|(p, _)| p == phase))
        });
        self.phases.push(phase.to_owned());
    }

    /// Evento que a LCU mandaria se o jogador respondesse o ready check com
    /// `response` (`"Accepted"` ou `"Declined"`): o ready check atual com o
    /// `playerResponse` novo. `Err` com a resposta HTTP de erro quando não há
    /// ready check em andamento. O chamador emite o evento.
    pub fn answer_ready_check(&self, response: &str) -> Result<EventFixture, Response> {
        let current = self.get(READY_CHECK);
        let in_progress = current.status == 200 && current.body["state"] == "InProgress";
        if !in_progress {
            return Err(Response::error(404, "Not attached to a matchmaking queue."));
        }
        let mut data = current.body;
        data["playerResponse"] = Value::String(response.to_owned());
        Ok(EventFixture {
            t_ms: 0,
            uri: READY_CHECK.to_owned(),
            event_type: "Update".to_owned(),
            data,
        })
    }

    /// Volta ao começo da gravação. A UI oficial continua como está.
    pub fn reset(&mut self) {
        self.phases = vec![self.initial_phase.clone()];
        self.overlay.clear();
    }

    /// Fase em que a gravação começa.
    pub fn initial_phase(&self) -> &str {
        &self.initial_phase
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use easylobby_agent::fixtures::{Manifest, RestFixture, FORMAT_VERSION};

    fn rest(path: &str, phase: &str, status: u16, body: Value) -> RestFixture {
        RestFixture {
            method: "GET".into(),
            path: path.into(),
            phase: phase.into(),
            status,
            body,
        }
    }

    fn event(uri: &str, event_type: &str, data: Value) -> EventFixture {
        EventFixture {
            t_ms: 0,
            uri: uri.into(),
            event_type: event_type.into(),
            data,
        }
    }

    fn set() -> FixtureSet {
        FixtureSet {
            manifest: Manifest {
                format_version: FORMAT_VERSION,
                patch: "teste".into(),
                game_version: None,
                recorded_at: "2026-01-01T00:00:00Z".into(),
                synthetic: true,
                notes: String::new(),
            },
            rest: vec![
                rest(GAMEFLOW_PHASE, "None", 200, json!("None")),
                rest(
                    "/lol-chat/v1/me",
                    "None",
                    200,
                    json!({ "id": "fake-pid-0001" }),
                ),
                rest(
                    "/lol-lobby/v2/lobby",
                    "None",
                    404,
                    json!({ "message": "LOBBY_NOT_FOUND" }),
                ),
                rest(GAMEFLOW_PHASE, "Lobby", 200, json!("Lobby")),
                rest(
                    "/lol-lobby/v2/lobby",
                    "Lobby",
                    200,
                    json!({ "partyId": "gravado" }),
                ),
            ],
            events: vec![],
        }
    }

    #[test]
    fn responde_com_a_fase_atual() {
        let mut s = LcuState::new(&set());
        assert_eq!(s.phase(), "None");
        assert_eq!(s.get("/lol-lobby/v2/lobby").status, 404);

        s.apply(&event(GAMEFLOW_PHASE, "Update", json!("Lobby")));
        assert_eq!(s.phase(), "Lobby");
        assert_eq!(s.get(GAMEFLOW_PHASE).body, json!("Lobby"));
        assert_eq!(s.get("/lol-lobby/v2/lobby").body["partyId"], "gravado");
        // Sem resposta gravada no Lobby: vale a da fase anterior.
        assert_eq!(s.get("/lol-chat/v1/me").body["id"], "fake-pid-0001");
    }

    #[test]
    fn eventos_mudam_e_apagam_recursos() {
        let mut s = LcuState::new(&set());
        s.apply(&event(GAMEFLOW_PHASE, "Update", json!("Lobby")));
        s.apply(&event(
            "/lol-lobby/v2/lobby",
            "Update",
            json!({ "partyId": "evento" }),
        ));
        assert_eq!(s.get("/lol-lobby/v2/lobby").body["partyId"], "evento");

        s.apply(&event("/lol-lobby/v2/lobby", "Delete", Value::Null));
        assert_eq!(s.get("/lol-lobby/v2/lobby").status, 404);

        // Recurso que só existe por evento.
        s.apply(&event(
            "/lol-chat/v1/conversations/x/messages/1",
            "Create",
            json!({ "body": "oi" }),
        ));
        assert_eq!(s.get("/lol-chat/v1/conversations/x/messages/1").status, 200);
    }

    #[test]
    fn mudanca_de_fase_volta_a_resposta_gravada() {
        let mut s = LcuState::new(&set());
        s.apply(&event(
            "/lol-lobby/v2/lobby",
            "Create",
            json!({ "partyId": "evento" }),
        ));
        s.apply(&event(GAMEFLOW_PHASE, "Update", json!("Lobby")));
        assert_eq!(s.get("/lol-lobby/v2/lobby").body["partyId"], "gravado");
    }

    #[test]
    fn query_e_caminho_desconhecido() {
        let s = LcuState::new(&set());
        assert_eq!(s.get("/lol-chat/v1/me?x=1").body["id"], "fake-pid-0001");
        let missing = s.get("/lol-nada/v1/coisa");
        assert_eq!(missing.status, 404);
        assert_eq!(missing.body["errorCode"], "RPC_ERROR");
    }

    #[test]
    fn responde_o_ready_check() {
        let mut s = LcuState::new(&set());
        assert_eq!(s.answer_ready_check("Accepted").unwrap_err().status, 404);

        s.apply(&event(
            READY_CHECK,
            "Update",
            json!({ "state": "InProgress", "playerResponse": "None", "timer": 2.0 }),
        ));
        let accepted = s.answer_ready_check("Accepted").unwrap();
        assert_eq!(accepted.uri, READY_CHECK);
        assert_eq!(accepted.data["playerResponse"], "Accepted");
        assert_eq!(accepted.data["timer"], 2.0);

        s.apply(&event(
            READY_CHECK,
            "Update",
            json!({ "state": "Invalid", "playerResponse": "None", "timer": 0.0 }),
        ));
        assert!(s.answer_ready_check("Declined").is_err());
    }

    #[test]
    fn reset_volta_ao_comeco() {
        let mut s = LcuState::new(&set());
        s.apply(&event(GAMEFLOW_PHASE, "Update", json!("Lobby")));
        s.reset();
        assert_eq!(s.phase(), "None");
        assert_eq!(s.get(GAMEFLOW_PHASE).body, json!("None"));
    }

    #[test]
    fn carrega_os_conjuntos_do_repositorio() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures");
        for name in ["exemplo", "16.22"] {
            let set = FixtureSet::load(&root.join(name)).unwrap();
            let mut s = LcuState::new(&set);
            for e in &set.events {
                s.apply(e);
            }
            let last_phase = set
                .events
                .iter()
                .rev()
                .find(|e| e.uri == GAMEFLOW_PHASE)
                .and_then(|e| e.data.as_str())
                .unwrap();
            assert_eq!(s.phase(), last_phase, "{name}");
            assert_eq!(s.get(GAMEFLOW_PHASE).body, json!(last_phase), "{name}");
        }
    }
}
