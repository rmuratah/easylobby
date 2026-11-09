//! Lista de personalizadas abertas e entrar nelas (#264), a aba "Entrar" da
//! tela de personalizada do client oficial.
//!
//! - `POST /lol-lobby/v1/custom-games/refresh` pede para a LCU buscar a lista
//!   de novo no servidor;
//! - `GET /lol-lobby/v1/custom-games` devolve a lista
//!   (`LolLobbyLobbyCustomGame[]`): `id`, `lobbyName`, `ownerDisplayName`,
//!   `mapId`, `gameType`, `filledPlayerSlots`, `maxPlayerSlots`,
//!   `filledSpectatorSlots`, `maxSpectatorSlots`, `spectatorPolicy` e
//!   `hasPassword`. A lista nova também chega pelo WAMP, no mesmo caminho;
//! - `POST /lol-lobby/v1/custom-games/{id}/join` com
//!   `{"asSpectator": false, "password": "..."}` entra na partida. O lobby
//!   dela chega pelos eventos de sempre de `/lol-lobby/v2/lobby`.
//!
//! Os nomes dos campos vêm do esquema da LCU, mas a lista ainda não foi
//! gravada no LoL real. Por isso a leitura aceita campo faltando, e o
//! `gameType` vai também cru para a UI: os valores dele (`BLIND_PICK`?
//! `SimulPickStrategy`?) ainda precisam ser conferidos.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::custom_game::{CustomMap, CustomPickType, SpectatorPolicy};
use crate::rest::{LcuClient, LcuError};

/// A lista de personalizadas abertas. Também é o caminho do evento do WAMP.
pub const CUSTOM_GAMES_PATH: &str = "/lol-lobby/v1/custom-games";

/// Pede para a LCU buscar a lista de novo.
pub const CUSTOM_GAMES_REFRESH_PATH: &str = "/lol-lobby/v1/custom-games/refresh";

/// Maior senha aceita pelo agente, em caracteres. A LCU não diz o limite; o
/// client oficial corta bem antes disso.
pub const MAX_PASSWORD_LEN: usize = 64;

/// Uma personalizada aberta na lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomGameListing {
    /// Id da partida, para o `joinCustomGame`.
    #[ts(type = "number")]
    pub id: u64,
    /// Nome da partida (`lobbyName`).
    pub name: String,
    /// Nome de quem criou (`ownerDisplayName`).
    pub owner: String,
    /// `mapId` da LCU: 11 no Summoner's Rift, 12 no Abismo Uivante.
    pub map_id: u32,
    /// O mapa, ou `null` fora dos três que o formulário oferece.
    pub map: Option<CustomMap>,
    /// `gameType` cru da LCU, para a UI mostrar quando `pickType` é `null`.
    pub game_type: String,
    /// O tipo de seleção lido do `gameType`, ou `null` se o agente não
    /// conhece o valor.
    pub pick_type: Option<CustomPickType>,
    /// Jogadores nas equipes (`filledPlayerSlots`).
    pub players: u32,
    /// Vagas nas equipes (`maxPlayerSlots`).
    pub max_players: u32,
    /// Espectadores (`filledSpectatorSlots`).
    pub spectators: u32,
    /// Vagas de espectador (`maxSpectatorSlots`).
    pub max_spectators: u32,
    /// Quem pode assistir, ou `null` com um valor desconhecido.
    pub spectator_policy: Option<SpectatorPolicy>,
    /// Pede senha para entrar (`hasPassword`).
    pub has_password: bool,
}

impl CustomGameListing {
    /// Lê uma entrada da lista. `None` sem `id`.
    pub fn from_json(value: &Value) -> Option<CustomGameListing> {
        let id = value
            .get("id")
            .and_then(Value::as_u64)
            .filter(|&id| id > 0)?;
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let count = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0)
        };
        let map_id = value.get("mapId").and_then(Value::as_u64).unwrap_or(0);
        let game_type = text("gameType");
        Some(CustomGameListing {
            id,
            name: text("lobbyName"),
            owner: text("ownerDisplayName"),
            map_id: u32::try_from(map_id).unwrap_or(0),
            map: CustomMap::from_map_id(map_id),
            pick_type: pick_type_from_game_type(&game_type),
            game_type,
            players: count("filledPlayerSlots"),
            max_players: count("maxPlayerSlots"),
            spectators: count("filledSpectatorSlots"),
            max_spectators: count("maxSpectatorSlots"),
            spectator_policy: SpectatorPolicy::from_lcu(&text("spectatorPolicy")),
            has_password: value
                .get("hasPassword")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    /// Se as equipes estão cheias.
    pub fn is_full(&self) -> bool {
        self.max_players > 0 && self.players >= self.max_players
    }
}

/// A lista inteira. `None` se o corpo não é uma lista; entradas sem `id`
/// ficam de fora.
pub fn from_json(value: &Value) -> Option<Vec<CustomGameListing>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(CustomGameListing::from_json)
            .collect(),
    )
}

/// O tipo de seleção pelo `gameType` da lista. Aceita os nomes das
/// estratégias do `gameConfig` (`SimulPickStrategy`...) e nomes no estilo
/// `BLIND_PICK`, porque o valor ainda não foi conferido no LoL real.
pub fn pick_type_from_game_type(game_type: &str) -> Option<CustomPickType> {
    if let Some(pick) = CustomPickType::from_mutator_name(game_type) {
        return Some(pick);
    }
    let normalized: String = game_type
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_uppercase();
    if normalized.is_empty() {
        None
    } else if normalized.contains("TOURNAMENT") {
        Some(CustomPickType::TournamentDraft)
    } else if normalized.contains("DRAFT") {
        Some(CustomPickType::DraftMode)
    } else if normalized.contains("RANDOM") {
        Some(CustomPickType::AllRandom)
    } else if normalized.contains("BLIND") || normalized.contains("SIMUL") {
        Some(CustomPickType::BlindPick)
    } else {
        None
    }
}

/// Caminho de entrar na partida `id`.
pub fn join_path(id: u64) -> String {
    format!("{CUSTOM_GAMES_PATH}/{id}/join")
}

/// Corpo do `POST` de entrar.
pub fn join_body(password: &str, as_spectator: bool) -> Value {
    json!({ "asSpectator": as_spectator, "password": password })
}

/// Pede a lista nova ao servidor e lê a que a LCU tem. A LCU pode responder o
/// `refresh` antes de a lista nova chegar; nesse caso ela vem depois pelo
/// WAMP. Um `refresh` recusado não impede de ler a lista que já está lá.
pub async fn refresh(lcu: &LcuClient) -> Result<Vec<CustomGameListing>, LcuError> {
    let _ = lcu
        .post::<_, Value>(CUSTOM_GAMES_REFRESH_PATH, &json!({}))
        .await;
    let body: Value = lcu.get(CUSTOM_GAMES_PATH).await?;
    Ok(from_json(&body).unwrap_or_default())
}

/// Entra na partida `id`.
pub async fn join(
    lcu: &LcuClient,
    id: u64,
    password: &str,
    as_spectator: bool,
) -> Result<(), LcuError> {
    lcu.post::<_, Value>(&join_path(id), &join_body(password, as_spectator))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_a_lista_da_lcu() {
        let list = from_json(&json!([
            {
                "filledPlayerSlots": 3,
                "filledSpectatorSlots": 1,
                "gameType": "DRAFT_MODE",
                "hasPassword": true,
                "id": 6_123_456_789u64,
                "lobbyName": "5v5 da firma",
                "mapId": 11,
                "maxPlayerSlots": 10,
                "maxSpectatorSlots": 4,
                "ownerDisplayName": "Fulano#BR1",
                "passbackUrl": "",
                "spectatorPolicy": "AllAllowed"
            },
            {"lobbyName": "sem id"},
            {"id": 7, "mapId": 30, "gameType": "?"}
        ]))
        .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0],
            CustomGameListing {
                id: 6_123_456_789,
                name: "5v5 da firma".into(),
                owner: "Fulano#BR1".into(),
                map_id: 11,
                map: Some(CustomMap::SummonersRift),
                game_type: "DRAFT_MODE".into(),
                pick_type: Some(CustomPickType::DraftMode),
                players: 3,
                max_players: 10,
                spectators: 1,
                max_spectators: 4,
                spectator_policy: Some(SpectatorPolicy::All),
                has_password: true,
            }
        );
        assert!(!list[0].is_full());
        assert_eq!(list[1].map, None);
        assert_eq!(list[1].pick_type, None);
        assert!(!list[1].has_password);
        assert_eq!(from_json(&json!({"erro": 1})), None);
    }

    #[test]
    fn tipo_de_selecao_pelo_game_type() {
        for (game_type, pick) in [
            ("BLIND_PICK", Some(CustomPickType::BlindPick)),
            ("SimulPickStrategy", Some(CustomPickType::BlindPick)),
            ("DRAFT_MODE", Some(CustomPickType::DraftMode)),
            ("ALL_RANDOM", Some(CustomPickType::AllRandom)),
            ("TOURNAMENT_DRAFT", Some(CustomPickType::TournamentDraft)),
            (
                "TournamentPickStrategy",
                Some(CustomPickType::TournamentDraft),
            ),
            ("", None),
            ("PRACTICE_GAME", None),
        ] {
            assert_eq!(pick_type_from_game_type(game_type), pick, "{game_type}");
        }
    }

    #[test]
    fn corpo_de_entrar() {
        assert_eq!(join_path(42), "/lol-lobby/v1/custom-games/42/join");
        assert_eq!(
            join_body("segredo", false),
            json!({"asSpectator": false, "password": "segredo"})
        );
    }

    #[test]
    fn equipes_cheias() {
        let mut game = CustomGameListing::from_json(&json!({
            "id": 1, "filledPlayerSlots": 10, "maxPlayerSlots": 10
        }))
        .unwrap();
        assert!(game.is_full());
        game.players = 9;
        assert!(!game.is_full());
    }
}
