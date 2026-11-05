//! Lobby da LCU falsa (#17): `POST` e `DELETE` em `/lol-lobby/v2/lobby`.
//!
//! As fixtures só têm `GET`, então o mock monta o lobby a partir do lobby
//! gravado (trocando a fila) e manda os mesmos eventos que a LCU manda:
//! a fase do gameflow vira `Lobby` e o lobby é criado ou atualizado. Sair do
//! lobby apaga o lobby e volta a fase para `None`.
//!
//! As funções daqui não mexem no estado: devolvem a resposta e os eventos, e
//! quem chama emite os eventos (o que atualiza o estado e avisa o WAMP).

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::state::{LcuState, Response, GAMEFLOW_PHASE};

/// Recurso do lobby.
pub const LOBBY: &str = "/lol-lobby/v2/lobby";

/// Lista de filas da LCU, usada para validar o `queueId`.
const QUEUES: &str = "/lol-game-queues/v1/queues";

/// Invocador logado, usado quando não há lobby gravado para servir de
/// modelo.
const CURRENT_SUMMONER: &str = "/lol-summoner/v1/current-summoner";

/// `POST /lol-lobby/v2/lobby` com `{"queueId": N}`: cria o lobby, ou troca a
/// fila do lobby atual.
pub fn create(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let Some(queue_id) = body.get("queueId").and_then(Value::as_u64) else {
        return (
            Response::error(400, "mock-lcu: o corpo precisa de \"queueId\""),
            Vec::new(),
        );
    };
    let queue = state.recorded_body(QUEUES).and_then(|queues| {
        queues
            .as_array()?
            .iter()
            .find(|q| q.get("id").and_then(Value::as_u64) == Some(queue_id))
    });
    if state.recorded_body(QUEUES).is_some() && queue.is_none() {
        return (
            Response::error(400, format!("mock-lcu: a fila {queue_id} não existe")),
            Vec::new(),
        );
    }
    let max_lobby_size = queue
        .and_then(|q| q.get("maximumParticipantListSize"))
        .and_then(Value::as_u64)
        .unwrap_or(5);

    let current = state.get(LOBBY);
    let mut lobby = if current.status == 200 {
        current.body
    } else {
        template(state)
    };
    if let Some(config) = lobby.get_mut("gameConfig").and_then(Value::as_object_mut) {
        config.insert("queueId".into(), json!(queue_id));
        config.insert("maxLobbySize".into(), json!(max_lobby_size));
    }
    let created = current.status != 200;

    let mut events = Vec::new();
    if state.phase() != "Lobby" {
        events.push(event(GAMEFLOW_PHASE, "Update", json!("Lobby")));
    }
    events.push(event(
        LOBBY,
        if created { "Create" } else { "Update" },
        lobby.clone(),
    ));
    (
        Response {
            status: 200,
            body: lobby,
        },
        events,
    )
}

/// `DELETE /lol-lobby/v2/lobby`: sai do lobby.
pub fn leave(state: &LcuState) -> (Response, Vec<EventFixture>) {
    let mut events = Vec::new();
    if state.get(LOBBY).status == 200 {
        events.push(event(LOBBY, "Delete", Value::Null));
    }
    if state.phase() != "None" {
        events.push(event(GAMEFLOW_PHASE, "Update", json!("None")));
    }
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        events,
    )
}

/// Lobby novo: o gravado, ou um mínimo com o invocador logado de líder.
fn template(state: &LcuState) -> Value {
    if let Some(recorded) = state.recorded_body(LOBBY) {
        return recorded.clone();
    }
    let me = state.recorded_body(CURRENT_SUMMONER);
    let field = |name: &str| me.and_then(|m| m.get(name)).cloned().unwrap_or(json!(0));
    let member = json!({
        "puuid": me.and_then(|m| m.get("puuid")).cloned().unwrap_or(json!("")),
        "summonerId": field("summonerId"),
        "summonerIconId": field("profileIconId"),
        "summonerLevel": field("summonerLevel"),
        "isLeader": true,
    });
    json!({
        "canStartActivity": true,
        "gameConfig": {},
        "localMember": member,
        "members": [member],
    })
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
    use easylobby_agent::fixtures::FixtureSet;

    fn state(name: &str) -> LcuState {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../agent/tests/fixtures")
            .join(name);
        LcuState::new(&FixtureSet::load(&dir).unwrap())
    }

    fn apply(state: &mut LcuState, events: &[EventFixture]) {
        for e in events {
            state.apply(e);
        }
    }

    #[test]
    fn cria_troca_a_fila_e_sai() {
        let mut s = state("16.22");
        assert_eq!(s.get(LOBBY).status, 404);

        let (response, events) = create(&s, &json!({"queueId": 450}));
        assert_eq!(response.status, 200);
        assert_eq!(response.body["gameConfig"]["queueId"], 450);
        assert_eq!(response.body["gameConfig"]["maxLobbySize"], 5);
        let kinds: Vec<_> = events
            .iter()
            .map(|e| (e.uri.as_str(), e.event_type.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [(GAMEFLOW_PHASE, "Update"), (LOBBY, "Create")],
            "a fase muda antes, para o lobby não ser trocado pelo gravado"
        );
        apply(&mut s, &events);
        assert_eq!(s.phase(), "Lobby");
        assert_eq!(s.get(LOBBY).body["gameConfig"]["queueId"], 450);

        // Trocar a fila atualiza o mesmo lobby, sem mudar a fase.
        let (response, events) = create(&s, &json!({"queueId": 420}));
        assert_eq!(response.body["gameConfig"]["maxLobbySize"], 2);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "Update");
        apply(&mut s, &events);
        assert_eq!(s.get(LOBBY).body["gameConfig"]["queueId"], 420);

        let (response, events) = leave(&s);
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        assert_eq!(s.get(LOBBY).status, 404);
        assert_eq!(s.phase(), "None");

        // Sair de novo não faz mal.
        let (response, events) = leave(&s);
        assert_eq!(response.status, 204);
        assert!(events.is_empty());
    }

    #[test]
    fn recusa_fila_inexistente_e_corpo_sem_fila() {
        let s = state("16.22");
        let (response, events) = create(&s, &json!({"queueId": 99999}));
        assert_eq!(response.status, 400);
        assert!(events.is_empty());
        let (response, _) = create(&s, &json!({}));
        assert_eq!(response.status, 400);
    }
}
