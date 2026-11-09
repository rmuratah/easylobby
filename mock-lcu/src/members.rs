//! Remover membro e passar a liderança na LCU falsa (#267).
//!
//! - `POST` [`MEMBERS`]`{summonerId}/kick` tira o membro de `members` (e das
//!   equipes e espectadores da personalizada).
//! - `POST` [`MEMBERS`]`{summonerId}/promote` passa a liderança: o membro
//!   vira o líder e o jogador logado perde `isLeader` e as permissões de
//!   líder.
//! - [`SIMULATE_JOIN`] (rota só do mock) põe um amigo da gravação no lobby,
//!   para ter com quem testar. A gravação não tem lobby com outro jogador.
//!
//! Os caminhos com `summonerId` vêm da lista `/help` da LCU; o corpo e as
//! respostas de erro são suposições do mock, ainda não conferidas no LoL
//! real. Como em [`crate::lobby`], as funções devolvem a resposta e os
//! eventos, e quem chama emite os eventos.

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::lobby::LOBBY;
use crate::state::{LcuState, Response};

/// Começo dos caminhos de cada membro do lobby.
pub const MEMBERS: &str = "/lol-lobby/v2/lobby/members/";

/// Rota só do mock: um amigo entra no lobby.
pub const SIMULATE_JOIN: &str = "/mock-lcu/lobby-member";

/// Lista de amigos gravada, de onde sai quem entra.
const FRIENDS: &str = "/lol-chat/v1/friends";

/// Permissões que só o líder tem no `localMember`.
const LEADER_FLAGS: [&str; 3] = [
    "allowedInviteOthers",
    "allowedStartActivity",
    "allowedChangeActivity",
];

/// Listas de jogadores da personalizada, em `gameConfig`.
const CUSTOM_LISTS: [&str; 3] = ["customTeam100", "customTeam200", "customSpectators"];

fn no_lobby() -> (Response, Vec<EventFixture>) {
    (
        Response::error(404, "mock-lcu: LOBBY_NOT_FOUND"),
        Vec::new(),
    )
}

/// `POST` [`MEMBERS`]`{summonerId}/kick` ou `/promote`, com `rest` sendo o
/// que vem depois de [`MEMBERS`]. `None` se o caminho não for um desses.
/// Fora de lobby responde 404; sem ser o líder, 403; com o próprio jogador,
/// 400; com quem não está no lobby, 404.
pub fn action(state: &LcuState, rest: &str) -> Option<(Response, Vec<EventFixture>)> {
    let (id, action) = rest.split_once('/')?;
    if action != "kick" && action != "promote" {
        return None;
    }
    let current = state.get(LOBBY);
    if current.status != 200 {
        return Some(no_lobby());
    }
    let Ok(summoner_id) = id.parse::<u64>() else {
        return Some((
            Response::error(400, "mock-lcu: summonerId inválido"),
            Vec::new(),
        ));
    };
    let mut lobby = current.body;
    if lobby["localMember"]["isLeader"] != json!(true) {
        return Some((
            Response::error(403, "mock-lcu: só o líder pode"),
            Vec::new(),
        ));
    }
    if lobby["localMember"]["summonerId"] == summoner_id {
        return Some((
            Response::error(400, "mock-lcu: não vale com o próprio jogador"),
            Vec::new(),
        ));
    }
    let members = lobby["members"].as_array().cloned().unwrap_or_default();
    if !members.iter().any(|m| m["summonerId"] == summoner_id) {
        return Some((
            Response::error(404, "mock-lcu: MEMBER_NOT_FOUND"),
            Vec::new(),
        ));
    }
    if action == "kick" {
        lobby["members"] = Value::Array(
            members
                .into_iter()
                .filter(|m| m["summonerId"] != summoner_id)
                .collect(),
        );
        for list in CUSTOM_LISTS {
            if let Some(players) = lobby["gameConfig"]
                .get_mut(list)
                .and_then(Value::as_array_mut)
            {
                players.retain(|m| m["summonerId"] != summoner_id);
            }
        }
        if let Some(invitations) = lobby.get_mut("invitations").and_then(Value::as_array_mut) {
            for invitation in invitations
                .iter_mut()
                .filter(|i| i["toSummonerId"] == summoner_id)
            {
                invitation["state"] = json!("Kicked");
            }
        }
    } else {
        let promote = |member: &mut Value| {
            member["isLeader"] = json!(member["summonerId"] == summoner_id);
        };
        if let Some(members) = lobby.get_mut("members").and_then(Value::as_array_mut) {
            members.iter_mut().for_each(promote);
        }
        for list in CUSTOM_LISTS {
            if let Some(players) = lobby["gameConfig"]
                .get_mut(list)
                .and_then(Value::as_array_mut)
            {
                players.iter_mut().for_each(promote);
            }
        }
        lobby["localMember"]["isLeader"] = json!(false);
        for flag in LEADER_FLAGS {
            lobby["localMember"][flag] = json!(false);
        }
    }
    Some((
        Response {
            status: 204,
            body: Value::Null,
        },
        vec![event(LOBBY, "Update", lobby)],
    ))
}

/// [`SIMULATE_JOIN`]: o primeiro amigo da gravação que não está no lobby
/// (ou um amigo inventado) entra como membro. Responde com o membro novo.
/// Fora de lobby responde 404; com o lobby cheio, 409.
pub fn simulate_join(state: &LcuState) -> (Response, Vec<EventFixture>) {
    let current = state.get(LOBBY);
    if current.status != 200 {
        return no_lobby();
    }
    let mut lobby = current.body;
    let members = lobby["members"].as_array().cloned().unwrap_or_default();
    let max = lobby["gameConfig"]["maxLobbySize"].as_u64().unwrap_or(5);
    if members.len() as u64 >= max {
        return (
            Response::error(409, "mock-lcu: o lobby está cheio"),
            Vec::new(),
        );
    }
    let taken = |id: u64| members.iter().any(|m| m["summonerId"] == id);
    let friend = state
        .recorded_body(FRIENDS)
        .and_then(Value::as_array)
        .and_then(|friends| {
            friends.iter().find(|f| {
                f["summonerId"]
                    .as_u64()
                    .is_some_and(|id| id != 0 && !taken(id))
            })
        })
        .cloned()
        .unwrap_or_else(|| {
            let id = (9000..).find(|id| !taken(*id)).unwrap_or(9000);
            json!({"summonerId": id, "puuid": format!("mock-amigo-{id}")})
        });
    let mut member = lobby["localMember"].clone();
    for (field, value) in [
        ("puuid", friend["puuid"].clone()),
        ("summonerId", friend["summonerId"].clone()),
        (
            "summonerIconId",
            friend["icon"].as_u64().map_or(json!(29), |i| json!(i)),
        ),
        ("isLeader", json!(false)),
        ("firstPositionPreference", json!("UNSELECTED")),
        ("secondPositionPreference", json!("UNSELECTED")),
        ("playerSlots", json!([])),
    ] {
        if let Some(member) = member.as_object_mut() {
            member.insert(field.into(), value);
        }
    }
    for flag in LEADER_FLAGS {
        member[flag] = json!(false);
    }
    let mut members = members;
    members.push(member.clone());
    lobby["members"] = Value::Array(members);
    (
        Response {
            status: 200,
            body: member,
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
    use crate::lobby::create;
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
    fn amigo_entra_e_o_lider_remove() {
        let mut s = state();
        assert_eq!(simulate_join(&s).0.status, 404);
        assert_eq!(action(&s, "1/kick").unwrap().0.status, 404);
        let (_, events) = create(&s, &json!({"queueId": 400}));
        apply(&mut s, &events);
        let (response, events) = simulate_join(&s);
        assert_eq!(response.status, 200);
        let friend = response.body["summonerId"].as_u64().unwrap();
        apply(&mut s, &events);
        assert_eq!(s.get(LOBBY).body["members"].as_array().unwrap().len(), 2);

        let me = s.get(LOBBY).body["localMember"]["summonerId"].clone();
        assert_eq!(action(&s, &format!("{me}/kick")).unwrap().0.status, 400);
        assert_eq!(action(&s, "123456789/kick").unwrap().0.status, 404);
        assert!(action(&s, &format!("{friend}/position-preferences")).is_none());

        let (response, events) = action(&s, &format!("{friend}/kick")).unwrap();
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        let members = s.get(LOBBY).body["members"].clone();
        assert_eq!(members.as_array().unwrap().len(), 1);
        assert_eq!(members[0]["summonerId"], me);
    }

    #[test]
    fn lider_passa_a_lideranca() {
        let mut s = state();
        let (_, events) = create(&s, &json!({"queueId": 400}));
        apply(&mut s, &events);
        let (response, events) = simulate_join(&s);
        let friend = response.body["summonerId"].as_u64().unwrap();
        apply(&mut s, &events);
        let (response, events) = action(&s, &format!("{friend}/promote")).unwrap();
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        let lobby = s.get(LOBBY).body;
        assert_eq!(lobby["localMember"]["isLeader"], false);
        assert_eq!(lobby["localMember"]["allowedInviteOthers"], false);
        let leaders: Vec<&Value> = lobby["members"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["isLeader"] == true)
            .collect();
        assert_eq!(leaders.len(), 1);
        assert_eq!(leaders[0]["summonerId"], friend);
        // Sem ser mais o líder, a LCU recusa.
        assert_eq!(action(&s, &format!("{friend}/kick")).unwrap().0.status, 403);
    }
}
