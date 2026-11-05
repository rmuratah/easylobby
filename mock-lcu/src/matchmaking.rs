//! Fila da LCU falsa (#19): `POST` e `DELETE` em
//! `/lol-lobby/v2/lobby/matchmaking/search`.
//!
//! Como no lobby (#17), o mock monta a busca a partir da busca gravada e
//! manda os mesmos eventos que a LCU manda: entrar na fila muda a fase para
//! `Matchmaking` e cria a busca; enquanto procura, [`tick`] soma um segundo
//! em `timeInQueue` (o servidor chama a cada segundo); sair da fila apaga a
//! busca e volta a fase para `Lobby`. A partida nunca é encontrada: para ver
//! o resto do caminho, use a reprodução da gravação.
//!
//! As funções daqui não mexem no estado: devolvem a resposta e os eventos, e
//! quem chama emite os eventos (o que atualiza o estado e avisa o WAMP).

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::lobby::LOBBY;
use crate::state::{LcuState, Response, GAMEFLOW_PHASE};

/// Entrar (`POST`) e sair (`DELETE`) da fila.
pub const LOBBY_SEARCH: &str = "/lol-lobby/v2/lobby/matchmaking/search";

/// Busca de partida em andamento.
pub const SEARCH: &str = "/lol-matchmaking/v1/search";

/// `POST /lol-lobby/v2/lobby/matchmaking/search`: entra na fila com o lobby
/// atual.
pub fn start(state: &LcuState) -> (Response, Vec<EventFixture>) {
    let lobby = state.get(LOBBY);
    if lobby.status != 200 {
        return (
            Response::error(400, "mock-lcu: não está em um lobby"),
            Vec::new(),
        );
    }
    if searching(state) {
        // Já está na fila: nada muda.
        return (no_content(), Vec::new());
    }
    let queue_id = lobby.body["gameConfig"]["queueId"].clone();

    let mut search = state
        .recorded_body(SEARCH)
        .cloned()
        .unwrap_or_else(|| json!({"estimatedQueueTime": 90.0, "errors": []}));
    if let Some(fields) = search.as_object_mut() {
        fields.insert("queueId".into(), queue_id);
        fields.insert("searchState".into(), json!("Searching"));
        fields.insert("isCurrentlyInQueue".into(), json!(true));
        fields.insert("timeInQueue".into(), json!(0.0));
    }

    // A fase muda antes: entrar em `Matchmaking` volta os recursos para o
    // gravado, então a busca e o lobby atuais vêm depois.
    let events = vec![
        event(GAMEFLOW_PHASE, "Update", json!("Matchmaking")),
        event(SEARCH, "Create", search),
        event(LOBBY, "Update", lobby.body),
    ];
    (no_content(), events)
}

/// `DELETE /lol-lobby/v2/lobby/matchmaking/search`: sai da fila.
pub fn stop(state: &LcuState) -> (Response, Vec<EventFixture>) {
    if !searching(state) {
        return (no_content(), Vec::new());
    }
    let lobby = state.get(LOBBY);
    let mut events = vec![event(GAMEFLOW_PHASE, "Update", json!("Lobby"))];
    // O gravado da fase `Lobby` pode ter uma busca; depois da troca de fase,
    // ela é apagada.
    events.push(event(SEARCH, "Delete", Value::Null));
    if lobby.status == 200 {
        events.push(event(LOBBY, "Update", lobby.body));
    }
    (no_content(), events)
}

/// Mais um segundo na fila, se estiver procurando.
pub fn tick(state: &LcuState) -> Option<EventFixture> {
    if !searching(state) {
        return None;
    }
    let mut search = state.get(SEARCH).body;
    let time = search["timeInQueue"].as_f64().unwrap_or(0.0) + 1.0;
    search["timeInQueue"] = json!(time);
    Some(event(SEARCH, "Update", search))
}

/// Se há uma busca procurando partida.
fn searching(state: &LcuState) -> bool {
    let search = state.get(SEARCH);
    search.status == 200 && search.body["searchState"] == "Searching"
}

fn no_content() -> Response {
    Response {
        status: 204,
        body: Value::Null,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby;
    use easylobby_agent::fixtures::FixtureSet;

    fn state() -> LcuState {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
        LcuState::new(&FixtureSet::load(&dir).unwrap())
    }

    fn apply(state: &mut LcuState, events: &[EventFixture]) {
        for e in events {
            state.apply(e);
        }
    }

    #[test]
    fn entra_conta_o_tempo_e_sai() {
        let mut s = state();
        let (_, events) = lobby::create(&s, &json!({"queueId": 450}));
        apply(&mut s, &events);

        let (response, events) = start(&s);
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        assert_eq!(s.phase(), "Matchmaking");
        let search = s.get(SEARCH);
        assert_eq!(search.status, 200);
        assert_eq!(search.body["searchState"], "Searching");
        assert_eq!(search.body["queueId"], 450);
        assert_eq!(search.body["timeInQueue"], 0.0);
        // O lobby continua o criado, não o gravado (fila 420).
        assert_eq!(s.get(LOBBY).body["gameConfig"]["queueId"], 450);

        // Entrar de novo não muda nada.
        assert!(start(&s).1.is_empty());

        for _ in 0..3 {
            let e = tick(&s).unwrap();
            s.apply(&e);
        }
        assert_eq!(s.get(SEARCH).body["timeInQueue"], 3.0);

        let (response, events) = stop(&s);
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        assert_eq!(s.phase(), "Lobby");
        assert_eq!(s.get(SEARCH).status, 404);
        assert_eq!(s.get(LOBBY).body["gameConfig"]["queueId"], 450);
        assert!(tick(&s).is_none());

        // Sair de novo não faz mal.
        let (response, events) = stop(&s);
        assert_eq!(response.status, 204);
        assert!(events.is_empty());
    }

    #[test]
    fn fora_de_lobby_recusa() {
        let s = state();
        let (response, events) = start(&s);
        assert_eq!(response.status, 400);
        assert!(events.is_empty());
    }
}
