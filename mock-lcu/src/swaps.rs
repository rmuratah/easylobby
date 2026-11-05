//! Trocas na seleção de campeões na LCU falsa (#25): de campeão, de vez no
//! draft e de cargo.
//!
//! - `POST /lol-champ-select/v1/session/{champion-swaps|pick-order-swaps|position-swaps}/{id}/request`
//!   pede a troca: a entrada vai para `SENT` e as outras `AVAILABLE` da
//!   mesma lista ficam `INVALID`, como na gravação 16.22.
//! - `.../cancel` cancela o pedido do jogador (`SENT`).
//! - `.../accept` aceita o pedido de um aliado (`RECEIVED`) e faz a troca na
//!   sessão: o campeão (e a skin), a vez nos picks que faltam ou o cargo.
//! - `.../decline` recusa o pedido de um aliado.
//!
//! Depois de cancelar, aceitar ou recusar, as entradas da lista voltam para
//! `AVAILABLE`. O aliado nunca responde sozinho: um pedido do jogador fica
//! em `SENT` até ele cancelar ou a gravação mudar a sessão.
//!
//! [`SIMULATE`] (rota só do mock) faz o primeiro aliado com troca
//! `AVAILABLE` pedir uma troca ao jogador, para a UI ter um pedido para
//! aceitar ou recusar.
//!
//! Como em [`crate::champ_select`], as funções daqui não mexem no estado:
//! devolvem a resposta e os eventos, e quem chama emite os eventos.

use easylobby_agent::fixtures::EventFixture;
use easylobby_agent::swaps::{SwapIntent, SwapKind};
use serde_json::{json, Value};

use crate::champ_select::SESSION;
use crate::state::{LcuState, Response};

/// Rota só do mock: um aliado pede uma troca ao jogador. Corpo opcional
/// `{"kind": "champion" | "pickOrder" | "position"}` (padrão `champion`).
pub const SIMULATE: &str = "/mock-lcu/received-swap";

/// Lê uma rota de troca: tipo, id e o que fazer. `None` se não for uma.
pub fn parse_route(path: &str) -> Option<(SwapKind, u64, SwapIntent)> {
    let rest = path.strip_prefix(SESSION)?.strip_prefix('/')?;
    let mut parts = rest.split('/');
    let kind_path = parts.next()?;
    let kind = SwapKind::ALL
        .into_iter()
        .find(|k| k.lcu_path() == kind_path)?;
    let id = parts.next()?.parse().ok()?;
    let verb = parts.next()?;
    let intent = [
        SwapIntent::Request,
        SwapIntent::Accept,
        SwapIntent::Decline,
        SwapIntent::Cancel,
    ]
    .into_iter()
    .find(|i| i.lcu_verb() == verb)?;
    parts.next().is_none().then_some((kind, id, intent))
}

/// Nome da lista do tipo na sessão.
fn list_key(kind: SwapKind) -> &'static str {
    match kind {
        SwapKind::Champion => "trades",
        SwapKind::PickOrder => "pickOrderSwaps",
        SwapKind::Position => "positionSwaps",
    }
}

/// Responde a um pedido de troca. 404 fora da seleção ou com troca
/// desconhecida; 400 com a troca no estado errado.
pub fn respond(
    state: &LcuState,
    kind: SwapKind,
    swap_id: u64,
    intent: SwapIntent,
) -> (Response, Vec<EventFixture>) {
    let current = state.get(SESSION);
    if current.status != 200 {
        return (Response::error(404, "No active delegate"), Vec::new());
    }
    let mut session = current.body;
    let key = list_key(kind);
    let Some(index) = session[key]
        .as_array()
        .and_then(|list| list.iter().position(|s| s["id"].as_u64() == Some(swap_id)))
    else {
        return (
            Response::error(404, format!("Unable to find swap {swap_id}")),
            Vec::new(),
        );
    };
    let entry = session[key][index].clone();
    let needed = match intent {
        SwapIntent::Request => "AVAILABLE",
        SwapIntent::Accept | SwapIntent::Decline => "RECEIVED",
        SwapIntent::Cancel => "SENT",
    };
    if entry["state"] != needed {
        return (
            Response::error(400, format!("Swap {swap_id} is not {needed}")),
            Vec::new(),
        );
    }
    match intent {
        SwapIntent::Request => open_swap(&mut session[key], index, "SENT"),
        SwapIntent::Accept => {
            let other = entry["cellId"].clone();
            let local = session["localPlayerCellId"].clone();
            swap_in_session(&mut session, kind, &local, &other);
            reset(&mut session[key]);
        }
        SwapIntent::Decline | SwapIntent::Cancel => reset(&mut session[key]),
    }
    let contract = json!({"id": swap_id, "cellId": entry["cellId"], "state": needed});
    (
        Response {
            status: 200,
            body: contract,
        },
        vec![event(session)],
    )
}

/// `POST` [`SIMULATE`]: o primeiro aliado com troca `AVAILABLE` do tipo
/// pede a troca ao jogador. 404 fora da seleção ou sem troca disponível.
pub fn simulate(state: &LcuState, kind: SwapKind) -> (Response, Vec<EventFixture>) {
    let current = state.get(SESSION);
    if current.status != 200 {
        return (Response::error(404, "No active delegate"), Vec::new());
    }
    let mut session = current.body;
    let key = list_key(kind);
    let Some(index) = session[key]
        .as_array()
        .and_then(|list| list.iter().position(|s| s["state"] == "AVAILABLE"))
    else {
        return (
            Response::error(404, "mock-lcu: nenhuma troca disponível"),
            Vec::new(),
        );
    };
    open_swap(&mut session[key], index, "RECEIVED");
    let body = session[key][index].clone();
    (Response { status: 200, body }, vec![event(session)])
}

/// Põe a entrada `index` em `state` e as outras disponíveis em `INVALID`.
fn open_swap(list: &mut Value, index: usize, state: &str) {
    let Some(list) = list.as_array_mut() else {
        return;
    };
    for (i, entry) in list.iter_mut().enumerate() {
        if i == index {
            entry["state"] = json!(state);
        } else if entry["state"] == "AVAILABLE" {
            entry["state"] = json!("INVALID");
        }
    }
}

/// Volta todas as entradas da lista para `AVAILABLE`.
fn reset(list: &mut Value) {
    for entry in list.as_array_mut().into_iter().flatten() {
        entry["state"] = json!("AVAILABLE");
    }
}

/// Faz a troca aceita entre as células `a` e `b` do time do jogador.
fn swap_in_session(session: &mut Value, kind: SwapKind, a: &Value, b: &Value) {
    match kind {
        SwapKind::Champion => swap_fields(session, a, b, &["championId", "selectedSkinId"]),
        SwapKind::Position => swap_fields(session, a, b, &["assignedPosition"]),
        SwapKind::PickOrder => {
            let picks = session["actions"]
                .as_array_mut()
                .into_iter()
                .flatten()
                .filter_map(Value::as_array_mut)
                .flatten()
                .filter(|action| action["type"] == "pick" && action["completed"] != true);
            for action in picks {
                if action["actorCellId"] == *a {
                    action["actorCellId"] = b.clone();
                } else if action["actorCellId"] == *b {
                    action["actorCellId"] = a.clone();
                }
            }
        }
    }
}

fn swap_fields(session: &mut Value, a: &Value, b: &Value, fields: &[&str]) {
    let Some(team) = session["myTeam"].as_array_mut() else {
        return;
    };
    let (Some(ia), Some(ib)) = (
        team.iter().position(|p| p["cellId"] == *a),
        team.iter().position(|p| p["cellId"] == *b),
    ) else {
        return;
    };
    for field in fields {
        let value_a = team[ia][*field].take();
        let value_b = std::mem::replace(&mut team[ib][*field], value_a);
        team[ia][*field] = value_b;
    }
}

fn event(session: Value) -> EventFixture {
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

    /// Estado com uma sessão: o jogador é a célula 2, com Kha'Zix na selva;
    /// a célula 4 tem Fizz no meio. As listas de troca vêm no formato da
    /// gravação 16.22.
    fn state() -> LcuState {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../agent/tests/fixtures/exemplo");
        let mut state = LcuState::new(&FixtureSet::load(&dir).unwrap());
        state.apply(&EventFixture {
            t_ms: 0,
            uri: SESSION.into(),
            event_type: "Create".into(),
            data: json!({
                "localPlayerCellId": 2,
                "myTeam": [
                    {"cellId": 2, "championId": 121, "selectedSkinId": 121000,
                     "assignedPosition": "jungle"},
                    {"cellId": 4, "championId": 105, "selectedSkinId": 105000,
                     "assignedPosition": "middle"},
                ],
                "actions": [[
                    {"id": 13, "type": "pick", "actorCellId": 2, "completed": false},
                    {"id": 14, "type": "pick", "actorCellId": 4, "completed": false},
                    {"id": 1, "type": "pick", "actorCellId": 4, "completed": true},
                ]],
                "trades": [
                    {"cellId": 4, "id": 47, "state": "AVAILABLE"},
                    {"cellId": 0, "id": 30, "state": "AVAILABLE"},
                ],
                "pickOrderSwaps": [{"cellId": 4, "id": 29, "state": "AVAILABLE"}],
                "positionSwaps": [{"cellId": 4, "id": 9, "state": "AVAILABLE"}],
            }),
        });
        state
    }

    fn apply(state: &mut LcuState, events: &[EventFixture]) {
        for e in events {
            state.apply(e);
        }
    }

    fn session(state: &LcuState) -> Value {
        state.get(SESSION).body
    }

    #[test]
    fn le_as_rotas() {
        assert_eq!(
            parse_route("/lol-champ-select/v1/session/champion-swaps/47/request"),
            Some((SwapKind::Champion, 47, SwapIntent::Request))
        );
        assert_eq!(
            parse_route("/lol-champ-select/v1/session/pick-order-swaps/28/cancel"),
            Some((SwapKind::PickOrder, 28, SwapIntent::Cancel))
        );
        assert_eq!(
            parse_route("/lol-champ-select/v1/session/position-swaps/9/decline"),
            Some((SwapKind::Position, 9, SwapIntent::Decline))
        );
        for path in [
            "/lol-champ-select/v1/session/trades/47/request",
            "/lol-champ-select/v1/session/champion-swaps/x/request",
            "/lol-champ-select/v1/session/champion-swaps/47/outra",
            "/lol-champ-select/v1/session/champion-swaps/47",
            "/lol-champ-select/v1/session/champion-swaps/47/accept/mais",
            "/lol-champ-select/v1/session/actions/13/complete",
        ] {
            assert_eq!(parse_route(path), None, "{path}");
        }
    }

    #[test]
    fn pede_e_cancela() {
        let mut state = state();
        let (response, events) = respond(&state, SwapKind::Champion, 47, SwapIntent::Request);
        assert_eq!(response.status, 200);
        apply(&mut state, &events);
        assert_eq!(
            session(&state)["trades"],
            json!([
                {"cellId": 4, "id": 47, "state": "SENT"},
                {"cellId": 0, "id": 30, "state": "INVALID"},
            ])
        );
        // Já pedida: não pede de novo, não aceita o próprio pedido.
        assert_eq!(
            respond(&state, SwapKind::Champion, 47, SwapIntent::Request)
                .0
                .status,
            400
        );
        assert_eq!(
            respond(&state, SwapKind::Champion, 47, SwapIntent::Accept)
                .0
                .status,
            400
        );

        let (response, events) = respond(&state, SwapKind::Champion, 47, SwapIntent::Cancel);
        assert_eq!(response.status, 200);
        apply(&mut state, &events);
        assert_eq!(session(&state)["trades"][0]["state"], "AVAILABLE");
        assert_eq!(session(&state)["trades"][1]["state"], "AVAILABLE");
        assert_eq!(session(&state)["myTeam"][0]["championId"], 121);
    }

    #[test]
    fn aceita_a_troca_de_campeao() {
        let mut state = state();
        let (response, events) = simulate(&state, SwapKind::Champion);
        assert_eq!(response.status, 200);
        assert_eq!(response.body["id"], 47);
        apply(&mut state, &events);
        assert_eq!(session(&state)["trades"][0]["state"], "RECEIVED");

        let (response, events) = respond(&state, SwapKind::Champion, 47, SwapIntent::Accept);
        assert_eq!(response.status, 200);
        apply(&mut state, &events);
        let s = session(&state);
        assert_eq!(s["myTeam"][0]["championId"], 105);
        assert_eq!(s["myTeam"][0]["selectedSkinId"], 105000);
        assert_eq!(s["myTeam"][1]["championId"], 121);
        assert_eq!(s["trades"][0]["state"], "AVAILABLE");
    }

    #[test]
    fn aceita_a_troca_de_cargo_e_de_vez() {
        let mut state = state();
        let (_, events) = simulate(&state, SwapKind::Position);
        apply(&mut state, &events);
        let (_, events) = respond(&state, SwapKind::Position, 9, SwapIntent::Accept);
        apply(&mut state, &events);
        let s = session(&state);
        assert_eq!(s["myTeam"][0]["assignedPosition"], "middle");
        assert_eq!(s["myTeam"][1]["assignedPosition"], "jungle");

        let (_, events) = simulate(&state, SwapKind::PickOrder);
        apply(&mut state, &events);
        let (_, events) = respond(&state, SwapKind::PickOrder, 29, SwapIntent::Accept);
        apply(&mut state, &events);
        let actions = &session(&state)["actions"][0];
        assert_eq!(actions[0]["actorCellId"], 4);
        assert_eq!(actions[1]["actorCellId"], 2);
        // Pick já feito não muda de dono.
        assert_eq!(actions[2]["actorCellId"], 4);
    }

    #[test]
    fn recusa() {
        let mut state = state();
        let (_, events) = simulate(&state, SwapKind::Champion);
        apply(&mut state, &events);
        let (response, events) = respond(&state, SwapKind::Champion, 47, SwapIntent::Decline);
        assert_eq!(response.status, 200);
        apply(&mut state, &events);
        let s = session(&state);
        assert_eq!(s["trades"][0]["state"], "AVAILABLE");
        assert_eq!(s["myTeam"][0]["championId"], 121);
    }

    #[test]
    fn erros() {
        let state = state();
        assert_eq!(
            respond(&state, SwapKind::Champion, 99, SwapIntent::Request)
                .0
                .status,
            404
        );
        assert_eq!(
            respond(&state, SwapKind::Champion, 47, SwapIntent::Decline)
                .0
                .status,
            400
        );
        assert_eq!(
            respond(&state, SwapKind::Champion, 47, SwapIntent::Cancel)
                .0
                .status,
            400
        );

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../agent/tests/fixtures/exemplo");
        let empty = LcuState::new(&FixtureSet::load(&dir).unwrap());
        assert_eq!(
            respond(&empty, SwapKind::Champion, 47, SwapIntent::Request)
                .0
                .status,
            404
        );
        assert_eq!(simulate(&empty, SwapKind::Champion).0.status, 404);
    }
}
