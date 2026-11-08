//! Lobby da LCU falsa (#17): `POST` e `DELETE` em `/lol-lobby/v2/lobby`, e
//! os cargos do jogador (#18) em `PUT` [`POSITION_PREFERENCES`] e os slots
//! do Jogo Dinâmico (#218) em `PUT` [`PLAYER_SLOTS`].
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

use crate::matchmaking::SEARCH;
use crate::state::{LcuState, Response, GAMEFLOW_PHASE};

/// Recurso do lobby.
pub const LOBBY: &str = "/lol-lobby/v2/lobby";

/// Cargos do jogador logado (#18).
pub const POSITION_PREFERENCES: &str =
    "/lol-lobby/v2/lobby/members/localMember/position-preferences";

/// Slots do Jogo Dinâmico do jogador logado (#218).
pub const PLAYER_SLOTS: &str = "/lol-lobby/v1/lobby/members/localMember/player-slots";

/// Lobby aberto ou fechado (#225).
pub const PARTY_TYPE: &str = "/lol-lobby/v2/lobby/partyType";

/// Cargos que a LCU aceita.
const POSITIONS: [&str; 7] = [
    "TOP",
    "JUNGLE",
    "MIDDLE",
    "BOTTOM",
    "UTILITY",
    "FILL",
    "UNSELECTED",
];

/// Lista de filas da LCU, usada para validar o `queueId`.
const QUEUES: &str = "/lol-game-queues/v1/queues";

/// Invocador logado, usado quando não há lobby gravado para servir de
/// modelo.
const CURRENT_SUMMONER: &str = "/lol-summoner/v1/current-summoner";

/// Inicia a seleção de campeões de uma personalizada (Modo de Treino, #209).
pub const CUSTOM_START_CHAMP_SELECT: &str = "/lol-lobby/v1/lobby/custom/start-champ-select";

/// `queueId` da personalizada pelo `gameMode` de `customGameLobby`: a fila
/// personalizada (`isCustom`) da lista gravada com esse modo, como a 3140
/// do Modo de Treino (#209).
fn custom_queue_id(state: &LcuState, body: &Value) -> Option<u64> {
    let mode = body
        .pointer("/customGameLobby/configuration/gameMode")
        .and_then(Value::as_str)?;
    state
        .recorded_body(QUEUES)?
        .as_array()?
        .iter()
        .find(|q| {
            q.get("isCustom").and_then(Value::as_bool) == Some(true)
                && q.get("gameMode").and_then(Value::as_str) == Some(mode)
        })?
        .get("id")?
        .as_u64()
}

/// `POST /lol-lobby/v2/lobby` com `{"queueId": N}`: cria o lobby, ou troca a
/// fila do lobby atual. Com `customGameLobby` (o Modo de Treino, #209), cria
/// a personalizada na fila personalizada do mesmo modo.
pub fn create(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let queue_id = body
        .get("queueId")
        .and_then(Value::as_u64)
        .or_else(|| custom_queue_id(state, body));
    let Some(queue_id) = queue_id else {
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
    let show_position_selector = queue
        .and_then(|q| q.get("showPositionSelector"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let show_quick_play_slots = queue
        .and_then(|q| q.get("showQuickPlaySlotSelection"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let current = state.get(LOBBY);
    let mut lobby = if current.status == 200 {
        current.body
    } else {
        template(state)
    };
    if let Some(config) = lobby.get_mut("gameConfig").and_then(Value::as_object_mut) {
        config.insert("queueId".into(), json!(queue_id));
        config.insert("maxLobbySize".into(), json!(max_lobby_size));
        config.insert("showPositionSelector".into(), json!(show_position_selector));
        config.insert(
            "showQuickPlaySlotSelection".into(),
            json!(show_quick_play_slots),
        );
    }
    // Personalizada (#229): as equipes no `gameConfig`; numa fila comum, sem.
    if body.get("customGameLobby").is_some() {
        crate::custom::make_custom(&mut lobby);
    } else {
        crate::custom::make_queue(&mut lobby);
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

/// `POST` [`CUSTOM_START_CHAMP_SELECT`]: o mock aceita sem mudar nada (não
/// tem uma seleção de personalizada gravada); fora de lobby responde 404.
pub fn start_custom_champ_select(state: &LcuState) -> (Response, Vec<EventFixture>) {
    if state.get(LOBBY).status != 200 {
        return (Response::error(404, "mock-lcu: fora de lobby"), Vec::new());
    }
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        Vec::new(),
    )
}

/// `DELETE /lol-lobby/v2/lobby`: sai do lobby.
pub fn leave(state: &LcuState) -> (Response, Vec<EventFixture>) {
    let mut events = Vec::new();
    if state.get(LOBBY).status == 200 {
        events.push(event(LOBBY, "Delete", Value::Null));
    }
    // Sem lobby não há busca, nem a parada por punição.
    if state.get(SEARCH).status == 200 {
        events.push(event(SEARCH, "Delete", Value::Null));
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

/// `PUT` [`POSITION_PREFERENCES`] com `{"firstPreference": "MIDDLE",
/// "secondPreference": "JUNGLE"}`: troca os cargos do jogador logado no
/// `localMember` e na lista `members`. Fora de lobby responde 404, e numa
/// fila sem escolha de cargos, 400.
pub fn set_positions(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let current = state.get(LOBBY);
    if current.status != 200 {
        return (
            Response::error(404, "mock-lcu: LOBBY_NOT_FOUND"),
            Vec::new(),
        );
    }
    let preference = |name: &str| {
        body.get(name)
            .and_then(Value::as_str)
            .filter(|p| POSITIONS.contains(p))
    };
    let (Some(first), Some(second)) = (
        preference("firstPreference"),
        preference("secondPreference"),
    ) else {
        return (
            Response::error(
                400,
                "mock-lcu: o corpo precisa de \"firstPreference\" e \"secondPreference\" válidos",
            ),
            Vec::new(),
        );
    };
    let mut lobby = current.body;
    if lobby["gameConfig"]["showPositionSelector"] != json!(true) {
        return (
            Response::error(400, "mock-lcu: esta fila não tem escolha de cargos"),
            Vec::new(),
        );
    }
    let puuid = lobby["localMember"]["puuid"].clone();
    let set = |member: &mut Value| {
        if let Some(member) = member.as_object_mut() {
            member.insert("firstPositionPreference".into(), json!(first));
            member.insert("secondPositionPreference".into(), json!(second));
        }
    };
    set(&mut lobby["localMember"]);
    if let Some(members) = lobby.get_mut("members").and_then(Value::as_array_mut) {
        for member in members.iter_mut().filter(|m| m["puuid"] == puuid) {
            set(member);
        }
    }
    (
        Response {
            status: 201,
            body: Value::Null,
        },
        vec![event(LOBBY, "Update", lobby)],
    )
}

/// `PUT` [`PLAYER_SLOTS`] com a lista de slots (`championId`,
/// `positionPreference`, `skinId`, `spell1`, `spell2`, `perks`): troca os
/// `playerSlots` do jogador logado no `localMember` e em `members`. Fora de
/// lobby responde 404; numa fila sem slots, ou com um slot sem campeão ou
/// com posição inválida, 400.
pub fn set_player_slots(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let current = state.get(LOBBY);
    if current.status != 200 {
        return (
            Response::error(404, "mock-lcu: LOBBY_NOT_FOUND"),
            Vec::new(),
        );
    }
    let valid = body.as_array().is_some_and(|slots| {
        slots.iter().all(|slot| {
            slot["championId"].as_u64().is_some_and(|id| id > 0)
                && slot["positionPreference"]
                    .as_str()
                    .is_some_and(|p| POSITIONS[..5].contains(&p))
        })
    });
    if !valid {
        return (
            Response::error(400, "mock-lcu: slots inválidos"),
            Vec::new(),
        );
    }
    let mut lobby = current.body;
    if lobby["gameConfig"]["showQuickPlaySlotSelection"] != json!(true) {
        return (
            Response::error(400, "mock-lcu: esta fila não tem slots"),
            Vec::new(),
        );
    }
    let puuid = lobby["localMember"]["puuid"].clone();
    let set = |member: &mut Value| {
        if let Some(member) = member.as_object_mut() {
            member.insert("playerSlots".into(), body.clone());
        }
    };
    set(&mut lobby["localMember"]);
    if let Some(members) = lobby.get_mut("members").and_then(Value::as_array_mut) {
        for member in members.iter_mut().filter(|m| m["puuid"] == puuid) {
            set(member);
        }
    }
    (
        Response {
            status: 201,
            body: Value::Null,
        },
        vec![event(LOBBY, "Update", lobby)],
    )
}

/// `PUT` [`PARTY_TYPE`] com `"open"` ou `"closed"` (#225): troca o
/// `partyType` do lobby. Fora de lobby responde 404; sem ser o líder, 403; com
/// outro corpo, 400.
pub fn set_party_type(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let current = state.get(LOBBY);
    if current.status != 200 {
        return (
            Response::error(404, "mock-lcu: LOBBY_NOT_FOUND"),
            Vec::new(),
        );
    }
    let Some(party_type) = body.as_str().filter(|t| *t == "open" || *t == "closed") else {
        return (
            Response::error(400, "mock-lcu: o corpo precisa ser \"open\" ou \"closed\""),
            Vec::new(),
        );
    };
    let mut lobby = current.body;
    if lobby["localMember"]["isLeader"] != json!(true) {
        return (
            Response::error(403, "mock-lcu: só o líder troca"),
            Vec::new(),
        );
    }
    lobby["partyType"] = json!(party_type);
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        vec![event(LOBBY, "Update", lobby)],
    )
}

/// Lobby novo: o gravado, ou um mínimo com o invocador logado de líder.
pub fn template(state: &LcuState) -> Value {
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
    fn troca_os_cargos() {
        let mut s = state("16.22");
        let cargos = json!({"firstPreference": "MIDDLE", "secondPreference": "JUNGLE"});
        // Fora de lobby.
        assert_eq!(set_positions(&s, &cargos).0.status, 404);

        let (_, events) = create(&s, &json!({"queueId": 420}));
        apply(&mut s, &events);
        assert_eq!(
            s.get(LOBBY).body["gameConfig"]["showPositionSelector"],
            true
        );
        let (response, events) = set_positions(&s, &cargos);
        assert_eq!(response.status, 201);
        assert_eq!(events.len(), 1);
        apply(&mut s, &events);
        let lobby = s.get(LOBBY).body;
        assert_eq!(lobby["localMember"]["firstPositionPreference"], "MIDDLE");
        assert_eq!(lobby["members"][0]["firstPositionPreference"], "MIDDLE");
        assert_eq!(lobby["members"][0]["secondPositionPreference"], "JUNGLE");

        // Cargo que não existe.
        let (response, events) = set_positions(
            &s,
            &json!({"firstPreference": "MEIO", "secondPreference": "JUNGLE"}),
        );
        assert_eq!(response.status, 400);
        assert!(events.is_empty());

        // ARAM não tem cargos.
        let (_, events) = create(&s, &json!({"queueId": 450}));
        apply(&mut s, &events);
        assert_eq!(
            s.get(LOBBY).body["gameConfig"]["showPositionSelector"],
            false
        );
        assert_eq!(set_positions(&s, &cargos).0.status, 400);
    }

    #[test]
    fn troca_os_slots_do_jogo_dinamico() {
        let mut s = state("16.22");
        let slots = json!([
            {"championId": 22, "positionPreference": "BOTTOM", "skinId": 22000, "spell1": 4, "spell2": 7, "perks": ""},
            {"championId": 266, "positionPreference": "TOP", "skinId": 266000, "spell1": 4, "spell2": 12, "perks": ""},
        ]);
        assert_eq!(set_player_slots(&s, &slots).0.status, 404);

        // A Solo/Duo não tem slots.
        let (_, events) = create(&s, &json!({"queueId": 420}));
        apply(&mut s, &events);
        assert_eq!(set_player_slots(&s, &slots).0.status, 400);

        let (_, events) = create(&s, &json!({"queueId": 480}));
        apply(&mut s, &events);
        assert_eq!(
            s.get(LOBBY).body["gameConfig"]["showQuickPlaySlotSelection"],
            true
        );
        let (response, events) = set_player_slots(&s, &slots);
        assert_eq!(response.status, 201);
        apply(&mut s, &events);
        let lobby = s.get(LOBBY).body;
        assert_eq!(lobby["localMember"]["playerSlots"], slots);
        assert_eq!(lobby["members"][0]["playerSlots"][1]["championId"], 266);

        // Preenchimento não vale num slot.
        let fill = json!([{"championId": 22, "positionPreference": "FILL"}]);
        assert_eq!(set_player_slots(&s, &fill).0.status, 400);
    }

    #[test]
    fn abre_e_fecha_o_lobby() {
        let mut s = state("16.22");
        assert_eq!(set_party_type(&s, &json!("open")).0.status, 404);
        let (_, events) = create(&s, &json!({"queueId": 400}));
        apply(&mut s, &events);
        assert_eq!(s.get(LOBBY).body["partyType"], "closed");
        let (response, events) = set_party_type(&s, &json!("open"));
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        assert_eq!(s.get(LOBBY).body["partyType"], "open");
        assert_eq!(set_party_type(&s, &json!("aberto")).0.status, 400);
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

    #[test]
    fn modo_de_treino_cria_a_personalizada_e_inicia() {
        let mut s = state("16.22");
        assert_eq!(start_custom_champ_select(&s).0.status, 404);
        let body = json!({
            "customGameLobby": {"configuration": {"gameMode": "PRACTICETOOL", "mapId": 11}},
            "isCustom": true
        });
        let (response, events) = create(&s, &body);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["gameConfig"]["queueId"], 3140);
        apply(&mut s, &events);
        assert_eq!(start_custom_champ_select(&s).0.status, 204);
    }
}
