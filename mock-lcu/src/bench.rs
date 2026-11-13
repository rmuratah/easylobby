//! Reroll e banco do ARAM na LCU falsa (#201).
//!
//! - `POST /lol-champ-select/v1/session/my-selection/reroll` sorteia outro
//!   campeão para o jogador logado (o primeiro do jogo que não está no time
//!   nem no banco), põe o antigo no banco e gasta um reroll.
//! - `POST /lol-champ-select/v1/session/bench/swap/{id}` troca o campeão do
//!   jogador pelo `id` do banco; o dele vai para o banco no lugar. Campeão
//!   fora de `pickable-champion-ids` (o jogador não tem), 400.
//!
//! Os dois mandam o `Update` da sessão pelo WAMP. Fora da seleção, 404;
//! sessão sem banco, sem reroll sobrando ou campeão fora do banco, 400.
//!
//! Como em [`crate::champ_select`], as funções daqui não mexem no estado:
//! devolvem a resposta e os eventos, e quem chama emite os eventos.

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::champ_select::SESSION;
use crate::champions;
use crate::state::{LcuState, Response};

/// Rota do reroll.
pub const REROLL: &str = "/lol-champ-select/v1/session/my-selection/reroll";

const BENCH_SWAP_PREFIX: &str = "/lol-champ-select/v1/session/bench/swap/";

/// O campeão de uma rota `bench/swap/{id}`, ou `None` se não for uma.
pub fn bench_swap_route(path: &str) -> Option<u64> {
    path.strip_prefix(BENCH_SWAP_PREFIX)?.parse().ok()
}

/// `POST .../my-selection/reroll`.
pub fn reroll(state: &LcuState) -> (Response, Vec<EventFixture>) {
    let current = state.get(SESSION);
    if current.status != 200 {
        return (Response::error(404, "No active delegate"), Vec::new());
    }
    let mut session = current.body;
    if session["allowRerolling"] != true {
        return (Response::error(400, "Rerolling is not allowed"), Vec::new());
    }
    let remaining = session["rerollsRemaining"].as_i64().unwrap_or(0);
    if remaining <= 0 {
        return (Response::error(400, "No rerolls remaining"), Vec::new());
    }
    let taken: Vec<i64> = session["myTeam"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["championId"].as_i64())
        .chain(bench_ids(&session))
        .collect();
    let catalog = champions::game_data(state).body;
    let Some(next) = catalog
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["id"].as_i64())
        .find(|&id| id > 0 && !taken.contains(&id))
    else {
        return (
            Response::error(400, "mock-lcu: sem campeão para sortear"),
            Vec::new(),
        );
    };
    let Some(old) = set_local_champion(&mut session, next) else {
        return (
            Response::error(404, "mock-lcu: jogador fora do time"),
            Vec::new(),
        );
    };
    push_bench(&mut session, old);
    session["rerollsRemaining"] = json!(remaining - 1);
    (no_content(), vec![update(session)])
}

/// `POST .../bench/swap/{id}`.
pub fn swap(state: &LcuState, champion_id: u64) -> (Response, Vec<EventFixture>) {
    let current = state.get(SESSION);
    if current.status != 200 {
        return (Response::error(404, "No active delegate"), Vec::new());
    }
    let mut session = current.body;
    if session["benchEnabled"] != true {
        return (Response::error(400, "Bench is not enabled"), Vec::new());
    }
    let wanted = i64::try_from(champion_id).unwrap_or(-1);
    let Some(bench) = session["benchChampions"].as_array_mut() else {
        return (
            Response::error(400, "Champion is not on the bench"),
            Vec::new(),
        );
    };
    let Some(index) = bench.iter().position(|c| c["championId"] == wanted) else {
        return (
            Response::error(400, "Champion is not on the bench"),
            Vec::new(),
        );
    };
    // Campeão que o jogador não tem (#446): a lista gravada ou a do ARAM
    // simulado diz quais ele pode escolher.
    let pickable = state.get(champions::PICKABLE);
    if pickable.status == 200
        && pickable
            .body
            .as_array()
            .is_some_and(|ids| !ids.contains(&json!(wanted)))
    {
        return (Response::error(400, "Champion is not owned"), Vec::new());
    }
    bench.remove(index);
    let Some(old) = set_local_champion(&mut session, wanted) else {
        return (
            Response::error(404, "mock-lcu: jogador fora do time"),
            Vec::new(),
        );
    };
    push_bench(&mut session, old);
    (no_content(), vec![update(session)])
}

fn bench_ids(session: &Value) -> Vec<i64> {
    session["benchChampions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["championId"].as_i64())
        .collect()
}

/// Troca o campeão (e a skin padrão) do jogador logado e devolve o antigo.
fn set_local_champion(session: &mut Value, champion: i64) -> Option<i64> {
    let cell = session["localPlayerCellId"].clone();
    let player = session["myTeam"]
        .as_array_mut()?
        .iter_mut()
        .find(|p| p["cellId"] == cell)?;
    let old = player["championId"].as_i64().unwrap_or(0);
    player["championId"] = json!(champion);
    player["selectedSkinId"] = json!(champion * 1000);
    Some(old)
}

fn push_bench(session: &mut Value, champion: i64) {
    if champion <= 0 {
        return;
    }
    if !session["benchChampions"].is_array() {
        session["benchChampions"] = json!([]);
    }
    if let Some(bench) = session["benchChampions"].as_array_mut() {
        bench.push(json!({"championId": champion, "isPriority": false}));
    }
}

fn no_content() -> Response {
    Response {
        status: 204,
        body: Value::Null,
    }
}

fn update(session: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: SESSION.into(),
        event_type: "Update".into(),
        data: session,
    }
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

    /// ARAM com o jogador (célula 0) de Aatrox (266) e Annie (1) no banco.
    fn state() -> LcuState {
        let mut state = empty();
        state.apply(&update(json!({
            "localPlayerCellId": 0,
            "myTeam": [{"cellId": 0, "championId": 266}, {"cellId": 1, "championId": 103}],
            "actions": [],
            "benchEnabled": true,
            "benchChampions": [{"championId": 1, "isPriority": false}],
            "allowRerolling": true,
            "rerollsRemaining": 1,
        })));
        state
    }

    fn apply(state: &mut LcuState, events: &[EventFixture]) {
        for e in events {
            state.apply(e);
        }
    }

    #[test]
    fn le_a_rota_do_banco() {
        assert_eq!(
            bench_swap_route("/lol-champ-select/v1/session/bench/swap/266"),
            Some(266)
        );
        assert_eq!(
            bench_swap_route("/lol-champ-select/v1/session/bench/swap/x"),
            None
        );
        assert_eq!(bench_swap_route(REROLL), None);
    }

    #[test]
    fn reroll_sorteia_e_poe_o_antigo_no_banco() {
        let mut state = state();
        let (response, events) = reroll(&state);
        assert_eq!(response.status, 204);
        apply(&mut state, &events);
        let session = state.get(SESSION).body;
        let mine = session["myTeam"][0]["championId"].as_i64().unwrap();
        assert!(![266, 103, 1, 0].contains(&mine));
        assert_eq!(bench_ids(&session), [1, 266]);
        assert_eq!(session["rerollsRemaining"], 0);
        // Sem reroll sobrando.
        assert_eq!(reroll(&state).0.status, 400);
    }

    #[test]
    fn troca_com_o_banco() {
        let mut state = state();
        let (response, events) = swap(&state, 1);
        assert_eq!(response.status, 204);
        apply(&mut state, &events);
        let session = state.get(SESSION).body;
        assert_eq!(session["myTeam"][0]["championId"], 1);
        assert_eq!(bench_ids(&session), [266]);
        assert_eq!(swap(&state, 103).0.status, 400);
    }

    #[test]
    fn fora_da_selecao() {
        let empty = empty();
        assert_eq!(reroll(&empty).0.status, 404);
        assert_eq!(swap(&empty, 1).0.status, 404);
    }
}
