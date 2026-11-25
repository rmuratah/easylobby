//! Riot ID e ícone de jogadores que não são amigos (#675).
//!
//! Os convites recebidos, os sugeridos e os recentes trazem só o
//! `summonerId` ou o `puuid` (às vezes um nome), sem o ícone de invocador.
//! Para amigos a UI usa a lista de amigos; para os outros o agente busca o
//! invocador na LCU e publica um [`PlayerCard`]:
//!
//! - pelo `puuid`: `GET /lol-summoner/v2/summoners/puuid/{puuid}`;
//! - pelo `summonerId`, quando não há `puuid` (convites recebidos):
//!   `GET /lol-summoner/v1/summoners/{summonerId}`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::player_profile::{self, SUMMONER_BY_PUUID_PREFIX};
use crate::rest::{LcuClient, LcuError};
use crate::summoner::Summoner;

/// Começo do invocador pelo `summonerId`: `<prefixo>/<summonerId>`.
pub const SUMMONER_BY_ID_PREFIX: &str = "/lol-summoner/v1/summoners";

/// Nick e ícone de um jogador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCard {
    pub puuid: String,
    #[ts(type = "number")]
    pub summoner_id: u64,
    /// Nome do Riot ID (antes do `#`), ou o nome antigo de invocador numa
    /// conta sem Riot ID.
    pub game_name: String,
    /// Tag do Riot ID. Vazia quando `game_name` é o nome antigo.
    pub tag_line: String,
    pub profile_icon_id: u32,
}

/// Quem buscar: o `puuid` quando a LCU mandou, senão o `summonerId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlayerKey {
    Puuid(String),
    SummonerId(u64),
}

impl PlayerKey {
    /// A chave de um jogador: o `puuid` se ele serve para montar o caminho,
    /// senão o `summonerId`. `None` sem nenhum dos dois.
    pub fn of(puuid: &str, summoner_id: u64) -> Option<PlayerKey> {
        if player_profile::valid_puuid(puuid) {
            Some(PlayerKey::Puuid(puuid.to_owned()))
        } else if summoner_id != 0 {
            Some(PlayerKey::SummonerId(summoner_id))
        } else {
            None
        }
    }

    /// Se `card` é deste jogador.
    pub fn matches(&self, card: &PlayerCard) -> bool {
        match self {
            PlayerKey::Puuid(puuid) => card.puuid == *puuid,
            PlayerKey::SummonerId(id) => card.summoner_id == *id,
        }
    }

    /// Caminho da LCU que traz o invocador.
    pub fn path(&self) -> String {
        match self {
            PlayerKey::Puuid(puuid) => format!("{SUMMONER_BY_PUUID_PREFIX}/{puuid}"),
            PlayerKey::SummonerId(id) => format!("{SUMMONER_BY_ID_PREFIX}/{id}"),
        }
    }
}

impl PlayerCard {
    /// Lê o invocador da LCU. `None` sem os campos obrigatórios ou sem nome
    /// nenhum.
    pub fn from_json(value: &Value) -> Option<PlayerCard> {
        let summoner = Summoner::deserialize(value).ok()?;
        let (game_name, tag_line) = if summoner.game_name.is_empty() {
            (summoner.display_name, String::new())
        } else {
            (summoner.game_name, summoner.tag_line)
        };
        if game_name.is_empty() {
            return None;
        }
        Some(PlayerCard {
            puuid: summoner.puuid,
            summoner_id: summoner.summoner_id,
            game_name,
            tag_line,
            profile_icon_id: summoner.profile_icon_id,
        })
    }
}

/// Busca o nick e o ícone de `key`.
pub async fn fetch(lcu: &LcuClient, key: &PlayerKey) -> Result<Option<PlayerCard>, LcuError> {
    let body = lcu.get::<Value>(&key.path()).await?;
    Ok(PlayerCard::from_json(&body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_o_invocador() {
        let card = PlayerCard::from_json(&json!({
            "gameName": "Zed", "tagLine": "BR1", "displayName": "", "summonerLevel": 30,
            "profileIconId": 4567, "puuid": "p-1", "summonerId": 77
        }))
        .unwrap();
        assert_eq!(
            card,
            PlayerCard {
                puuid: "p-1".into(),
                summoner_id: 77,
                game_name: "Zed".into(),
                tag_line: "BR1".into(),
                profile_icon_id: 4567,
            }
        );
    }

    #[test]
    fn sem_riot_id_usa_o_nome_antigo_e_sem_nome_nao_serve() {
        let antigo = PlayerCard::from_json(&json!({
            "displayName": "Velho", "summonerLevel": 30, "puuid": "p", "summonerId": 1
        }))
        .unwrap();
        assert_eq!(
            (antigo.game_name.as_str(), antigo.tag_line.as_str()),
            ("Velho", "")
        );
        assert_eq!(antigo.profile_icon_id, 0);
        assert!(PlayerCard::from_json(&json!({
            "summonerLevel": 30, "puuid": "p", "summonerId": 1
        }))
        .is_none());
        assert!(PlayerCard::from_json(&json!({})).is_none());
    }

    #[test]
    fn chave_e_caminho() {
        assert_eq!(
            PlayerKey::of("abc-1", 5).unwrap().path(),
            "/lol-summoner/v2/summoners/puuid/abc-1"
        );
        assert_eq!(
            PlayerKey::of("", 5).unwrap().path(),
            "/lol-summoner/v1/summoners/5"
        );
        // Um puuid que mudaria o caminho cai no summonerId.
        assert_eq!(PlayerKey::of("../x", 5), Some(PlayerKey::SummonerId(5)));
        assert_eq!(PlayerKey::of("", 0), None);
    }
}
