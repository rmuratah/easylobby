//! Criar partida personalizada com as opções do client oficial (#262).
//!
//! A personalizada nasce no mesmo `POST /lol-lobby/v2/lobby` do Modo de
//! Treino ([`crate::training`]), com `customGameLobby`:
//!
//! ```json
//! {
//!   "customGameLobby": {
//!     "configuration": {
//!       "gameMode": "CLASSIC", "gameMutator": "", "gameServerRegion": "",
//!       "mapId": 11, "mutators": { "id": 18 },
//!       "spectatorPolicy": "AllAllowed", "teamSize": 5
//!     },
//!     "lobbyName": "Partida de Fulano", "lobbyPassword": ""
//!   },
//!   "isCustom": true
//! }
//! ```
//!
//! O tipo de seleção vai em `mutators.id`: é o `gameTypeConfig.id` das filas
//! personalizadas da lista da LCU (gravação real do patch 16.22): 19 às
//! cegas, 18 alternada, 21 todos aleatórios e 6 alternada para torneios.
//! Nessa lista, os dois mapas aceitam os quatro tipos (filas 3100 a 3130 no
//! Summoner's Rift e 3200 a 3230 no Abismo Uivante), com até 5 por equipe.
//! Por isso a UI pode oferecer qualquer combinação dos tipos abaixo, e o
//! agente só confere o nome e o tamanho da equipe.
//!
//! Depois de criada, a personalizada mostra no lobby o nome, o mapa, o tipo de
//! seleção e quem pode assistir ([`CustomGameSettings`]), lidos do
//! `gameConfig` (`customLobbyName`, `mapId`, `gameMode`,
//! `customMutatorName`, `customSpectatorPolicy`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::lobby::{Lobby, LOBBY_PATH};
use crate::rest::{LcuClient, LcuError};

/// Maior equipe de uma personalizada.
pub const MAX_TEAM_SIZE: u32 = 5;

/// Maior nome de personalizada aceito pelo agente, em caracteres.
pub const MAX_NAME_LEN: usize = 64;

/// Mapa e modo da personalizada, como o client oficial oferece. O Modo de
/// Treino fica fora: ele continua só no card da aba Treinamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CustomMap {
    /// Summoner's Rift, modo Clássico (`CLASSIC`, mapa 11).
    SummonersRift,
    /// Abismo Uivante, modo ARAM (`ARAM`, mapa 12).
    HowlingAbyss,
}

impl CustomMap {
    pub fn map_id(self) -> u32 {
        match self {
            CustomMap::SummonersRift => 11,
            CustomMap::HowlingAbyss => 12,
        }
    }

    pub fn game_mode(self) -> &'static str {
        match self {
            CustomMap::SummonersRift => "CLASSIC",
            CustomMap::HowlingAbyss => "ARAM",
        }
    }

    /// O mapa de uma personalizada pelo `mapId` do `gameConfig`. `None` em
    /// outros mapas (Arena, TFT...).
    pub fn from_map_id(map_id: u64) -> Option<CustomMap> {
        match map_id {
            11 => Some(CustomMap::SummonersRift),
            12 => Some(CustomMap::HowlingAbyss),
            _ => None,
        }
    }
}

/// Tipo de seleção de campeões da personalizada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CustomPickType {
    /// Escolha às cegas.
    BlindPick,
    /// Escolha alternada, com bans.
    DraftMode,
    /// Todos aleatórios.
    AllRandom,
    /// Alternada para torneios.
    TournamentDraft,
}

impl CustomPickType {
    /// `mutators.id` do corpo de criar: o `gameTypeConfig.id` das filas
    /// personalizadas da LCU.
    pub fn mutator_id(self) -> u32 {
        match self {
            CustomPickType::BlindPick => 19,
            CustomPickType::DraftMode => 18,
            CustomPickType::AllRandom => 21,
            CustomPickType::TournamentDraft => 6,
        }
    }

    /// O tipo pelo `customMutatorName` do `gameConfig`. `None` com um nome
    /// desconhecido (as filas comuns usam `TeamBuilderDraftPickStrategy`).
    pub fn from_mutator_name(name: &str) -> Option<CustomPickType> {
        match name {
            "SimulPickStrategy" | "BlindPickStrategy" => Some(CustomPickType::BlindPick),
            "DraftModeSinglePickStrategy" | "DraftPickStrategy" => Some(CustomPickType::DraftMode),
            "AllRandomPickStrategy" => Some(CustomPickType::AllRandom),
            "TournamentPickStrategy" | "TournamentDraftPickStrategy" => {
                Some(CustomPickType::TournamentDraft)
            }
            _ => None,
        }
    }
}

/// Quem pode assistir à personalizada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SpectatorPolicy {
    /// Ninguém (`NotAllowed`).
    None,
    /// Só quem está no lobby (`LobbyAllowed`).
    LobbyOnly,
    /// Só amigos (`FriendsAllowed`).
    FriendsOnly,
    /// Todos (`AllAllowed`).
    All,
}

impl SpectatorPolicy {
    pub fn to_lcu(self) -> &'static str {
        match self {
            SpectatorPolicy::None => "NotAllowed",
            SpectatorPolicy::LobbyOnly => "LobbyAllowed",
            SpectatorPolicy::FriendsOnly => "FriendsAllowed",
            SpectatorPolicy::All => "AllAllowed",
        }
    }

    pub fn from_lcu(value: &str) -> Option<SpectatorPolicy> {
        match value {
            "NotAllowed" => Some(SpectatorPolicy::None),
            "LobbyAllowed" => Some(SpectatorPolicy::LobbyOnly),
            "FriendsAllowed" => Some(SpectatorPolicy::FriendsOnly),
            "AllAllowed" => Some(SpectatorPolicy::All),
            _ => None,
        }
    }
}

/// As opções do formulário de criar personalizada (comando
/// `createCustomGame`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomGameOptions {
    /// Nome da partida, como aparece na lista de personalizadas. Não pode
    /// ficar vazio nem passar de 64 caracteres. O client oficial sugere
    /// "Partida de {invocador}".
    pub name: String,
    /// Senha para entrar. Vazia deixa a partida sem senha.
    pub password: String,
    pub map: CustomMap,
    pub pick_type: CustomPickType,
    /// Jogadores por equipe, de 1 a 5.
    pub team_size: u32,
    pub spectators: SpectatorPolicy,
}

impl CustomGameOptions {
    /// Confere as opções antes de falar com a LCU. Devolve o motivo do erro.
    pub fn validate(&self) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("o nome da personalizada não pode ficar vazio".into());
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(format!(
                "o nome da personalizada passa de {MAX_NAME_LEN} caracteres"
            ));
        }
        if !(1..=MAX_TEAM_SIZE).contains(&self.team_size) {
            return Err(format!(
                "o tamanho da equipe vai de 1 a {MAX_TEAM_SIZE}, veio {}",
                self.team_size
            ));
        }
        Ok(())
    }

    /// Corpo do `POST /lol-lobby/v2/lobby` que cria a personalizada.
    pub fn body(&self) -> Value {
        lobby_body(&LobbyConfig {
            name: self.name.trim(),
            password: &self.password,
            game_mode: self.map.game_mode(),
            map_id: self.map.map_id(),
            mutator_id: self.pick_type.mutator_id(),
            team_size: self.team_size,
            spectators: self.spectators,
        })
    }
}

/// Tudo o que vai no `customGameLobby`. Usado também pelo Modo de Treino.
pub(crate) struct LobbyConfig<'a> {
    pub name: &'a str,
    pub password: &'a str,
    pub game_mode: &'a str,
    pub map_id: u32,
    pub mutator_id: u32,
    pub team_size: u32,
    pub spectators: SpectatorPolicy,
}

/// Corpo do `POST /lol-lobby/v2/lobby` de uma personalizada.
pub(crate) fn lobby_body(config: &LobbyConfig) -> Value {
    json!({
        "customGameLobby": {
            "configuration": {
                "gameMode": config.game_mode,
                "gameMutator": "",
                "gameServerRegion": "",
                "mapId": config.map_id,
                "mutators": { "id": config.mutator_id },
                "spectatorPolicy": config.spectators.to_lcu(),
                "teamSize": config.team_size
            },
            "lobbyName": config.name,
            "lobbyPassword": config.password
        },
        "isCustom": true
    })
}

/// Cria a personalizada. Devolve o lobby que a LCU respondeu.
pub async fn create(
    lcu: &LcuClient,
    options: &CustomGameOptions,
) -> Result<Option<Lobby>, LcuError> {
    let body: Value = lcu.post(LOBBY_PATH, &options.body()).await?;
    Ok(Lobby::from_json(&body))
}

/// As opções de uma personalizada já criada, para o cabeçalho do lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomGameSettings {
    /// Nome da partida (`customLobbyName`).
    pub name: String,
    /// `gameMode` da LCU: `CLASSIC`, `ARAM`, `PRACTICETOOL`...
    pub game_mode: String,
    /// `mapId` da LCU: 11 no Summoner's Rift, 12 no Abismo Uivante.
    pub map_id: u32,
    /// O mapa, ou `null` fora dos dois que o formulário oferece.
    pub map: Option<CustomMap>,
    /// O tipo de seleção, ou `null` quando a LCU manda um nome desconhecido.
    pub pick_type: Option<CustomPickType>,
    /// Quem pode assistir, ou `null` quando a LCU manda um valor
    /// desconhecido.
    pub spectators: Option<SpectatorPolicy>,
}

impl CustomGameSettings {
    /// Lê as opções do `gameConfig` de uma personalizada.
    pub fn from_game_config(config: &Value) -> CustomGameSettings {
        let text = |key: &str| config.get(key).and_then(Value::as_str).unwrap_or_default();
        let map_id = config.get("mapId").and_then(Value::as_u64).unwrap_or(0);
        CustomGameSettings {
            name: text("customLobbyName").to_owned(),
            game_mode: text("gameMode").to_owned(),
            map_id: u32::try_from(map_id).unwrap_or(0),
            map: CustomMap::from_map_id(map_id),
            pick_type: CustomPickType::from_mutator_name(text("customMutatorName")),
            spectators: SpectatorPolicy::from_lcu(text("customSpectatorPolicy")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> CustomGameOptions {
        CustomGameOptions {
            name: "  Partida de Fulano ".into(),
            password: "segredo".into(),
            map: CustomMap::HowlingAbyss,
            pick_type: CustomPickType::DraftMode,
            team_size: 3,
            spectators: SpectatorPolicy::FriendsOnly,
        }
    }

    #[test]
    fn corpo_leva_as_opcoes_do_formulario() {
        let body = options().body();
        let config = &body["customGameLobby"]["configuration"];
        assert_eq!(config["gameMode"], "ARAM");
        assert_eq!(config["mapId"], 12);
        assert_eq!(config["mutators"]["id"], 18);
        assert_eq!(config["teamSize"], 3);
        assert_eq!(config["spectatorPolicy"], "FriendsAllowed");
        assert_eq!(body["customGameLobby"]["lobbyName"], "Partida de Fulano");
        assert_eq!(body["customGameLobby"]["lobbyPassword"], "segredo");
        assert_eq!(body["isCustom"], true);
    }

    #[test]
    fn tipos_de_selecao_usam_o_game_type_config_das_filas() {
        // Filas personalizadas do Summoner's Rift na gravação do 16.22.
        let rest: Value =
            serde_json::from_str(include_str!("../tests/fixtures/16.22/rest.json")).unwrap();
        let queues = rest
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == "/lol-game-queues/v1/queues")
            .map(|e| e["body"].clone())
            .unwrap();
        let game_type = |id: u64| {
            queues
                .as_array()
                .unwrap()
                .iter()
                .find(|q| q["id"] == id)
                .map(|q| q["gameTypeConfig"]["id"].as_u64().unwrap())
                .unwrap()
        };
        for (queue, pick) in [
            (3100, CustomPickType::BlindPick),
            (3110, CustomPickType::DraftMode),
            (3120, CustomPickType::AllRandom),
            (3130, CustomPickType::TournamentDraft),
            (3200, CustomPickType::BlindPick),
            (3210, CustomPickType::DraftMode),
            (3220, CustomPickType::AllRandom),
            (3230, CustomPickType::TournamentDraft),
        ] {
            assert_eq!(
                game_type(queue),
                u64::from(pick.mutator_id()),
                "fila {queue}"
            );
        }
    }

    #[test]
    fn valida_nome_e_tamanho_da_equipe() {
        assert_eq!(options().validate(), Ok(()));
        for broken in [
            CustomGameOptions {
                name: "   ".into(),
                ..options()
            },
            CustomGameOptions {
                name: "x".repeat(MAX_NAME_LEN + 1),
                ..options()
            },
            CustomGameOptions {
                team_size: 0,
                ..options()
            },
            CustomGameOptions {
                team_size: 6,
                ..options()
            },
        ] {
            assert!(broken.validate().is_err(), "{broken:?}");
        }
        let one = CustomGameOptions {
            team_size: 1,
            password: String::new(),
            ..options()
        };
        assert_eq!(one.validate(), Ok(()));
    }

    #[test]
    fn le_as_opcoes_do_lobby_gravado() {
        // Personalizada de ARAM: Desordem gravada no LoL real (16.22-aram).
        let config = json!({
            "customLobbyName": "Partida de fake-name-0001",
            "customMutatorName": "AllRandomPickStrategy",
            "customSpectatorPolicy": "AllAllowed",
            "gameMode": "KIWI",
            "isCustom": true,
            "mapId": 12
        });
        assert_eq!(
            CustomGameSettings::from_game_config(&config),
            CustomGameSettings {
                name: "Partida de fake-name-0001".into(),
                game_mode: "KIWI".into(),
                map_id: 12,
                map: Some(CustomMap::HowlingAbyss),
                pick_type: Some(CustomPickType::AllRandom),
                spectators: Some(SpectatorPolicy::All),
            }
        );
        let unknown = CustomGameSettings::from_game_config(&json!({
            "customMutatorName": "TeamBuilderDraftPickStrategy",
            "customSpectatorPolicy": "?",
            "mapId": 30
        }));
        assert_eq!(unknown.map, None);
        assert_eq!(unknown.pick_type, None);
        assert_eq!(unknown.spectators, None);
    }
}
