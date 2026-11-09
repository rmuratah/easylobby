//! Personalizadas abertas na LCU falsa (#264): a lista e entrar numa delas.
//!
//! - `GET` [`CUSTOM_GAMES`] responde a lista gravada, se houver; senão, a
//!   lista de [`games`], com uma aberta, uma com senha e uma cheia.
//! - `POST` [`REFRESH`] responde 204 e manda a lista pelo WAMP, como a LCU
//!   faz quando a lista nova chega do servidor.
//! - `POST` `/lol-lobby/v2/party/{partyId}/join` com `{"lobbyPassword"}`
//!   confere a senha ([`PASSWORD`] na partida com senha) e as vagas e põe o
//!   jogador no lobby da partida, sem ser o líder. Senha errada responde 403;
//!   partida cheia, 409; `partyId` que não está na lista, 404.
//!
//! O formato da lista segue a gravação do LoL real (#264): `id` 0 em todas,
//! `gameType` `"CUSTOM"`, `spectatorPolicy` `"ALL"`/`"NONE"` e o `partyId`
//! de cada partida. As respostas de erro do `join` são suposições do mock.

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::lobby::{create, custom_queue_id, LOBBY, QUEUES};
use crate::state::{LcuState, Response};

/// A lista de personalizadas abertas.
pub const CUSTOM_GAMES: &str = "/lol-lobby/v1/custom-games";

/// Pedir a lista de novo.
pub const REFRESH: &str = "/lol-lobby/v1/custom-games/refresh";

/// Senha da personalizada com senha da lista do mock.
pub const PASSWORD: &str = "senha";

/// Fila personalizada do Summoner's Rift às cegas, quando a gravação não
/// tem a lista de filas.
const FALLBACK_QUEUE: u64 = 3100;

/// A lista do mock: uma aberta no Summoner's Rift, uma com senha no Abismo
/// Uivante e uma cheia.
pub fn games() -> Value {
    json!([
        {
            "filledPlayerSlots": 3, "filledSpectatorSlots": 0, "gameType": "CUSTOM",
            "hasPassword": false, "id": 0, "lobbyName": "5v5 da firma", "mapId": 11,
            "maxPlayerSlots": 10, "maxSpectatorSlots": 15, "ownerDisplayName": "Ahri Mid #BR1",
            "partyId": "mock-party-1001", "passbackUrl": "", "spectatorPolicy": "ALL"
        },
        {
            "filledPlayerSlots": 2, "filledSpectatorSlots": 1, "gameType": "CUSTOM",
            "hasPassword": true, "id": 0, "lobbyName": "ARAM fechado", "mapId": 12,
            "maxPlayerSlots": 10, "maxSpectatorSlots": 15, "ownerDisplayName": "Garen Top #BR1",
            "partyId": "mock-party-1002", "passbackUrl": "", "spectatorPolicy": "ALL"
        },
        {
            "filledPlayerSlots": 2, "filledSpectatorSlots": 0, "gameType": "CUSTOM",
            "hasPassword": false, "id": 0, "lobbyName": "1v1 no meio", "mapId": 11,
            "maxPlayerSlots": 2, "maxSpectatorSlots": 15, "ownerDisplayName": "Yasuo #0010",
            "partyId": "mock-party-1003", "passbackUrl": "", "spectatorPolicy": "NONE"
        }
    ])
}

/// `GET` [`CUSTOM_GAMES`].
pub fn list(state: &LcuState) -> Response {
    let recorded = state.get(CUSTOM_GAMES);
    if recorded.status == 200 {
        return recorded;
    }
    Response {
        status: 200,
        body: games(),
    }
}

/// `POST` [`REFRESH`].
pub fn refresh(state: &LcuState) -> (Response, Vec<EventFixture>) {
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        vec![event(CUSTOM_GAMES, "Update", list(state).body)],
    )
}

/// Começo do caminho de entrar no grupo de uma partida.
const PARTY: &str = "/lol-lobby/v2/party/";

/// O `partyId` de `/lol-lobby/v2/party/{partyId}/join`, ou `None` se `path`
/// não é esse caminho.
pub fn join_route(path: &str) -> Option<&str> {
    path.strip_prefix(PARTY)?
        .strip_suffix("/join")
        .filter(|id| !id.is_empty() && !id.contains('/'))
}

/// `POST` `/lol-lobby/v2/party/{partyId}/join`.
pub fn join(state: &LcuState, party_id: &str, body: &Value) -> (Response, Vec<EventFixture>) {
    let listing = list(state).body;
    let Some(game) = listing
        .as_array()
        .and_then(|games| games.iter().find(|g| g["partyId"] == json!(party_id)))
        .cloned()
    else {
        return error(404, "PARTY_NOT_FOUND");
    };
    let password = body["lobbyPassword"].as_str().unwrap_or_default();
    if game["hasPassword"] == json!(true) && password != PASSWORD {
        return error(403, "WRONG_PASSWORD");
    }
    let count = |key: &str| game[key].as_u64().unwrap_or(0);
    if count("filledPlayerSlots") >= count("maxPlayerSlots") {
        return error(409, "CUSTOM_GAME_FULL");
    }

    let map_id = game["mapId"].as_u64().unwrap_or(11);
    // A lista não diz o tipo de seleção: o mock usa às cegas.
    let mutator = 19;
    let spectators = match game["spectatorPolicy"].as_str() {
        Some("NONE") => "NotAllowed",
        _ => "AllAllowed",
    };
    let mode = match map_id {
        12 => "ARAM",
        453 => "JADE",
        _ => "CLASSIC",
    };
    let mut body = json!({
        "customGameLobby": {
            "configuration": {
                "gameMode": mode,
                "mapId": map_id,
                "mutators": {"id": mutator},
                "spectatorPolicy": spectators,
                "teamSize": (count("maxPlayerSlots") / 2).max(1)
            },
            "lobbyName": game["lobbyName"]
        },
        "isCustom": true
    });
    // A fila personalizada do mesmo mapa e tipo de seleção, como ao criar.
    // Sem ela na gravação, qualquer fila personalizada gravada, ou qualquer
    // fila (o conjunto `exemplo` só tem duas filas comuns).
    let queue_id = custom_queue_id(state, &body)
        .or_else(|| recorded_queue(state, true))
        .or_else(|| recorded_queue(state, false))
        .unwrap_or(FALLBACK_QUEUE);
    body["queueId"] = json!(queue_id);
    let (created, mut events) = create(state, &body);
    if created.status != 200 {
        return (created, events);
    }
    // Quem entra não é o líder: o dono da partida é.
    if let Some(lobby) = events.iter_mut().rev().find(|e| e.uri == LOBBY) {
        let lobby = &mut lobby.data;
        lobby["gameConfig"]["gameMode"] = json!(mode);
        lobby["localMember"]["isLeader"] = json!(false);
        if let Some(members) = lobby["members"].as_array_mut() {
            for member in members {
                member["isLeader"] = json!(false);
            }
        }
        if let Some(team) = lobby["gameConfig"]["customTeam100"].as_array_mut() {
            for member in team.iter_mut() {
                member["isLeader"] = json!(false);
            }
        }
    }
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        events,
    )
}

/// A primeira fila da lista de filas gravada; com `custom`, a primeira
/// personalizada.
fn recorded_queue(state: &LcuState, custom: bool) -> Option<u64> {
    state
        .recorded_body(QUEUES)?
        .as_array()?
        .iter()
        .find(|q| !custom || q["isCustom"] == json!(true))?
        .get("id")?
        .as_u64()
}

fn error(status: u16, message: &str) -> (Response, Vec<EventFixture>) {
    (
        Response::error(status, format!("mock-lcu: {message}")),
        Vec::new(),
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
    use easylobby_agent::custom_game_list::join_body;
    use easylobby_agent::fixtures::FixtureSet;

    fn state() -> LcuState {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
        LcuState::new(&FixtureSet::load(&dir).unwrap())
    }

    #[test]
    fn rota_de_entrar() {
        assert_eq!(
            join_route("/lol-lobby/v2/party/mock-party-1001/join"),
            Some("mock-party-1001")
        );
        assert_eq!(join_route("/lol-lobby/v2/party//join"), None);
        assert_eq!(join_route("/lol-lobby/v1/custom-games/refresh"), None);
    }

    #[test]
    fn entra_conferindo_senha_e_vagas() {
        let s = state();
        let status = |id, body| join(&s, id, &body).0.status;
        assert_eq!(status("outro", json!({})), 404);
        assert_eq!(status("mock-party-1002", join_body("errada")), 403);
        assert_eq!(status("mock-party-1003", json!({})), 409);
        let (response, events) = join(&s, "mock-party-1002", &join_body(PASSWORD));
        assert_eq!(response.status, 204);
        let lobby = &events.last().unwrap().data;
        assert_eq!(lobby["gameConfig"]["customLobbyName"], "ARAM fechado");
        assert_eq!(lobby["gameConfig"]["mapId"], 12);
        assert_eq!(lobby["gameConfig"]["isCustom"], true);
        assert_eq!(lobby["localMember"]["isLeader"], false);
    }
}
