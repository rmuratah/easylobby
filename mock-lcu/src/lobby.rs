//! Lobby da LCU falsa (#17): `POST` e `DELETE` em `/lol-lobby/v2/lobby`, e
//! os cargos do jogador (#18) em `PUT` [`POSITION_PREFERENCES`] e os slots
//! do Jogo Dinâmico (#218) em `PUT` [`PLAYER_SLOTS`].
//!
//! Arena (#224): o lobby de uma fila `CHERRY` ganha equipes de 3
//! (`maxTeamSize`) até caber o `maxLobbySize` (6 na Arena 3x6), e cada
//! membro uma vaga em `subteamIndex` e `intraSubteamPosition`. O jogador
//! troca de vaga em `PUT` [`SUBTEAM_DATA`]. A gravação não tem um lobby da
//! Arena: os índices começando em 1 e a recusa de uma vaga ocupada são
//! suposições do mock, ainda não conferidas no LoL real.
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

/// Equipe e vaga do jogador logado na Arena (#224).
pub const SUBTEAM_DATA: &str = "/lol-lobby/v2/lobby/subteamData";

/// `gameMode` da Arena.
const ARENA_GAME_MODE: &str = "CHERRY";

/// Vagas de cada equipe na Arena 3x6.
const ARENA_TEAM_SIZE: u64 = 3;

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
/// do Modo de Treino (#209). Com `mapId` e `mutators.id` (#262), prefere a
/// fila do mesmo mapa com esse `gameTypeConfig.id` (o tipo de seleção).
fn custom_queue_id(state: &LcuState, body: &Value) -> Option<u64> {
    let config = body.pointer("/customGameLobby/configuration")?;
    let mode = config.get("gameMode").and_then(Value::as_str)?;
    let map_id = config.get("mapId").and_then(Value::as_u64);
    let mutator = config.pointer("/mutators/id").and_then(Value::as_u64);
    let recorded = state.recorded_body(QUEUES)?;
    let customs: Vec<&Value> = recorded
        .as_array()?
        .iter()
        .filter(|q| {
            q.get("isCustom").and_then(Value::as_bool) == Some(true)
                && q.get("gameMode").and_then(Value::as_str) == Some(mode)
        })
        .collect();
    customs
        .iter()
        .find(|q| {
            map_id.is_some_and(|m| q.get("mapId").and_then(Value::as_u64) == Some(m))
                && mutator.is_some_and(|m| {
                    q.pointer("/gameTypeConfig/id").and_then(Value::as_u64) == Some(m)
                })
        })
        .or_else(|| customs.first())?
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
    let game_mode = queue
        .and_then(|q| q.get("gameMode"))
        .and_then(Value::as_str);

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
    if let (Some(mode), Some(config)) = (
        game_mode,
        lobby.get_mut("gameConfig").and_then(Value::as_object_mut),
    ) {
        config.insert("gameMode".into(), json!(mode));
    }
    if body.get("customGameLobby").is_some() {
        crate::custom::make_custom(&mut lobby, body);
    } else {
        crate::custom::make_queue(&mut lobby);
    }
    // Equipes da Arena (#224); as outras filas ficam sem.
    make_subteams(
        &mut lobby,
        game_mode == Some(ARENA_GAME_MODE),
        max_lobby_size,
    );
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

/// Monta as equipes da Arena no lobby (`arena`), ou tira as equipes. Na
/// Arena, cada membro sem vaga vai para a primeira vaga livre.
fn make_subteams(lobby: &mut Value, arena: bool, max_lobby_size: u64) {
    let teams = if arena {
        max_lobby_size.div_ceil(ARENA_TEAM_SIZE).max(1)
    } else {
        1
    };
    if let Some(config) = lobby.get_mut("gameConfig").and_then(Value::as_object_mut) {
        config.insert("numberOfTeamsInLobby".into(), json!(teams));
        if arena {
            config.insert("maxTeamSize".into(), json!(ARENA_TEAM_SIZE));
        } else if config.get("isCustom") != Some(&json!(true))
            && config.get("maxTeamSize").and_then(Value::as_u64) == Some(ARENA_TEAM_SIZE)
        {
            // Saindo da Arena; a personalizada (#262) escolhe o tamanho.
            config.insert("maxTeamSize".into(), json!(5));
        }
    }
    let Some(members) = lobby.get_mut("members").and_then(Value::as_array_mut) else {
        return;
    };
    if !arena {
        for member in members.iter_mut() {
            member["subteamIndex"] = Value::Null;
            member["intraSubteamPosition"] = Value::Null;
        }
    } else {
        let mut taken: Vec<(u64, u64)> = members.iter().filter_map(subteam_of).collect();
        for member in members.iter_mut().filter(|m| subteam_of(m).is_none()) {
            let free = (1..=teams)
                .flat_map(|team| (1..=ARENA_TEAM_SIZE).map(move |position| (team, position)))
                .find(|slot| !taken.contains(slot));
            if let Some((team, position)) = free {
                member["subteamIndex"] = json!(team);
                member["intraSubteamPosition"] = json!(position);
                taken.push((team, position));
            }
        }
    }
    let puuid = lobby["localMember"]["puuid"].clone();
    let me = lobby["members"]
        .as_array()
        .and_then(|members| members.iter().find(|m| m["puuid"] == puuid))
        .cloned();
    if let (Some(me), Some(local)) = (me, lobby["localMember"].as_object_mut()) {
        local.insert("subteamIndex".into(), me["subteamIndex"].clone());
        local.insert(
            "intraSubteamPosition".into(),
            me["intraSubteamPosition"].clone(),
        );
    }
}

/// A equipe e a vaga de um membro, se ele tem.
fn subteam_of(member: &Value) -> Option<(u64, u64)> {
    Some((
        member["subteamIndex"].as_u64()?,
        member["intraSubteamPosition"].as_u64()?,
    ))
}

/// `PUT` [`SUBTEAM_DATA`] com `{"subteamIndex": N, "intraSubteamPosition":
/// M}` (#224): põe o jogador logado na vaga, no `localMember` e em
/// `members`. Fora de lobby responde 404; fora da Arena, com outro corpo ou
/// com uma vaga que não existe, 400; com a vaga de outro jogador, 409.
pub fn set_subteam_data(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let current = state.get(LOBBY);
    if current.status != 200 {
        return (
            Response::error(404, "mock-lcu: LOBBY_NOT_FOUND"),
            Vec::new(),
        );
    }
    let mut lobby = current.body;
    let Some(slot) = subteam_of(body) else {
        return (
            Response::error(
                400,
                "mock-lcu: o corpo precisa de \"subteamIndex\" e \"intraSubteamPosition\"",
            ),
            Vec::new(),
        );
    };
    if lobby["gameConfig"]["gameMode"] != json!(ARENA_GAME_MODE) {
        return (
            Response::error(400, "mock-lcu: esta fila não tem equipes"),
            Vec::new(),
        );
    }
    let teams = lobby["gameConfig"]["numberOfTeamsInLobby"]
        .as_u64()
        .unwrap_or(1);
    let size = lobby["gameConfig"]["maxTeamSize"].as_u64().unwrap_or(0);
    if !(1..=teams).contains(&slot.0) || !(1..=size).contains(&slot.1) {
        return (
            Response::error(400, "mock-lcu: essa vaga não existe"),
            Vec::new(),
        );
    }
    let puuid = lobby["localMember"]["puuid"].clone();
    let taken = lobby["members"].as_array().is_some_and(|members| {
        members
            .iter()
            .any(|m| m["puuid"] != puuid && subteam_of(m) == Some(slot))
    });
    if taken {
        return (
            Response::error(409, "mock-lcu: a vaga está ocupada"),
            Vec::new(),
        );
    }
    let set = |member: &mut Value| {
        if let Some(member) = member.as_object_mut() {
            member.insert("subteamIndex".into(), json!(slot.0));
            member.insert("intraSubteamPosition".into(), json!(slot.1));
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
    fn equipes_e_vagas_da_arena() {
        fn slot(s: &LcuState, team: u64, position: u64) -> (Response, Vec<EventFixture>) {
            set_subteam_data(
                s,
                &json!({"subteamIndex": team, "intraSubteamPosition": position}),
            )
        }

        let mut s = state("16.22");
        assert_eq!(
            set_subteam_data(&s, &json!({"subteamIndex": 1, "intraSubteamPosition": 1}))
                .0
                .status,
            404
        );

        let (response, events) = create(&s, &json!({"queueId": 1750}));
        assert_eq!(response.status, 200);
        let config = &response.body["gameConfig"];
        assert_eq!(config["gameMode"], "CHERRY");
        assert_eq!(config["maxLobbySize"], 18);
        assert_eq!(config["numberOfTeamsInLobby"], 6);
        assert_eq!(config["maxTeamSize"], 3);
        assert_eq!(response.body["localMember"]["subteamIndex"], 1);
        assert_eq!(response.body["members"][0]["intraSubteamPosition"], 1);
        apply(&mut s, &events);

        assert_eq!(slot(&s, 7, 1).0.status, 400, "só 6 equipes");
        assert_eq!(slot(&s, 1, 4).0.status, 400, "só 3 vagas");
        assert_eq!(slot(&s, 0, 1).0.status, 400, "começa em 1");
        assert_eq!(set_subteam_data(&s, &json!({})).0.status, 400);
        let (response, events) = slot(&s, 3, 2);
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        let lobby = s.get(LOBBY).body;
        assert_eq!(lobby["localMember"]["subteamIndex"], 3);
        assert_eq!(lobby["members"][0]["intraSubteamPosition"], 2);

        // Vaga de outro jogador.
        let mut cheio = lobby.clone();
        cheio["members"]
            .as_array_mut()
            .unwrap()
            .push(json!({"puuid": "amigo", "subteamIndex": 5, "intraSubteamPosition": 1}));
        apply(&mut s, &[event(LOBBY, "Update", cheio)]);
        assert_eq!(slot(&s, 5, 1).0.status, 409);

        // Voltar para uma fila comum tira as equipes.
        let (response, _) = create(&s, &json!({"queueId": 400}));
        assert_eq!(response.body["gameConfig"]["numberOfTeamsInLobby"], 1);
        assert_eq!(response.body["gameConfig"]["maxTeamSize"], 5);
        assert!(response.body["members"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["subteamIndex"].is_null()));
        apply(&mut s, &[event(LOBBY, "Update", response.body)]);
        assert_eq!(slot(&s, 1, 1).0.status, 400, "fora da Arena");
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

    #[test]
    fn personalizada_usa_a_fila_do_mapa_e_do_tipo_de_selecao() {
        use easylobby_agent::custom_game::{
            CustomGameOptions, CustomMap, CustomPickType, SpectatorPolicy,
        };
        use easylobby_agent::lobby::Lobby;

        let s = state("16.22");
        let options = CustomGameOptions {
            name: "Partida de Fulano".into(),
            password: "segredo".into(),
            map: CustomMap::HowlingAbyss,
            pick_type: CustomPickType::TournamentDraft,
            team_size: 3,
            spectators: SpectatorPolicy::LobbyOnly,
            hide_from_public_list: false,
            spectator_delay: true,
        };
        let (response, _) = create(&s, &options.body());
        assert_eq!(response.status, 200);
        assert_eq!(response.body["gameConfig"]["queueId"], 3230);
        let custom = Lobby::from_json(&response.body)
            .and_then(|l| l.custom)
            .expect("lobby de personalizada");
        assert_eq!(custom.team_size, 3);
        assert_eq!(custom.settings.name, "Partida de Fulano");
        assert_eq!(custom.settings.map, Some(CustomMap::HowlingAbyss));
        assert_eq!(
            custom.settings.pick_type,
            Some(CustomPickType::TournamentDraft)
        );
        assert_eq!(custom.settings.spectators, Some(SpectatorPolicy::LobbyOnly));

        let blind = CustomGameOptions {
            map: CustomMap::SummonersRift,
            pick_type: CustomPickType::BlindPick,
            ..options
        };
        assert_eq!(
            create(&s, &blind.body()).0.body["gameConfig"]["queueId"],
            3100
        );
    }
}
