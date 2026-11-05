//! Convites de lobby da LCU falsa (#32).
//!
//! A gravação 16.22 não tem convite recebido nenhum, então o mock monta os
//! convites no formato da LCU:
//!
//! - `GET` [`RECEIVED`] devolve a lista de convites recebidos (`[]` sem
//!   nenhum, como a LCU).
//! - [`SIMULATE`] (rota só do mock) faz o primeiro amigo da gravação convidar
//!   o jogador para uma Ranqueada Solo/Duo, para a UI ter um convite para
//!   mostrar.
//! - `POST` [`RECEIVED`]`/{id}/accept` tira o convite da lista e põe o
//!   jogador no lobby de quem convidou, com ele de líder.
//! - `POST` [`RECEIVED`]`/{id}/decline` tira o convite da lista.
//! - `POST` [`LOBBY_INVITATIONS`] com `[{"toSummonerId": 1, "toPuuid": "..."}]`
//!   põe os convites em `invitations` do lobby, como `Pending`.
//!
//! Como em [`crate::lobby`], as funções devolvem a resposta e os eventos, e
//! quem chama emite os eventos.

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::lobby::{self, LOBBY};
use crate::state::{LcuState, Response, GAMEFLOW_PHASE};

/// Convites recebidos.
pub const RECEIVED: &str = "/lol-lobby/v2/received-invitations";

/// Convites que o lobby atual mandou.
pub const LOBBY_INVITATIONS: &str = "/lol-lobby/v2/lobby/invitations";

/// Rota só do mock: chega um convite de um amigo.
pub const SIMULATE: &str = "/mock-lcu/received-invitation";

/// Lista de amigos gravada, de onde sai quem convida.
const FRIENDS: &str = "/lol-chat/v1/friends";

/// Fila do convite simulado: Ranqueada Solo/Duo.
const SIMULATED_QUEUE: u64 = 420;

/// `GET` [`RECEIVED`]: a lista atual, ou `[]` se nada chegou.
pub fn received(state: &LcuState) -> Response {
    let current = state.get(RECEIVED);
    if current.status == 200 {
        return current;
    }
    Response {
        status: 200,
        body: json!([]),
    }
}

fn received_list(state: &LcuState) -> Vec<Value> {
    received(state).body.as_array().cloned().unwrap_or_default()
}

/// [`SIMULATE`]: o primeiro amigo da gravação (ou um amigo inventado, se não
/// houver lista gravada) convida o jogador. Responde com o convite novo.
pub fn simulate(state: &LcuState) -> (Response, Vec<EventFixture>) {
    let friend = state
        .recorded_body(FRIENDS)
        .and_then(Value::as_array)
        .and_then(|friends| friends.first())
        .cloned()
        .unwrap_or_else(|| {
            json!({"summonerId": 9999, "puuid": "mock-amigo", "gameName": "Amigo", "gameTag": "BR1"})
        });
    let mut list = received_list(state);
    let invitation = json!({
        "canAcceptInvitation": true,
        "fromSummonerId": friend["summonerId"].as_u64().unwrap_or(0),
        "fromSummonerName": friend["gameName"].as_str().unwrap_or(""),
        "gameConfig": {
            "gameMode": "CLASSIC",
            "inviteGameType": "RANKED_GAME_SOLO",
            "mapId": 11,
            "queueId": SIMULATED_QUEUE,
        },
        "invitationId": format!("mock-convite-{}", next_id(&list)),
        "invitationType": "lobby",
        "restrictions": [],
        "state": "Pending",
        "timestamp": "0",
        // Só para o mock montar o lobby de quem convidou ao aceitar.
        "mockFromPuuid": friend["puuid"].as_str().unwrap_or(""),
    });
    list.push(invitation.clone());
    (
        Response {
            status: 200,
            body: invitation,
        },
        vec![event(RECEIVED, "Update", Value::Array(list))],
    )
}

/// Um número que nenhum convite da lista usa.
fn next_id(list: &[Value]) -> usize {
    (1..)
        .find(|n| {
            let id = format!("mock-convite-{n}");
            !list.iter().any(|i| i["invitationId"] == id.as_str())
        })
        .unwrap_or(1)
}

/// `POST` [`RECEIVED`]`/{id}/accept` ou `/decline`, com `rest` sendo o que
/// vem depois de [`RECEIVED`]. `None` se o caminho não for um desses.
pub fn answer(state: &LcuState, rest: &str) -> Option<(Response, Vec<EventFixture>)> {
    let (id, action) = rest.strip_prefix('/')?.split_once('/')?;
    let accept = match action {
        "accept" => true,
        "decline" => false,
        _ => return None,
    };
    let mut list = received_list(state);
    let Some(index) = list
        .iter()
        .position(|i| i["invitationId"] == id && i["state"] == "Pending")
    else {
        return Some((
            Response::error(404, format!("mock-lcu: convite {id} não encontrado")),
            Vec::new(),
        ));
    };
    let invitation = list.remove(index);
    let mut events = vec![event(RECEIVED, "Update", Value::Array(list))];
    if accept {
        if state.phase() != "Lobby" {
            events.push(event(GAMEFLOW_PHASE, "Update", json!("Lobby")));
        }
        let created = state.get(LOBBY).status != 200;
        events.push(event(
            LOBBY,
            if created { "Create" } else { "Update" },
            joined_lobby(state, &invitation),
        ));
    }
    Some((
        Response {
            status: 204,
            body: Value::Null,
        },
        events,
    ))
}

/// O lobby de quem convidou, com quem convidou de líder e o jogador logado
/// de membro comum.
fn joined_lobby(state: &LcuState, invitation: &Value) -> Value {
    let mut lobby = lobby::template(state);
    let queue_id = invitation["gameConfig"]["queueId"]
        .as_u64()
        .unwrap_or(SIMULATED_QUEUE);
    if let Some(config) = lobby.get_mut("gameConfig").and_then(Value::as_object_mut) {
        config.insert("queueId".into(), json!(queue_id));
        config.insert(
            "maxLobbySize".into(),
            json!(if queue_id == 420 { 2 } else { 5 }),
        );
    }
    let mut me = lobby["localMember"].clone();
    for (field, value) in [
        ("isLeader", json!(false)),
        ("allowedInviteOthers", json!(false)),
        ("allowedStartActivity", json!(false)),
        ("allowedChangeActivity", json!(false)),
    ] {
        if let Some(me) = me.as_object_mut() {
            me.insert(field.into(), value);
        }
    }
    let mut leader = me.clone();
    for (field, value) in [
        ("isLeader", json!(true)),
        ("puuid", invitation["mockFromPuuid"].clone()),
        ("summonerId", invitation["fromSummonerId"].clone()),
        ("allowedInviteOthers", json!(true)),
        ("allowedStartActivity", json!(true)),
        ("allowedChangeActivity", json!(true)),
    ] {
        if let Some(leader) = leader.as_object_mut() {
            leader.insert(field.into(), value);
        }
    }
    lobby["localMember"] = me.clone();
    lobby["members"] = json!([leader, me]);
    lobby["invitations"] = json!([]);
    lobby
}

/// `POST` [`LOBBY_INVITATIONS`]: convida para o lobby atual. Fora de lobby,
/// 404; sem permissão de convidar, 403; corpo que não é uma lista de
/// convidados, 400. Quem já está no lobby ou já tem convite pendente é
/// ignorado.
pub fn invite(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let current = state.get(LOBBY);
    if current.status != 200 {
        return (
            Response::error(404, "mock-lcu: LOBBY_NOT_FOUND"),
            Vec::new(),
        );
    }
    let invitees: Option<Vec<(u64, String)>> = body.as_array().and_then(|list| {
        list.iter()
            .map(|i| {
                let id = i.get("toSummonerId").and_then(Value::as_u64).unwrap_or(0);
                let puuid = i.get("toPuuid").and_then(Value::as_str).unwrap_or("");
                (id != 0 || !puuid.is_empty()).then(|| (id, puuid.to_owned()))
            })
            .collect()
    });
    let Some(invitees) = invitees.filter(|list| !list.is_empty()) else {
        return (
            Response::error(
                400,
                "mock-lcu: o corpo precisa ser uma lista de {\"toSummonerId\", \"toPuuid\"}",
            ),
            Vec::new(),
        );
    };
    let mut lobby = current.body;
    let me = &lobby["localMember"];
    if me["isLeader"] != json!(true) && me["allowedInviteOthers"] != json!(true) {
        return (
            Response::error(403, "mock-lcu: o jogador não pode convidar"),
            Vec::new(),
        );
    }
    let members: Vec<Value> = lobby["members"].as_array().cloned().unwrap_or_default();
    let mut invitations: Vec<Value> = lobby["invitations"].as_array().cloned().unwrap_or_default();
    let mut sent = Vec::new();
    for (summoner_id, puuid) in invitees {
        let same = |v: &Value, id_field: &str, puuid_field: &str| {
            (!puuid.is_empty() && v[puuid_field] == puuid.as_str())
                || (summoner_id != 0 && v[id_field] == summoner_id)
        };
        if members.iter().any(|m| same(m, "summonerId", "puuid"))
            || invitations
                .iter()
                .any(|i| i["state"] == "Pending" && same(i, "toSummonerId", "toPuuid"))
        {
            continue;
        }
        let invitation = json!({
            "invitationId": "",
            "invitationType": "lobby",
            "state": "Pending",
            "timestamp": "0",
            "toPuuid": puuid,
            "toSummonerId": summoner_id,
            "toSummonerName": "",
        });
        invitations.push(invitation.clone());
        sent.push(invitation);
    }
    lobby["invitations"] = Value::Array(invitations);
    (
        Response {
            status: 200,
            body: Value::Array(sent),
        },
        vec![event(LOBBY, "Update", lobby)],
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
    fn recebe_e_recusa() {
        let mut s = state();
        assert_eq!(received(&s).body, json!([]));
        let (response, events) = simulate(&s);
        assert_eq!(response.body["invitationId"], "mock-convite-1");
        assert_eq!(response.body["fromSummonerId"], 1003);
        apply(&mut s, &events);
        let (_, events) = simulate(&s);
        apply(&mut s, &events);
        assert_eq!(received(&s).body.as_array().unwrap().len(), 2);

        let (response, events) = answer(&s, "/mock-convite-1/decline").unwrap();
        assert_eq!(response.status, 204);
        assert_eq!(events.len(), 1, "recusar não mexe no lobby");
        apply(&mut s, &events);
        let list = received(&s).body;
        assert_eq!(list.as_array().unwrap().len(), 1);
        assert_eq!(list[0]["invitationId"], "mock-convite-2");

        // Convite que não existe, e caminhos que não são de convite.
        assert_eq!(answer(&s, "/mock-convite-1/accept").unwrap().0.status, 404);
        assert!(answer(&s, "/mock-convite-2/ignorar").is_none());
        assert!(answer(&s, "").is_none());
    }

    #[test]
    fn aceita_e_entra_no_lobby_de_quem_convidou() {
        let mut s = state();
        let (_, events) = simulate(&s);
        apply(&mut s, &events);
        let (response, events) = answer(&s, "/mock-convite-1/accept").unwrap();
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        assert_eq!(received(&s).body, json!([]));
        assert_eq!(s.phase(), "Lobby");
        let lobby = s.get(LOBBY).body;
        assert_eq!(lobby["gameConfig"]["queueId"], 420);
        assert_eq!(lobby["localMember"]["isLeader"], false);
        assert_eq!(lobby["members"][0]["isLeader"], true);
        assert_eq!(lobby["members"][0]["puuid"], "fake-puuid-0002");
        assert_eq!(lobby["members"][1]["puuid"], "fake-puuid-0001");
    }

    #[test]
    fn convida_para_o_lobby() {
        let mut s = state();
        let amigo = json!([{"toSummonerId": 1003, "toPuuid": "fake-puuid-0002"}]);
        // Fora de lobby.
        assert_eq!(invite(&s, &amigo).0.status, 404);

        let (_, events) = lobby::create(&s, &json!({"queueId": 400}));
        apply(&mut s, &events);
        let (response, events) = invite(&s, &amigo);
        assert_eq!(response.status, 200);
        apply(&mut s, &events);
        let invitations = s.get(LOBBY).body["invitations"].clone();
        let pending: Vec<_> = invitations
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["state"] == "Pending")
            .collect();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0]["toPuuid"], "fake-puuid-0002");

        // Convidar de novo não duplica.
        let (response, events) = invite(&s, &amigo);
        assert_eq!(response.body, json!([]));
        apply(&mut s, &events);
        assert_eq!(
            s.get(LOBBY).body["invitations"].as_array().unwrap().len(),
            invitations.as_array().unwrap().len()
        );

        // Corpo inválido.
        for body in [json!({}), json!([]), json!([{}])] {
            assert_eq!(invite(&s, &body).0.status, 400, "{body}");
        }
    }

    #[test]
    fn so_o_lider_ou_quem_ele_libera_convida() {
        let mut s = state();
        let (_, events) = simulate(&s);
        apply(&mut s, &events);
        let (_, events) = answer(&s, "/mock-convite-1/accept").unwrap();
        apply(&mut s, &events);
        let (response, events) = invite(&s, &json!([{"toSummonerId": 5}]));
        assert_eq!(response.status, 403);
        assert!(events.is_empty());
    }
}
