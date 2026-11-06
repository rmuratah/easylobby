//! Feitiços de invocador na LCU falsa (#24).
//!
//! - `PATCH /lol-champ-select/v1/session/my-selection` com
//!   `{"spell1Id": N, "spell2Id": M}` troca os feitiços do jogador logado na
//!   sessão atual e manda o `Update` dela pelo WAMP, como a LCU. Um dos dois
//!   campos sozinho troca só aquele.
//! - `GET /lol-game-data/assets/v1/summoner-spells.json` sai da gravação ou,
//!   enquanto ela não tiver, do arquivo **sintético** em `mock-lcu/data/`.
//!
//! Como em [`crate::champ_select`], as funções daqui não mexem no estado:
//! devolvem a resposta e os eventos, e quem chama emite os eventos.

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::champ_select::SESSION;
use crate::state::{LcuState, Response};

/// Escolhas do jogador logado na seleção.
pub const MY_SELECTION: &str = "/lol-champ-select/v1/session/my-selection";

/// Os feitiços do jogo.
pub const SUMMONER_SPELLS: &str = "/lol-game-data/assets/v1/summoner-spells.json";

const SYNTHETIC_SUMMONER_SPELLS: &str = include_str!("../data/summoner-spells.json");

/// `GET` de `summoner-spells.json`: o gravado, ou o sintético.
pub fn game_data(state: &LcuState) -> Response {
    let recorded = state.get(SUMMONER_SPELLS);
    if recorded.status == 200 {
        return recorded;
    }
    Response {
        status: 200,
        body: serde_json::from_str(SYNTHETIC_SUMMONER_SPELLS)
            .expect("dados sintéticos são JSON válido"),
    }
}

/// `PATCH .../my-selection`: troca os feitiços do jogador logado. 404 fora
/// da seleção; 400 sem nenhum dos dois campos.
pub fn select(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let current = state.get(SESSION);
    if current.status != 200 {
        return (Response::error(404, "No active delegate"), Vec::new());
    }
    let spell1 = body["spell1Id"].as_i64();
    let spell2 = body["spell2Id"].as_i64();
    if spell1.is_none() && spell2.is_none() {
        return (
            Response::error(400, "mock-lcu: corpo sem spell1Id nem spell2Id"),
            Vec::new(),
        );
    }
    let mut session = current.body;
    let cell = session["localPlayerCellId"].clone();
    let Some(player) = session["myTeam"]
        .as_array_mut()
        .and_then(|team| team.iter_mut().find(|p| p["cellId"] == cell))
    else {
        return (
            Response::error(404, "mock-lcu: jogador fora do time"),
            Vec::new(),
        );
    };
    if let Some(id) = spell1 {
        player["spell1Id"] = json!(id);
    }
    if let Some(id) = spell2 {
        player["spell2Id"] = json!(id);
    }
    let event = EventFixture {
        t_ms: 0,
        uri: SESSION.into(),
        event_type: "Update".into(),
        data: session,
    };
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        vec![event],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use easylobby_agent::fixtures::FixtureSet;

    fn empty() -> LcuState {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../agent/tests/fixtures/exemplo");
        LcuState::new(&FixtureSet::load(&dir).unwrap())
    }

    fn state() -> LcuState {
        let mut state = empty();
        state.apply(&EventFixture {
            t_ms: 0,
            uri: SESSION.into(),
            event_type: "Create".into(),
            data: json!({
                "localPlayerCellId": 1,
                "myTeam": [
                    {"cellId": 0, "spell1Id": 4, "spell2Id": 7},
                    {"cellId": 1, "spell1Id": 4, "spell2Id": 14},
                ],
            }),
        });
        state
    }

    #[test]
    fn troca_os_feiticos_do_jogador() {
        let mut state = state();
        let (response, events) = select(&state, &json!({"spell1Id": 12, "spell2Id": 4}));
        assert_eq!(response.status, 204);
        for e in &events {
            state.apply(e);
        }
        let session = state.get(SESSION).body;
        assert_eq!(session["myTeam"][1]["spell1Id"], 12);
        assert_eq!(session["myTeam"][1]["spell2Id"], 4);
        // O aliado não muda.
        assert_eq!(session["myTeam"][0]["spell1Id"], 4);

        let (_, events) = select(&state, &json!({"spell2Id": 11}));
        for e in &events {
            state.apply(e);
        }
        let session = state.get(SESSION).body;
        assert_eq!(session["myTeam"][1]["spell1Id"], 12);
        assert_eq!(session["myTeam"][1]["spell2Id"], 11);
    }

    #[test]
    fn recusa_sem_campos_e_fora_da_selecao() {
        assert_eq!(select(&state(), &json!({})).0.status, 400);
        let (response, events) = select(&empty(), &json!({"spell1Id": 4}));
        assert_eq!(response.status, 404);
        assert!(events.is_empty());
    }

    #[test]
    fn dados_sinteticos_sem_gravacao() {
        let response = game_data(&empty());
        assert_eq!(response.status, 200);
        let spells = response.body.as_array().unwrap();
        assert!(spells.iter().any(|s| s["id"] == 4 && s["name"] == "Flash"));
    }
}
