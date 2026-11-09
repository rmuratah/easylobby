//! Ações da seleção de campeões na LCU falsa (#23): hover, ban e pick.
//!
//! - `PATCH /lol-champ-select/v1/session/actions/{id}` com
//!   `{"championId": N}` marca o campeão na ação. Num pick do jogador logado
//!   ainda fora da vez, vira a intenção (`championPickIntent`); na vez, o
//!   campeão aparece em `championId` do jogador.
//! - `POST /lol-champ-select/v1/session/actions/{id}/complete` confirma a
//!   ação em andamento.
//!
//! Os dois mudam a sessão atual e mandam o `Update` dela pelo WAMP, como a
//! LCU. O draft não anda sozinho depois disso: a vez seguinte só vem da
//! reprodução da gravação.
//!
//! Como em [`crate::lobby`], as funções daqui não mexem no estado: devolvem a
//! resposta e os eventos, e quem chama emite os eventos.

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::state::{LcuState, Response};

/// Sessão da seleção de campeões.
pub const SESSION: &str = "/lol-champ-select/v1/session";

/// Prefixo das ações da sessão.
const ACTIONS_PREFIX: &str = "/lol-champ-select/v1/session/actions/";

/// Uma rota de ação: `actions/{id}` ou `actions/{id}/complete`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRoute {
    Action(u64),
    Complete(u64),
}

/// Lê o caminho de uma rota de ação, ou `None` se não for uma.
pub fn parse_route(path: &str) -> Option<ActionRoute> {
    let rest = path.strip_prefix(ACTIONS_PREFIX)?;
    let (id, complete) = match rest.split_once('/') {
        Some((id, "complete")) => (id, true),
        Some(_) => return None,
        None => (rest, false),
    };
    let id = id.parse().ok()?;
    Some(if complete {
        ActionRoute::Complete(id)
    } else {
        ActionRoute::Action(id)
    })
}

/// `PATCH .../actions/{id}`: marca o campeão do corpo na ação. Com
/// `completed: true` no corpo, também confirma, como a LCU (#311).
pub fn hover(state: &LcuState, action_id: u64, body: &Value) -> (Response, Vec<EventFixture>) {
    let Some(champion) = body["championId"].as_i64() else {
        return (
            Response::error(400, "mock-lcu: corpo sem championId"),
            Vec::new(),
        );
    };
    let complete_too = body["completed"] == true;
    change(state, action_id, |session, action| {
        mark_action(session, action, champion)?;
        if complete_too {
            complete_action(session, action)?;
        }
        Ok(())
    })
}

/// `POST .../actions/{id}/complete`: confirma a ação em andamento.
pub fn complete(state: &LcuState, action_id: u64) -> (Response, Vec<EventFixture>) {
    change(state, action_id, complete_action)
}

fn mark_action(session: &mut Value, action: &mut Value, champion: i64) -> Result<(), Response> {
    if action["completed"] == true {
        return Err(Response::error(400, "Action has already been completed"));
    }
    action["championId"] = json!(champion);
    if action["type"] == "pick" {
        let in_progress = action["isInProgress"] == true;
        if let Some(player) = local_player(session, &action["actorCellId"]) {
            if in_progress {
                player["championId"] = json!(champion);
            } else {
                player["championPickIntent"] = json!(champion);
            }
        }
    }
    Ok(())
}

fn complete_action(session: &mut Value, action: &mut Value) -> Result<(), Response> {
    if action["completed"] == true {
        return Err(Response::error(400, "Action has already been completed"));
    }
    if action["isInProgress"] != true {
        return Err(Response::error(400, "Action is not in progress"));
    }
    let champion = action["championId"].as_i64().unwrap_or(0);
    if champion <= 0 {
        return Err(Response::error(400, "No champion selected"));
    }
    action["completed"] = json!(true);
    action["isInProgress"] = json!(false);
    if action["type"] == "pick" {
        if let Some(player) = local_player(session, &action["actorCellId"]) {
            player["championId"] = json!(champion);
            player["championPickIntent"] = json!(0);
        }
    }
    Ok(())
}

/// Aplica `f` na ação `action_id` da sessão atual (e na sessão) e devolve o
/// `Update` da sessão nova. 404 fora da seleção ou com ação desconhecida,
/// como a LCU.
fn change(
    state: &LcuState,
    action_id: u64,
    f: impl FnOnce(&mut Value, &mut Value) -> Result<(), Response>,
) -> (Response, Vec<EventFixture>) {
    let current = state.get(SESSION);
    if current.status != 200 {
        return (Response::error(404, "No active delegate"), Vec::new());
    }
    let mut session = current.body;
    let Some((group, index)) = find_action(&session, action_id) else {
        return (
            Response::error(404, format!("Unable to find action {action_id}")),
            Vec::new(),
        );
    };
    let mut action = session["actions"][group][index].take();
    let result = f(&mut session, &mut action);
    session["actions"][group][index] = action;
    match result {
        Ok(()) => (no_content(), vec![event(SESSION, "Update", session)]),
        Err(error) => (error, Vec::new()),
    }
}

fn find_action(session: &Value, action_id: u64) -> Option<(usize, usize)> {
    let groups = session["actions"].as_array()?;
    groups.iter().enumerate().find_map(|(g, group)| {
        let index = group
            .as_array()?
            .iter()
            .position(|a| a["id"].as_u64() == Some(action_id))?;
        Some((g, index))
    })
}

/// O jogador da célula `cell` em `myTeam`, se for o jogador logado.
fn local_player<'a>(session: &'a mut Value, cell: &Value) -> Option<&'a mut Value> {
    if session["localPlayerCellId"] != *cell {
        return None;
    }
    session["myTeam"]
        .as_array_mut()?
        .iter_mut()
        .find(|p| p["cellId"] == *cell)
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
    use easylobby_agent::fixtures::FixtureSet;

    /// A gravação de exemplo, que não tem seleção de campeões.
    fn empty() -> LcuState {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../agent/tests/fixtures/exemplo");
        LcuState::new(&FixtureSet::load(&dir).unwrap())
    }

    /// Estado com uma sessão: o jogador (célula 1) tem um ban feito, o pick
    /// 13 em andamento e o pick 15 depois.
    fn state() -> LcuState {
        let mut state = empty();
        state.apply(&event(
            SESSION,
            "Create",
            json!({
                "localPlayerCellId": 1,
                "myTeam": [
                    {"cellId": 0, "championId": 0, "championPickIntent": 0},
                    {"cellId": 1, "championId": 0, "championPickIntent": 0},
                ],
                "actions": [
                    [{"id": 1, "type": "ban", "actorCellId": 1, "championId": 43,
                      "completed": true, "isInProgress": false}],
                    [{"id": 13, "type": "pick", "actorCellId": 1, "championId": 0,
                      "completed": false, "isInProgress": true}],
                    [{"id": 15, "type": "pick", "actorCellId": 1, "championId": 0,
                      "completed": false, "isInProgress": false}],
                ],
            }),
        ));
        state
    }

    fn apply(state: &mut LcuState, events: &[EventFixture]) {
        for e in events {
            state.apply(e);
        }
    }

    #[test]
    fn le_as_rotas() {
        assert_eq!(
            parse_route("/lol-champ-select/v1/session/actions/13"),
            Some(ActionRoute::Action(13))
        );
        assert_eq!(
            parse_route("/lol-champ-select/v1/session/actions/13/complete"),
            Some(ActionRoute::Complete(13))
        );
        assert_eq!(parse_route("/lol-champ-select/v1/session/actions/x"), None);
        assert_eq!(
            parse_route("/lol-champ-select/v1/session/actions/1/outra"),
            None
        );
        assert_eq!(parse_route("/lol-champ-select/v1/session"), None);
    }

    #[test]
    fn hover_e_confirma_o_pick() {
        let mut state = state();
        let (response, events) = hover(&state, 13, &json!({"championId": 121}));
        assert_eq!(response.status, 204);
        apply(&mut state, &events);
        let session = state.get(SESSION).body;
        assert_eq!(session["actions"][1][0]["championId"], 121);
        assert_eq!(session["actions"][1][0]["completed"], false);
        assert_eq!(session["myTeam"][1]["championId"], 121);

        let (response, events) = complete(&state, 13);
        assert_eq!(response.status, 204);
        apply(&mut state, &events);
        let session = state.get(SESSION).body;
        assert_eq!(session["actions"][1][0]["completed"], true);
        assert_eq!(session["actions"][1][0]["isInProgress"], false);
        assert_eq!(session["myTeam"][1]["championId"], 121);

        // Confirmada, não muda mais.
        assert_eq!(complete(&state, 13).0.status, 400);
        assert_eq!(hover(&state, 13, &json!({"championId": 1})).0.status, 400);
    }

    #[test]
    fn hover_fora_da_vez_vira_intencao() {
        let mut state = state();
        let (response, events) = hover(&state, 15, &json!({"championId": 64}));
        assert_eq!(response.status, 204);
        apply(&mut state, &events);
        let session = state.get(SESSION).body;
        assert_eq!(session["myTeam"][1]["championPickIntent"], 64);
        assert_eq!(session["myTeam"][1]["championId"], 0);
        // Fora da vez, não confirma.
        assert_eq!(complete(&state, 15).0.status, 400);
    }

    #[test]
    fn recusa_sem_campeao_sem_acao_e_fora_da_selecao() {
        let state = state();
        assert_eq!(complete(&state, 13).0.status, 400);
        assert_eq!(hover(&state, 13, &json!({})).0.status, 400);
        assert_eq!(hover(&state, 99, &json!({"championId": 1})).0.status, 404);
        assert_eq!(complete(&state, 1).0.status, 400);

        let empty = empty();
        assert_eq!(hover(&empty, 13, &json!({"championId": 1})).0.status, 404);
        assert!(complete(&empty, 13).1.is_empty());
    }
}
