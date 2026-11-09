//! Lista de personalizadas abertas e entrar nelas (#264), a aba "Entrar" da
//! tela de personalizada do client oficial.
//!
//! Conferido no LoL real (patch 16.22, gravação do #264):
//!
//! - `POST /lol-lobby/v1/custom-games/refresh` responde 204, e o `GET` logo
//!   depois já traz a lista;
//! - `GET /lol-lobby/v1/custom-games` devolve `LolLobbyLobbyCustomGame[]`:
//!   `lobbyName`, `ownerDisplayName` ("Nome #TAG"), `mapId`,
//!   `filledPlayerSlots`, `maxPlayerSlots`, `filledSpectatorSlots`,
//!   `maxSpectatorSlots`, `spectatorPolicy` (`"ALL"`, `"NONE"`...),
//!   `hasPassword` e `partyId`. O `id` vem 0 em todas, e o `gameType` é
//!   sempre `"CUSTOM"`: a lista não diz o tipo de seleção;
//! - `/lol-lobby/v1/custom-games` não tem rota de entrar. Entra-se pelo grupo
//!   da partida: `POST /lol-lobby/v2/party/{partyId}/join` com
//!   `LolLobbyCustomJoinOptionsDto` (`{"lobbyPassword": "..."}`; o campo
//!   opcional `team` não é usado). O lobby chega pelos eventos de sempre de
//!   `/lol-lobby/v2/lobby`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::custom_game::{CustomMap, SpectatorPolicy};
use crate::rest::{LcuClient, LcuError};

/// A lista de personalizadas abertas. Também é o caminho do evento do WAMP.
pub const CUSTOM_GAMES_PATH: &str = "/lol-lobby/v1/custom-games";

/// Pede para a LCU buscar a lista de novo.
pub const CUSTOM_GAMES_REFRESH_PATH: &str = "/lol-lobby/v1/custom-games/refresh";

/// Começo do caminho de entrar no grupo de uma partida.
pub const PARTY_PATH: &str = "/lol-lobby/v2/party";

/// Maior senha aceita pelo agente, em caracteres. A LCU não diz o limite; o
/// client oficial corta bem antes disso.
pub const MAX_PASSWORD_LEN: usize = 64;

/// Uma personalizada aberta na lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomGameListing {
    /// Grupo da partida (`partyId`, um UUID), para o `joinCustomGame`. É o
    /// único campo que distingue as partidas: o `id` da LCU vem 0 em todas.
    pub party_id: String,
    /// Nome da partida (`lobbyName`).
    pub name: String,
    /// Nome de quem criou (`ownerDisplayName`), como "Nome #TAG".
    pub owner: String,
    /// `mapId` da LCU: 11 no Summoner's Rift, 12 no Abismo Uivante.
    pub map_id: u32,
    /// O mapa, ou `null` fora dos três que o formulário oferece.
    pub map: Option<CustomMap>,
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
    /// Lê uma entrada da lista. `None` sem `partyId` válido.
    pub fn from_json(value: &Value) -> Option<CustomGameListing> {
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let party_id = Some(text("partyId")).filter(|id| valid_party_id(id))?;
        let count = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0)
        };
        let map_id = value.get("mapId").and_then(Value::as_u64).unwrap_or(0);
        Some(CustomGameListing {
            party_id,
            name: text("lobbyName"),
            owner: text("ownerDisplayName"),
            map_id: u32::try_from(map_id).unwrap_or(0),
            map: CustomMap::from_map_id(map_id),
            players: count("filledPlayerSlots"),
            max_players: count("maxPlayerSlots"),
            spectators: count("filledSpectatorSlots"),
            max_spectators: count("maxSpectatorSlots"),
            spectator_policy: spectator_policy(&text("spectatorPolicy")),
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

/// A lista inteira. `None` se o corpo não é uma lista; entradas sem
/// `partyId` ficam de fora.
pub fn from_json(value: &Value) -> Option<Vec<CustomGameListing>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(CustomGameListing::from_json)
            .collect(),
    )
}

/// `spectatorPolicy` da lista. O LoL real manda `"ALL"` e `"NONE"`; os
/// outros dois nomes seguem o mesmo padrão, e os nomes do `gameConfig`
/// (`AllAllowed`...) também valem.
fn spectator_policy(value: &str) -> Option<SpectatorPolicy> {
    match value {
        "ALL" => Some(SpectatorPolicy::All),
        "NONE" => Some(SpectatorPolicy::None),
        "FRIENDS" => Some(SpectatorPolicy::FriendsOnly),
        "LOBBY" => Some(SpectatorPolicy::LobbyOnly),
        other => SpectatorPolicy::from_lcu(other),
    }
}

/// Se `party_id` pode ir no caminho sem escapar nada: um UUID, ou pelo menos
/// letras, números e hífens.
pub fn valid_party_id(party_id: &str) -> bool {
    !party_id.is_empty()
        && party_id.len() <= 64
        && party_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Caminho de entrar no grupo `party_id`.
pub fn join_path(party_id: &str) -> String {
    format!("{PARTY_PATH}/{party_id}/join")
}

/// Corpo do `POST` de entrar (`LolLobbyCustomJoinOptionsDto`).
pub fn join_body(password: &str) -> Value {
    json!({ "lobbyPassword": password })
}

/// Pede a lista nova ao servidor e lê a que a LCU tem. Um `refresh` recusado
/// não impede de ler a lista que já está lá.
pub async fn refresh(lcu: &LcuClient) -> Result<Vec<CustomGameListing>, LcuError> {
    let _ = lcu
        .post::<_, Value>(CUSTOM_GAMES_REFRESH_PATH, &json!({}))
        .await;
    let body: Value = lcu.get(CUSTOM_GAMES_PATH).await?;
    Ok(from_json(&body).unwrap_or_default())
}

/// Entra na partida do grupo `party_id`. Quem chama confere o `party_id` com
/// [`valid_party_id`].
pub async fn join(lcu: &LcuClient, party_id: &str, password: &str) -> Result<(), LcuError> {
    lcu.post::<_, Value>(&join_path(party_id), &join_body(password))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As duas primeiras partidas da lista gravada no LoL real (16.22).
    fn recorded() -> Value {
        json!([
            {"filledPlayerSlots":1,"filledSpectatorSlots":0,"gameType":"CUSTOM","hasPassword":true,"id":0,
             "lobbyName":"Partida de fake-name-0040","mapId":11,"maxPlayerSlots":10,"maxSpectatorSlots":15,
             "ownerDisplayName":"fake-name-0040 #br1","partyId":"06645d51-8137-48c7-9a29-e41cebf815dc",
             "passbackUrl":"","spectatorPolicy":"ALL"},
            {"filledPlayerSlots":1,"filledSpectatorSlots":0,"gameType":"CUSTOM","hasPassword":false,"id":0,
             "lobbyName":"Partida de fake-name-0041","mapId":12,"maxPlayerSlots":10,"maxSpectatorSlots":15,
             "ownerDisplayName":"fake-name-0041 #1010","partyId":"06cc18c2-6410-45ec-84f8-eced69f594e9",
             "passbackUrl":"","spectatorPolicy":"NONE"}
        ])
    }

    #[test]
    fn le_a_lista_gravada_com_id_zero() {
        let list = from_json(&recorded()).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0],
            CustomGameListing {
                party_id: "06645d51-8137-48c7-9a29-e41cebf815dc".into(),
                name: "Partida de fake-name-0040".into(),
                owner: "fake-name-0040 #br1".into(),
                map_id: 11,
                map: Some(CustomMap::SummonersRift),
                players: 1,
                max_players: 10,
                spectators: 0,
                max_spectators: 15,
                spectator_policy: Some(SpectatorPolicy::All),
                has_password: true,
            }
        );
        assert_eq!(list[1].map, Some(CustomMap::HowlingAbyss));
        assert_eq!(list[1].spectator_policy, Some(SpectatorPolicy::None));
        assert!(!list[1].has_password);
        assert_eq!(from_json(&json!({"erro": 1})), None);
    }

    #[test]
    fn sem_party_id_valido_fica_de_fora() {
        let list = from_json(&json!([
            {"id": 0, "lobbyName": "sem grupo"},
            {"partyId": "../lobby", "lobbyName": "caminho"},
            {"partyId": "abc-123", "mapId": 30, "spectatorPolicy": "?"}
        ]))
        .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].map, None);
        assert_eq!(list[0].spectator_policy, None);
    }

    #[test]
    fn corpo_de_entrar() {
        assert_eq!(
            join_path("06645d51-8137-48c7-9a29-e41cebf815dc"),
            "/lol-lobby/v2/party/06645d51-8137-48c7-9a29-e41cebf815dc/join"
        );
        assert_eq!(join_body("segredo"), json!({"lobbyPassword": "segredo"}));
        assert!(valid_party_id("06645d51-8137-48c7-9a29-e41cebf815dc"));
        assert!(!valid_party_id(""));
        assert!(!valid_party_id("a/b"));
        assert!(!valid_party_id("a b"));
    }

    #[test]
    fn equipes_cheias() {
        let mut game = CustomGameListing::from_json(&json!({
            "partyId": "p", "filledPlayerSlots": 10, "maxPlayerSlots": 10
        }))
        .unwrap();
        assert!(game.is_full());
        game.players = 9;
        assert!(!game.is_full());
    }
}
