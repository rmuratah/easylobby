//! Lobby de personalizada, com as duas equipes e os bots (#229).
//!
//! O Modo de Treino é uma personalizada ([`crate::training`]). No client
//! oficial ela abre um lobby com Equipe 1 e Equipe 2, de 5 vagas cada, onde o
//! líder põe bots e qualquer um troca de equipe. A LCU manda as equipes em
//! `gameConfig.customTeam100` e `gameConfig.customTeam200` (um membro por
//! item, bots incluídos, com `isBot`, `botChampionId`, `botDifficulty`,
//! `botPosition`, `botUuid` e `summonerInternalName`).
//!
//! Rotas, tiradas da lista `/help` da LCU (LoL real, 2026-11-08). O formato
//! dos corpos não está documentado e precisa ser conferido no LoL real:
//!
//! - `GET /lol-lobby/v2/lobby/custom/available-bots`: campeões que podem ser
//!   bots ([`available_bots`]).
//! - `POST /lol-lobby/v1/lobby/custom/bots`: adiciona um bot ([`add_bot`]).
//! - `POST /lol-lobby/v1/lobby/custom/bots/{summonerInternalName}/{botUuid}`:
//!   muda o bot ([`update_bot`]).
//! - `DELETE /lol-lobby/v1/lobby/custom/bots/{summonerInternalName}/{botUuid}/{teamId}`:
//!   tira o bot ([`remove_bot`]).
//!
//!   O LoL real (16.22) manda os bots sem `summonerInternalName`, e o client
//!   oficial põe o `botUuid` no lugar dos dois (`bots/{uuid}/{uuid}`), como
//!   mostra o trace do LeagueClient (#409).
//! - `POST /lol-lobby/v2/lobby/team/{TEAM1|TEAM2}`: troca de equipe, o
//!   "Juntar-se" ([`switch_team`]). Com `100`/`200` a LCU responde 400
//!   `INVALID_REQUEST` (#409).
//! - `POST /lol-lobby/v2/lobby/team/spectator`: vira espectador ([`spectate`],
//!   #263). A rota não aparece separada no `/help` e precisa ser conferida
//!   no LoL real; voltar a uma equipe é o mesmo "Juntar-se".
//! - `POST /lol-lobby/v1/lobby/custom/start-champ-select`: inicia a partida
//!   ([`crate::training::start_custom_champ_select`]).

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::chat::encode;
use crate::custom_game::CustomGameSettings;
use crate::lobby::Position;
use crate::rest::{LcuClient, LcuError};

/// Vira espectador da personalizada (#263).
pub const SPECTATE_PATH: &str = "/lol-lobby/v2/lobby/team/spectator";

/// Vagas de espectador quando a LCU não manda `maxLobbySpectatorCount`.
pub const DEFAULT_MAX_SPECTATORS: u32 = 4;

/// Campeões que podem ser bots na personalizada atual.
pub const AVAILABLE_BOTS_PATH: &str = "/lol-lobby/v2/lobby/custom/available-bots";

/// Bots da personalizada.
pub const BOTS_PATH: &str = "/lol-lobby/v1/lobby/custom/bots";

/// Troca de equipe (`/{team}` no fim).
pub const TEAM_PATH: &str = "/lol-lobby/v2/lobby/team";

/// Uma das duas equipes da personalizada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CustomTeam {
    /// Equipe 1 (azul), `teamId` 100.
    Team1,
    /// Equipe 2 (vermelha), `teamId` 200.
    Team2,
}

impl CustomTeam {
    /// `teamId` da LCU.
    pub const fn id(self) -> u32 {
        match self {
            CustomTeam::Team1 => 100,
            CustomTeam::Team2 => 200,
        }
    }

    /// Nome da equipe na rota de troca de equipe (#409).
    pub const fn lcu_name(self) -> &'static str {
        match self {
            CustomTeam::Team1 => "TEAM1",
            CustomTeam::Team2 => "TEAM2",
        }
    }
}

/// Dificuldade de um bot, como o client oficial mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BotDifficulty {
    /// Introdução (`RSINTRO`).
    Intro,
    /// Iniciante (`RSBEGINNER`).
    Beginner,
    /// Intermediário (`RSINTERMEDIATE`).
    Intermediate,
    /// Um nome que a LCU não mandava quando este código foi escrito.
    Unknown,
}

impl BotDifficulty {
    /// Nome da dificuldade que o agente manda para a LCU.
    pub const fn lcu_name(self) -> &'static str {
        match self {
            BotDifficulty::Intro => "RSINTRO",
            BotDifficulty::Beginner => "RSBEGINNER",
            BotDifficulty::Intermediate | BotDifficulty::Unknown => "RSINTERMEDIATE",
        }
    }

    /// A dificuldade de um nome da LCU. Aceita também os nomes antigos
    /// (`INTRO`, `EASY`, `MEDIUM`), porque o nome usado na personalizada não
    /// está documentado.
    pub fn from_lcu(name: &str) -> BotDifficulty {
        match name.to_ascii_uppercase().as_str() {
            "RSINTRO" | "INTRO" => BotDifficulty::Intro,
            "RSBEGINNER" | "EASY" => BotDifficulty::Beginner,
            "RSINTERMEDIATE" | "MEDIUM" => BotDifficulty::Intermediate,
            _ => BotDifficulty::Unknown,
        }
    }
}

/// O lobby de personalizada: as opções da partida e as duas equipes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomLobby {
    /// Nome, mapa, tipo de seleção e espectadores da partida (#262), para o
    /// cabeçalho do lobby.
    pub settings: CustomGameSettings,
    /// Vagas em cada equipe (5 no Summoner's Rift).
    pub team_size: u32,
    /// Equipe 1, na ordem da LCU.
    pub team1: Vec<CustomMember>,
    /// Equipe 2, na ordem da LCU.
    pub team2: Vec<CustomMember>,
    /// Quem está assistindo (`customSpectators`, #263), na ordem da LCU.
    pub spectators: Vec<CustomMember>,
    /// Quantos podem assistir (`maxLobbySpectatorCount`; 4 na gravação).
    pub max_spectators: u32,
}

/// Uma vaga ocupada: um jogador ou um bot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomMember {
    /// `puuid` do jogador. Vazio num bot.
    pub puuid: String,
    #[ts(type = "number")]
    pub summoner_id: u64,
    pub summoner_icon_id: u32,
    pub is_leader: bool,
    /// O bot, ou `null` num jogador.
    pub bot: Option<CustomBot>,
}

/// Um bot da personalizada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomBot {
    /// `botUuid` da LCU: é por ele que a UI pede para mudar ou tirar o bot.
    pub uuid: String,
    pub champion_id: u32,
    /// Posição do bot: uma das 5 rotas, ou `unselected` sem posição.
    pub position: Position,
    pub difficulty: BotDifficulty,
}

impl CustomLobby {
    /// Lê as equipes do `gameConfig` do lobby. `None` fora de personalizada
    /// (`isCustom` falso).
    pub fn from_game_config(config: &Value) -> Option<CustomLobby> {
        if config.get("isCustom").and_then(Value::as_bool) != Some(true) {
            return None;
        }
        let team = |key: &str| -> Vec<CustomMember> {
            config
                .get(key)
                .and_then(Value::as_array)
                .map(|members| members.iter().filter_map(member).collect())
                .unwrap_or_default()
        };
        let team_size = config
            .get("maxTeamSize")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .filter(|&n| n > 0)
            .unwrap_or(5);
        let max_spectators = config
            .get("maxLobbySpectatorCount")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(DEFAULT_MAX_SPECTATORS);
        Some(CustomLobby {
            settings: CustomGameSettings::from_game_config(config),
            team_size,
            team1: team("customTeam100"),
            team2: team("customTeam200"),
            spectators: team("customSpectators"),
            max_spectators,
        })
    }

    /// A equipe do jogador `puuid`, se ele estiver numa.
    pub fn team_of(&self, puuid: &str) -> Option<CustomTeam> {
        if self.team1.iter().any(|m| m.puuid == puuid) {
            Some(CustomTeam::Team1)
        } else if self.team2.iter().any(|m| m.puuid == puuid) {
            Some(CustomTeam::Team2)
        } else {
            None
        }
    }

    /// Se há outro jogador (bots não contam) além de `puuid`, nas equipes ou
    /// assistindo. Sem outro jogador a LCU não deixa ninguém assistir (#409).
    pub fn has_other_player(&self, puuid: &str) -> bool {
        self.team1
            .iter()
            .chain(&self.team2)
            .chain(&self.spectators)
            .any(|m| m.bot.is_none() && m.puuid != puuid)
    }

    /// Se o jogador `puuid` está assistindo.
    pub fn is_spectator(&self, puuid: &str) -> bool {
        self.spectators.iter().any(|m| m.puuid == puuid)
    }

    /// Os membros de uma equipe.
    pub fn team(&self, team: CustomTeam) -> &[CustomMember] {
        match team {
            CustomTeam::Team1 => &self.team1,
            CustomTeam::Team2 => &self.team2,
        }
    }

    /// O bot `uuid` e a equipe dele.
    pub fn bot(&self, uuid: &str) -> Option<(CustomTeam, &CustomBot)> {
        [CustomTeam::Team1, CustomTeam::Team2]
            .into_iter()
            .find_map(|team| {
                self.team(team)
                    .iter()
                    .filter_map(|m| m.bot.as_ref())
                    .find(|b| b.uuid == uuid)
                    .map(|bot| (team, bot))
            })
    }
}

/// Lê uma vaga de `customTeam100`/`customTeam200`/`customSpectators`.
///
/// Cada campo é lido sozinho e cai no padrão quando vem `null`, negativo ou
/// em outro tipo: um campo estranho num bot não pode sumir com a vaga
/// inteira (#313). Um item que nem é objeto fica de fora.
fn member(value: &Value) -> Option<CustomMember> {
    value.as_object()?;
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default();
    let number = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
            .unwrap_or_default()
    };
    let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);
    let is_bot = flag("isBot");
    let bot = is_bot.then(|| CustomBot {
        uuid: [text("botUuid"), text("botId")]
            .into_iter()
            .find(|id| !id.is_empty())
            .unwrap_or_default()
            .to_owned(),
        champion_id: u32::try_from(number("botChampionId")).unwrap_or_default(),
        position: Position::from_lcu(value.get("botPosition").and_then(Value::as_str)),
        difficulty: BotDifficulty::from_lcu(text("botDifficulty")),
    });
    Some(CustomMember {
        puuid: if is_bot {
            String::new()
        } else {
            text("puuid").to_owned()
        },
        summoner_id: number("summonerId"),
        summoner_icon_id: u32::try_from(number("summonerIconId")).unwrap_or_default(),
        is_leader: flag("isLeader"),
        bot,
    })
}

/// Corpo de adicionar ou mudar um bot.
pub fn bot_body(
    team: CustomTeam,
    champion_id: u32,
    difficulty: BotDifficulty,
    position: Position,
) -> Value {
    json!({
        "botDifficulty": difficulty.lcu_name(),
        "championId": champion_id,
        "teamId": team.id().to_string(),
        "position": position.lcu_name(),
    })
}

/// Corpo de mudar um bot: o de [`bot_body`] com o `botUuid`. Sem ele a LCU
/// responde 400 `INVALID_REQUEST`; o client oficial manda o corpo com 128
/// bytes, o tamanho exato deste com o `botUuid` (trace do LeagueClient,
/// LoL 16.22, #409).
pub fn update_bot_body(
    bot: &CustomBot,
    team: CustomTeam,
    champion_id: u32,
    difficulty: BotDifficulty,
    position: Position,
) -> Value {
    let mut body = bot_body(team, champion_id, difficulty, position);
    body["botUuid"] = json!(bot.uuid);
    body
}

/// Campeões que podem ser bots, a partir da resposta de
/// [`AVAILABLE_BOTS_PATH`], em ordem de id. Cada item traz o campeão em `id`
/// (ou `championId`); itens com `active: false` ficam de fora.
pub fn available_bots_from_json(value: &Value) -> Option<Vec<u32>> {
    let items = value.as_array()?;
    let mut ids: Vec<u32> = items
        .iter()
        .filter(|item| item.get("active").and_then(Value::as_bool) != Some(false))
        .filter_map(|item| {
            item.get("id")
                .or_else(|| item.get("championId"))
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())
        })
        .filter(|&id| id > 0)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    Some(ids)
}

/// Busca os campeões que podem ser bots.
pub async fn available_bots(lcu: &LcuClient) -> Result<Option<Vec<u32>>, LcuError> {
    let body: Value = lcu.get(AVAILABLE_BOTS_PATH).await?;
    Ok(available_bots_from_json(&body))
}

/// Adiciona um bot na equipe `team`.
pub async fn add_bot(
    lcu: &LcuClient,
    team: CustomTeam,
    champion_id: u32,
    difficulty: BotDifficulty,
    position: Position,
) -> Result<(), LcuError> {
    lcu.post::<_, Option<Value>>(
        BOTS_PATH,
        &bot_body(team, champion_id, difficulty, position),
    )
    .await
    .map(|_| ())
}

/// Muda o campeão, a dificuldade ou a posição do bot.
pub async fn update_bot(
    lcu: &LcuClient,
    bot: &CustomBot,
    team: CustomTeam,
    champion_id: u32,
    difficulty: BotDifficulty,
    position: Position,
) -> Result<(), LcuError> {
    let path = format!("{BOTS_PATH}/{}/{}", encode(&bot.uuid), encode(&bot.uuid));
    lcu.post::<_, Option<Value>>(
        &path,
        &update_bot_body(bot, team, champion_id, difficulty, position),
    )
    .await
    .map(|_| ())
}

/// Tira o bot da equipe `team`.
pub async fn remove_bot(
    lcu: &LcuClient,
    bot: &CustomBot,
    team: CustomTeam,
) -> Result<(), LcuError> {
    let path = format!(
        "{BOTS_PATH}/{}/{}/{}",
        encode(&bot.uuid),
        encode(&bot.uuid),
        team.id()
    );
    lcu.delete::<Option<Value>>(&path).await.map(|_| ())
}

/// Leva o jogador logado para a equipe `team` ("Juntar-se").
pub async fn switch_team(lcu: &LcuClient, team: CustomTeam) -> Result<(), LcuError> {
    let path = format!("{TEAM_PATH}/{}", team.lcu_name());
    lcu.send::<Option<Value>>(Method::POST, &path, None)
        .await
        .map(|_| ())
}

/// Leva o jogador logado para os espectadores da personalizada (#263).
pub async fn spectate(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.send::<Option<Value>>(Method::POST, SPECTATE_PATH, None)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Value {
        json!({
            "isCustom": true,
            "maxTeamSize": 5,
            "customTeam100": [
                {"puuid": "eu", "summonerId": 1, "summonerIconId": 29, "isLeader": true, "isBot": false},
                {"puuid": "", "isBot": true, "botChampionId": 22, "botDifficulty": "RSBEGINNER",
                 "botPosition": "BOTTOM", "botUuid": "uuid-ashe", "summonerInternalName": "Bot_Ashe"},
            ],
            "customTeam200": [
                {"isBot": true, "botChampionId": 1, "botDifficulty": "EASY",
                 "botPosition": "NONE", "botId": "id-annie", "summonerInternalName": "Bot Annie"},
            ],
        })
    }

    /// A LoL manda todo bot com `summonerIconId` -1 (#313, gravado no LoL
    /// real): o bot não pode sumir da equipe por isso.
    #[test]
    fn bot_com_icone_negativo_aparece() {
        let custom = CustomLobby::from_game_config(&json!({
            "isCustom": true,
            "customTeam100": [
                {"puuid": "eu", "summonerId": 1, "summonerIconId": 29, "isLeader": true, "isBot": false},
            ],
            "customTeam200": [
                {"puuid": "", "summonerId": 0, "summonerIconId": -1, "isLeader": false,
                 "isBot": true, "botChampionId": 63, "botDifficulty": "RSINTRO",
                 "botPosition": "TOP", "botUuid": "uuid-brand", "summonerInternalName": "Brand bot",
                 "subteamIndex": null, "teamId": 200},
                {"isBot": true, "botChampionId": null, "botDifficulty": null,
                 "botPosition": null, "botUuid": null, "summonerIconId": -1},
            ],
        }))
        .unwrap();
        assert_eq!(custom.team2.len(), 2);
        let brand = &custom.team2[0];
        assert_eq!(brand.summoner_icon_id, 0);
        let bot = brand.bot.as_ref().unwrap();
        assert_eq!(
            (
                bot.uuid.as_str(),
                bot.champion_id,
                bot.position,
                bot.difficulty
            ),
            ("uuid-brand", 63, Position::Top, BotDifficulty::Intro)
        );
        // Campos `null` caem no padrão em vez de tirar o bot.
        let empty = custom.team2[1].bot.as_ref().unwrap();
        assert_eq!(
            (empty.champion_id, empty.difficulty),
            (0, BotDifficulty::Unknown)
        );
    }

    #[test]
    fn le_as_duas_equipes() {
        let custom = CustomLobby::from_game_config(&config()).unwrap();
        assert_eq!(custom.team_size, 5);
        assert_eq!(custom.team1.len(), 2);
        assert_eq!(custom.team1[0].puuid, "eu");
        assert!(custom.team1[0].is_leader);
        assert_eq!(custom.team1[0].bot, None);
        let ashe = custom.team1[1].bot.as_ref().unwrap();
        assert_eq!(
            (ashe.champion_id, ashe.position, ashe.difficulty),
            (22, Position::Bottom, BotDifficulty::Beginner)
        );
        // Sem `botUuid`, o `botId`; nome antigo de dificuldade.
        let annie = custom.team2[0].bot.as_ref().unwrap();
        assert_eq!(annie.uuid, "id-annie");
        assert_eq!(annie.position, Position::Unselected);
        assert_eq!(annie.difficulty, BotDifficulty::Beginner);
        assert_eq!(custom.team_of("eu"), Some(CustomTeam::Team1));
        assert_eq!(custom.team_of("outro"), None);
        assert_eq!(custom.bot("id-annie").unwrap().0, CustomTeam::Team2);
        assert!(custom.bot("nada").is_none());
        assert!(custom.spectators.is_empty());
        assert_eq!(custom.max_spectators, DEFAULT_MAX_SPECTATORS);
        // Só bots além do jogador: ninguém para assistir junto (#409).
        assert!(!custom.has_other_player("eu"));
    }

    #[test]
    fn le_os_espectadores() {
        let mut config = config();
        config["customSpectators"] =
            json!([{"puuid": "amigo", "summonerId": 2, "summonerIconId": 7, "isBot": false}]);
        config["maxLobbySpectatorCount"] = json!(2);
        let custom = CustomLobby::from_game_config(&config).unwrap();
        assert_eq!(custom.spectators.len(), 1);
        assert_eq!(custom.spectators[0].puuid, "amigo");
        assert_eq!(custom.max_spectators, 2);
        assert!(custom.is_spectator("amigo"));
        // O amigo assistindo conta como outro jogador; os bots, não (#409).
        assert!(custom.has_other_player("eu"));
        assert!(custom.has_other_player("amigo"));
        assert!(!custom.is_spectator("eu"));
        assert_eq!(custom.team_of("amigo"), None);
    }

    #[test]
    fn fora_de_personalizada_nao_tem_equipes() {
        assert_eq!(
            CustomLobby::from_game_config(&json!({"queueId": 420})),
            None
        );
        assert_eq!(
            CustomLobby::from_game_config(&json!({"isCustom": false})),
            None
        );
        let empty = CustomLobby::from_game_config(&json!({"isCustom": true})).unwrap();
        assert_eq!(empty.team_size, 5);
        assert!(empty.team1.is_empty() && empty.team2.is_empty());
    }

    #[test]
    fn dificuldades() {
        for d in [
            BotDifficulty::Intro,
            BotDifficulty::Beginner,
            BotDifficulty::Intermediate,
        ] {
            assert_eq!(BotDifficulty::from_lcu(d.lcu_name()), d);
        }
        assert_eq!(
            BotDifficulty::from_lcu("MEDIUM"),
            BotDifficulty::Intermediate
        );
        assert_eq!(BotDifficulty::from_lcu("NONE"), BotDifficulty::Unknown);
        assert_eq!(
            serde_json::to_value(BotDifficulty::Intermediate).unwrap(),
            json!("intermediate")
        );
    }

    #[test]
    fn corpo_do_bot_e_campeoes_disponiveis() {
        assert_eq!(
            bot_body(
                CustomTeam::Team2,
                22,
                BotDifficulty::Intro,
                Position::Middle
            ),
            json!({"botDifficulty": "RSINTRO", "championId": 22, "teamId": "200", "position": "MIDDLE"})
        );
        let bot = CustomBot {
            uuid: "99314620-6d81-4e76-8750-5cd59a7f167f".into(),
            champion_id: 63,
            position: Position::Utility,
            difficulty: BotDifficulty::Intro,
        };
        let body = update_bot_body(
            &bot,
            CustomTeam::Team2,
            63,
            BotDifficulty::Intro,
            Position::Utility,
        );
        assert_eq!(body["botUuid"], "99314620-6d81-4e76-8750-5cd59a7f167f");
        // O mesmo tamanho do corpo do client oficial no trace (#409).
        assert_eq!(serde_json::to_string(&body).unwrap().len(), 128);
        let bots = json!([
            {"active": true, "id": 22, "name": "Ashe"},
            {"active": false, "id": 99, "name": "Fora"},
            {"championId": 1, "name": "Annie"},
            {"id": 22},
        ]);
        assert_eq!(available_bots_from_json(&bots), Some(vec![1, 22]));
        assert_eq!(available_bots_from_json(&json!({})), None);
    }

    #[test]
    fn formato_para_a_ui() {
        let custom = CustomLobby::from_game_config(&config()).unwrap();
        let value = serde_json::to_value(&custom.team1[1]).unwrap();
        assert_eq!(
            value,
            json!({
                "puuid": "",
                "summonerId": 0,
                "summonerIconId": 0,
                "isLeader": false,
                "bot": {"uuid": "uuid-ashe", "championId": 22, "position": "bottom", "difficulty": "beginner"},
            })
        );
    }
}
