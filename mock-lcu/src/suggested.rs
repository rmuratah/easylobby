//! Jogadores sugeridos para convidar na LCU falsa (#377).
//!
//! - `GET` [`SUGGESTED`] devolve a lista. A gravação 16.22 não tem
//!   sugeridos, então o mock monta dois a partir dos amigos gravados: um
//!   amigo de amigo (com o primeiro amigo como amigo em comum) e um que já
//!   jogou em grupo com o jogador. Sem amigos gravados, a lista vem vazia.
//! - `DELETE` [`SUGGESTED`]`/{summonerId}` dispensa o jogador: ele sai da
//!   lista e a resposta é 204. Quem não está na lista, 404.
//!
//! O formato segue o esquema da LCU (`summonerId`, `summonerName`,
//! `reason`, `commonFriendId`, `commonFriendName`); falta conferir no LoL
//! real.
//!
//! Como em [`crate::champ_select`], as funções daqui não mexem no estado:
//! devolvem a resposta e os eventos, e quem chama emite os eventos.

use easylobby_agent::fixtures::EventFixture;
use easylobby_agent::players::SUGGESTED_PLAYERS_PATH;
use serde_json::{json, Value};

use crate::state::{LcuState, Response};

/// Lista de sugeridos.
pub const SUGGESTED: &str = SUGGESTED_PLAYERS_PATH;

const FRIENDS: &str = "/lol-chat/v1/friends";

/// O `summonerId` de `DELETE` [`SUGGESTED`]`/{id}`, ou `None` se o caminho
/// não for um desses.
pub fn dismiss_route(path: &str) -> Option<u64> {
    path.strip_prefix(SUGGESTED)?
        .strip_prefix('/')?
        .parse()
        .ok()
}

/// `GET` [`SUGGESTED`].
pub fn list(state: &LcuState) -> Response {
    Response {
        status: 200,
        body: current(state),
    }
}

/// `DELETE` [`SUGGESTED`]`/{summonerId}`.
pub fn dismiss(state: &LcuState, summoner_id: u64) -> (Response, Vec<EventFixture>) {
    let mut players = current(state);
    let Some(list) = players.as_array_mut() else {
        return (Response::error(404, "mock-lcu: sem sugeridos"), Vec::new());
    };
    let before = list.len();
    list.retain(|p| p["summonerId"] != summoner_id);
    if list.len() == before {
        return (
            Response::error(
                404,
                format!("mock-lcu: {summoner_id} não está nos sugeridos"),
            ),
            Vec::new(),
        );
    }
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        vec![EventFixture {
            t_ms: 0,
            uri: SUGGESTED.into(),
            event_type: "Update".into(),
            data: players,
        }],
    )
}

/// A lista atual: a já mudada por um `DELETE` ou a montada dos amigos.
fn current(state: &LcuState) -> Value {
    let stored = state.get(SUGGESTED);
    if stored.status == 200 {
        return stored.body;
    }
    let friends = state.get(FRIENDS).body;
    let friends = friends.as_array().map(Vec::as_slice).unwrap_or_default();
    let Some(common) = friends.first() else {
        return json!([]);
    };
    let mut players = vec![json!({
        "summonerId": 9_000_001_u64,
        "summonerName": "",
        "gameName": "Amigo do Amigo",
        "tagLine": "BR1",
        "puuid": "mock-sugerido-1",
        "reason": "FRIEND_OF_FRIEND",
        "commonFriendId": common["summonerId"],
        "commonFriendName": common["gameName"],
    })];
    if let Some(premade) = friends.get(1) {
        players.push(json!({
            "summonerId": premade["summonerId"],
            "summonerName": "",
            "gameName": premade["gameName"],
            "tagLine": premade["gameTag"],
            "puuid": premade["puuid"],
            "reason": "PREVIOUS_PREMADE",
            "commonFriendId": 0,
            "commonFriendName": "",
        }));
    }
    Value::Array(players)
}

#[cfg(test)]
mod tests {
    use super::*;
    use easylobby_agent::fixtures::FixtureSet;

    fn state() -> LcuState {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
        LcuState::new(&FixtureSet::load(&dir).unwrap())
    }

    #[test]
    fn lista_e_dispensa() {
        let mut s = state();
        let body = list(&s).body;
        let players = body.as_array().unwrap();
        assert_eq!(players.len(), 2);
        assert_eq!(players[0]["reason"], "FRIEND_OF_FRIEND");
        let id = players[1]["summonerId"].as_u64().unwrap();

        assert_eq!(dismiss_route(&format!("{SUGGESTED}/{id}")), Some(id));
        assert_eq!(dismiss_route(SUGGESTED), None);
        let (response, events) = dismiss(&s, id);
        assert_eq!(response.status, 204);
        for event in &events {
            s.apply(event);
        }
        assert_eq!(list(&s).body.as_array().unwrap().len(), 1);
        assert_eq!(dismiss(&s, id).0.status, 404);
    }
}
