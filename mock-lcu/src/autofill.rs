//! Autofill na seleção de campeões na LCU falsa (#360).
//!
//! - `GET` [`DISPLAYED`] diz se o aviso de autofill já foi mostrado
//!   (`false` até alguém marcar).
//! - `PUT` [`DISPLAYED`] marca o aviso como mostrado e responde 204.
//! - [`SIMULATE`] (rota só do mock) põe o jogador logado em autofill na
//!   seleção atual (`isAutofilled` verdadeiro) e desmarca o aviso, para a UI
//!   ter um aviso para mostrar. A gravação 16.22 não tem autofill.
//!
//! Como em [`crate::champ_select`], as funções daqui não mexem no estado:
//! devolvem a resposta e os eventos, e quem chama emite os eventos.

use easylobby_agent::champ_select::AUTOFILL_DISPLAYED_PATH;
use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::champ_select::SESSION;
use crate::state::{LcuState, Response};

/// Se o aviso de autofill já foi mostrado.
pub const DISPLAYED: &str = AUTOFILL_DISPLAYED_PATH;

/// Rota só do mock: o jogador logado cai em autofill.
pub const SIMULATE: &str = "/mock-lcu/autofill";

/// `GET` [`DISPLAYED`]: `false` até um `PUT`.
pub fn displayed(state: &LcuState) -> Response {
    let current = state.get(DISPLAYED);
    let body = if current.status == 200 {
        current.body
    } else {
        json!(false)
    };
    Response { status: 200, body }
}

/// `PUT` [`DISPLAYED`]: marca o aviso como mostrado.
pub fn mark_displayed() -> (Response, Vec<EventFixture>) {
    (no_content(), vec![event(DISPLAYED, json!(true))])
}

/// [`SIMULATE`]: o jogador logado fica em autofill na seleção atual. Fora
/// da seleção responde 404.
pub fn simulate(state: &LcuState) -> (Response, Vec<EventFixture>) {
    let current = state.get(SESSION);
    if current.status != 200 {
        return (Response::error(404, "No active delegate"), Vec::new());
    }
    let mut session = current.body;
    let cell = session["localPlayerCellId"].clone();
    let Some(me) = session["myTeam"]
        .as_array_mut()
        .and_then(|team| team.iter_mut().find(|p| p["cellId"] == cell))
    else {
        return (
            Response::error(404, "mock-lcu: o jogador não está na seleção"),
            Vec::new(),
        );
    };
    me["isAutofilled"] = json!(true);
    let body = me.clone();
    (
        Response { status: 200, body },
        vec![event(DISPLAYED, json!(false)), event(SESSION, session)],
    )
}

fn no_content() -> Response {
    Response {
        status: 204,
        body: Value::Null,
    }
}

fn event(uri: &str, data: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.into(),
        event_type: "Update".into(),
        data,
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

    fn apply(state: &mut LcuState, events: &[EventFixture]) {
        for event in events {
            state.apply(event);
        }
    }

    #[test]
    fn marca_o_aviso_e_poe_o_jogador_em_autofill() {
        let mut s = empty();
        assert_eq!(displayed(&s).body, json!(false));
        assert_eq!(simulate(&s).0.status, 404);

        apply(
            &mut s,
            &[event(
                SESSION,
                json!({
                    "localPlayerCellId": 1,
                    "myTeam": [
                        {"cellId": 0, "isAutofilled": false},
                        {"cellId": 1, "isAutofilled": false},
                    ],
                }),
            )],
        );
        let (response, events) = mark_displayed();
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        assert_eq!(displayed(&s).body, json!(true));

        let (response, events) = simulate(&s);
        assert_eq!(response.status, 200);
        apply(&mut s, &events);
        let session = s.get(SESSION).body;
        assert_eq!(session["myTeam"][0]["isAutofilled"], false);
        assert_eq!(session["myTeam"][1]["isAutofilled"], true);
        assert_eq!(displayed(&s).body, json!(false));
    }
}
