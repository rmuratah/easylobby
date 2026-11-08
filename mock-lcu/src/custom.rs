//! Lobby de personalizada na LCU falsa (#229): bots e troca de equipe.
//!
//! O formato dos corpos segue o que o agente manda ([`easylobby_agent::custom_lobby`]);
//! as equipes ficam em `gameConfig.customTeam100` e `customTeam200`, como na
//! LCU. Cada mudança manda o lobby novo pelo WAMP.

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::champions;
use crate::lobby::LOBBY;
use crate::state::{LcuState, Response};

/// Campeões que podem ser bots.
pub const AVAILABLE_BOTS: &str = "/lol-lobby/v2/lobby/custom/available-bots";

/// Bots da personalizada.
pub const BOTS: &str = "/lol-lobby/v1/lobby/custom/bots";

/// Troca de equipe (`/{team}` no fim).
pub const TEAM: &str = "/lol-lobby/v2/lobby/team";

const DIFFICULTIES: [&str; 3] = ["RSINTRO", "RSBEGINNER", "RSINTERMEDIATE"];
const POSITIONS: [&str; 5] = ["TOP", "JUNGLE", "MIDDLE", "BOTTOM", "UTILITY"];

fn team_key(team_id: &str) -> Option<&'static str> {
    match team_id {
        "100" => Some("customTeam100"),
        "200" => Some("customTeam200"),
        _ => None,
    }
}

fn error(status: u16, message: &str) -> (Response, Vec<EventFixture>) {
    (
        Response::error(status, format!("mock-lcu: {message}")),
        Vec::new(),
    )
}

fn updated(lobby: Value) -> (Response, Vec<EventFixture>) {
    (
        Response {
            status: 204,
            body: Value::Null,
        },
        vec![EventFixture {
            t_ms: 0,
            uri: LOBBY.into(),
            event_type: "Update".into(),
            data: lobby,
        }],
    )
}

/// O lobby atual, se for uma personalizada.
fn custom_lobby(state: &LcuState) -> Result<Value, (Response, Vec<EventFixture>)> {
    let current = state.get(LOBBY);
    if current.status != 200 {
        return Err(error(404, "LOBBY_NOT_FOUND"));
    }
    if current.body["gameConfig"]["isCustom"] != json!(true) {
        return Err(error(400, "o lobby não é uma personalizada"));
    }
    Ok(current.body)
}

/// Põe a personalizada no `gameConfig`: o jogador logado na Equipe 1.
pub fn make_custom(lobby: &mut Value, body: &Value) {
    let me = lobby["localMember"].clone();
    let custom = &body["customGameLobby"];
    let config_in = &custom["configuration"];
    let team_size = config_in["teamSize"].as_u64().unwrap_or(5);
    let mutator = match config_in.pointer("/mutators/id").and_then(Value::as_u64) {
        Some(18) => "DraftModeSinglePickStrategy",
        Some(21) => "AllRandomPickStrategy",
        Some(6) => "TournamentPickStrategy",
        _ => "SimulPickStrategy",
    };
    if let Some(config) = lobby.get_mut("gameConfig").and_then(Value::as_object_mut) {
        config.insert("isCustom".into(), json!(true));
        config.insert("maxTeamSize".into(), json!(team_size));
        config.insert("customTeam100".into(), json!([me]));
        config.insert("customTeam200".into(), json!([]));
        config.insert("customSpectators".into(), json!([]));
        // Uma personalizada real tem 4 vagas de espectador (16.22-aram).
        config.insert("maxLobbySpectatorCount".into(), json!(4));
        config.insert("customMutatorName".into(), json!(mutator));
        for (key, from) in [
            ("customLobbyName", &custom["lobbyName"]),
            ("customSpectatorPolicy", &config_in["spectatorPolicy"]),
            ("mapId", &config_in["mapId"]),
        ] {
            if !from.is_null() {
                config.insert(key.into(), from.clone());
            }
        }
    }
}

/// Tira a personalizada do `gameConfig` (uma fila comum).
pub fn make_queue(lobby: &mut Value) {
    if let Some(config) = lobby.get_mut("gameConfig").and_then(Value::as_object_mut) {
        config.insert("isCustom".into(), json!(false));
        config.insert("customTeam100".into(), json!([]));
        config.insert("customTeam200".into(), json!([]));
    }
}

/// `GET` [`AVAILABLE_BOTS`]: todos os campeões do jogo.
pub fn available_bots(state: &LcuState) -> Response {
    let summary = champions::game_data(state).body;
    let bots: Vec<Value> = summary
        .as_array()
        .map(|champions| {
            champions
                .iter()
                .filter(|c| c["id"].as_i64().is_some_and(|id| id > 0))
                .map(|c| json!({"active": true, "id": c["id"], "name": c["name"]}))
                .collect()
        })
        .unwrap_or_default();
    Response {
        status: 200,
        body: Value::Array(bots),
    }
}

struct BotChoice {
    champion_id: u64,
    difficulty: String,
    position: String,
    team_id: String,
}

fn bot_choice(body: &Value) -> Option<BotChoice> {
    let champion_id = body["championId"].as_u64().filter(|&id| id > 0)?;
    let difficulty = body["botDifficulty"]
        .as_str()
        .filter(|d| DIFFICULTIES.contains(d))?;
    let position = body["position"]
        .as_str()
        .filter(|p| POSITIONS.contains(p))?;
    let team_id = body["teamId"].as_str()?;
    team_key(team_id)?;
    Some(BotChoice {
        champion_id,
        difficulty: difficulty.into(),
        position: position.into(),
        team_id: team_id.into(),
    })
}

fn full(lobby: &Value, key: &str) -> bool {
    let size = lobby["gameConfig"]["maxTeamSize"].as_u64().unwrap_or(5);
    lobby["gameConfig"][key]
        .as_array()
        .is_some_and(|t| t.len() as u64 >= size)
}

/// `POST` [`BOTS`]: adiciona um bot.
pub fn add_bot(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let mut lobby = match custom_lobby(state) {
        Ok(lobby) => lobby,
        Err(error) => return error,
    };
    let Some(choice) = bot_choice(body) else {
        return error(400, "bot inválido");
    };
    let key = team_key(&choice.team_id).unwrap();
    if full(&lobby, key) {
        return error(400, "equipe cheia");
    }
    let count = ["customTeam100", "customTeam200"]
        .iter()
        .filter_map(|k| lobby["gameConfig"][k].as_array().map(Vec::len))
        .sum::<usize>();
    let bot = json!({
        "isBot": true, "isLeader": false, "puuid": "", "summonerId": 0,
        "botChampionId": choice.champion_id, "botDifficulty": choice.difficulty,
        "botPosition": choice.position, "botUuid": format!("mock-bot-{count}-{}", choice.champion_id),
        "summonerInternalName": format!("Bot_{}_{count}", choice.champion_id),
    });
    if let Some(team) = lobby["gameConfig"][key].as_array_mut() {
        team.push(bot);
    }
    updated(lobby)
}

/// Acha o bot `uuid` numa das equipes: (chave da equipe, posição).
fn find(lobby: &Value, name: &str, uuid: &str) -> Option<(&'static str, usize)> {
    ["customTeam100", "customTeam200"]
        .into_iter()
        .find_map(|key| {
            lobby["gameConfig"][key]
                .as_array()?
                .iter()
                .position(|m| {
                    m["isBot"] == json!(true)
                        && m["botUuid"] == json!(uuid)
                        && m["summonerInternalName"] == json!(name)
                })
                .map(|index| (key, index))
        })
}

/// Separa `/{nome}/{uuid}[/{teamId}]` depois de [`BOTS`].
fn bot_path(path: &str) -> Option<Vec<String>> {
    let rest = path.strip_prefix(BOTS)?.strip_prefix('/')?;
    Some(rest.split('/').map(percent_decode).collect::<Vec<_>>())
}

fn percent_decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&segment[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Se `path` é de um bot ([`BOTS`] com mais segmentos).
pub fn is_bot_path(path: &str) -> bool {
    path.starts_with(&format!("{BOTS}/"))
}

/// `POST` [`BOTS`]`/{nome}/{uuid}`: muda o bot. Trocar `teamId` muda de
/// equipe.
pub fn update_bot(state: &LcuState, path: &str, body: &Value) -> (Response, Vec<EventFixture>) {
    let mut lobby = match custom_lobby(state) {
        Ok(lobby) => lobby,
        Err(error) => return error,
    };
    let (Some(parts), Some(choice)) = (bot_path(path), bot_choice(body)) else {
        return error(400, "bot inválido");
    };
    let [name, uuid] = parts.as_slice() else {
        return error(404, "rota de bot inválida");
    };
    let Some((key, index)) = find(&lobby, name, uuid) else {
        return error(404, "bot não encontrado");
    };
    let config = &mut lobby["gameConfig"];
    let bot = &mut config[key][index];
    bot["botChampionId"] = json!(choice.champion_id);
    bot["botDifficulty"] = json!(choice.difficulty);
    bot["botPosition"] = json!(choice.position);
    updated(lobby)
}

/// `DELETE` [`BOTS`]`/{nome}/{uuid}/{teamId}`: tira o bot.
pub fn remove_bot(state: &LcuState, path: &str) -> (Response, Vec<EventFixture>) {
    let mut lobby = match custom_lobby(state) {
        Ok(lobby) => lobby,
        Err(error) => return error,
    };
    let Some(parts) = bot_path(path) else {
        return error(404, "rota de bot inválida");
    };
    let [name, uuid, team_id] = parts.as_slice() else {
        return error(404, "rota de bot inválida");
    };
    match find(&lobby, name, uuid) {
        Some((key, index)) if Some(key) == team_key(team_id) => {
            if let Some(team) = lobby["gameConfig"][key].as_array_mut() {
                team.remove(index);
            }
            updated(lobby)
        }
        _ => error(404, "bot não encontrado"),
    }
}

/// `POST` [`TEAM`]`/{team}`: leva o jogador logado para a equipe, ou para
/// os espectadores com `/spectator` (#263).
pub fn switch_team(state: &LcuState, path: &str) -> (Response, Vec<EventFixture>) {
    let mut lobby = match custom_lobby(state) {
        Ok(lobby) => lobby,
        Err(error) => return error,
    };
    let target = path
        .strip_prefix(TEAM)
        .and_then(|rest| rest.strip_prefix('/'));
    let Some(key) = target.and_then(|t| {
        if t == "spectator" {
            Some(SPECTATORS)
        } else {
            team_key(t)
        }
    }) else {
        return error(400, "equipe inválida");
    };
    let puuid = lobby["localMember"]["puuid"].clone();
    let Some((from, index)) = ["customTeam100", "customTeam200", SPECTATORS]
        .into_iter()
        .filter(|&k| k != key)
        .find_map(|k| {
            lobby["gameConfig"][k]
                .as_array()
                .and_then(|t| t.iter().position(|m| m["puuid"] == puuid))
                .map(|i| (k, i))
        })
    else {
        return error(400, "já está nessa equipe");
    };
    if key == SPECTATORS {
        let config = &lobby["gameConfig"];
        if config["customSpectatorPolicy"] == "NotAllowed" {
            return error(400, "a personalizada não aceita espectadores");
        }
        let max = config["maxLobbySpectatorCount"].as_u64().unwrap_or(4) as usize;
        if config[SPECTATORS].as_array().map_or(0, Vec::len) >= max {
            return error(400, "vagas de espectador cheias");
        }
    } else if full(&lobby, key) {
        return error(400, "equipe cheia");
    }
    let me = lobby["gameConfig"][from]
        .as_array_mut()
        .map(|t| t.remove(index))
        .unwrap_or_default();
    match lobby["gameConfig"][key].as_array_mut() {
        Some(team) => team.push(me),
        None => lobby["gameConfig"][key] = json!([me]),
    }
    updated(lobby)
}

/// Espectadores da personalizada no `gameConfig`.
const SPECTATORS: &str = "customSpectators";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby;
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

    fn practice(state: &mut LcuState) {
        let body = json!({
            "customGameLobby": {"configuration": {"gameMode": "PRACTICETOOL", "mapId": 11}},
            "isCustom": true
        });
        let (_, events) = lobby::create(state, &body);
        apply(state, &events);
    }

    #[test]
    fn bots_e_troca_de_equipe() {
        let mut s = state();
        let bot = json!({"botDifficulty": "RSINTRO", "championId": 22, "teamId": "200", "position": "BOTTOM"});
        assert_eq!(add_bot(&s, &bot).0.status, 404);
        practice(&mut s);
        let config = s.get(LOBBY).body["gameConfig"].clone();
        assert_eq!(config["isCustom"], true);
        assert_eq!(config["customTeam100"].as_array().unwrap().len(), 1);

        let (response, events) = add_bot(&s, &bot);
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        let added = s.get(LOBBY).body["gameConfig"]["customTeam200"][0].clone();
        assert_eq!(added["botChampionId"], 22);
        let name = added["summonerInternalName"].as_str().unwrap().to_owned();
        let uuid = added["botUuid"].as_str().unwrap().to_owned();

        let path = format!("{BOTS}/{name}/{uuid}");
        let change = json!({"botDifficulty": "RSINTERMEDIATE", "championId": 1, "teamId": "200", "position": "MIDDLE"});
        let (response, events) = update_bot(&s, &path, &change);
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        let changed = &s.get(LOBBY).body["gameConfig"]["customTeam200"][0];
        assert_eq!(changed["botChampionId"], 1);
        assert_eq!(changed["botPosition"], "MIDDLE");

        let (response, events) = switch_team(&s, &format!("{TEAM}/200"));
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        let config = s.get(LOBBY).body["gameConfig"].clone();
        assert!(config["customTeam100"].as_array().unwrap().is_empty());
        assert_eq!(config["customTeam200"].as_array().unwrap().len(), 2);
        assert_eq!(switch_team(&s, &format!("{TEAM}/200")).0.status, 400);

        assert_eq!(remove_bot(&s, &format!("{path}/100")).0.status, 404);
        let (response, events) = remove_bot(&s, &format!("{path}/200"));
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        assert_eq!(
            s.get(LOBBY).body["gameConfig"]["customTeam200"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        // Bot inválido.
        let fill = json!({"botDifficulty": "RSINTRO", "championId": 22, "teamId": "100", "position": "FILL"});
        assert_eq!(add_bot(&s, &fill).0.status, 400);
        assert!(!available_bots(&s).body.as_array().unwrap().is_empty());
    }

    #[test]
    fn vira_espectador_e_volta() {
        let mut s = state();
        practice(&mut s);
        // O Modo de Treino não aceita espectadores.
        assert_eq!(switch_team(&s, &format!("{TEAM}/spectator")).0.status, 400);

        let body = json!({
            "customGameLobby": {
                "configuration": {"gameMode": "CLASSIC", "mapId": 11, "mutators": {"id": 18},
                                  "spectatorPolicy": "AllAllowed", "teamSize": 5},
                "lobbyName": "Partida", "lobbyPassword": ""
            },
            "isCustom": true
        });
        let (_, events) = lobby::create(&s, &body);
        apply(&mut s, &events);
        let (response, events) = switch_team(&s, &format!("{TEAM}/spectator"));
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        let config = s.get(LOBBY).body["gameConfig"].clone();
        assert!(config["customTeam100"].as_array().unwrap().is_empty());
        assert_eq!(config["customSpectators"].as_array().unwrap().len(), 1);
        assert_eq!(switch_team(&s, &format!("{TEAM}/spectator")).0.status, 400);

        let (response, events) = switch_team(&s, &format!("{TEAM}/100"));
        assert_eq!(response.status, 204);
        apply(&mut s, &events);
        let config = s.get(LOBBY).body["gameConfig"].clone();
        assert_eq!(config["customTeam100"].as_array().unwrap().len(), 1);
        assert!(config["customSpectators"].as_array().unwrap().is_empty());
    }
}
