//! Invocador logado (#7).
//!
//! Lê `GET /lol-summoner/v1/current-summoner` pelo [`LcuClient`] e guarda só
//! os campos que o easylobby usa. Os outros campos da resposta são ignorados,
//! então um campo novo num patch não quebra a leitura.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::rest::{LcuClient, LcuError};

/// Endpoint do invocador logado.
pub const CURRENT_SUMMONER_PATH: &str = "/lol-summoner/v1/current-summoner";

/// Invocador logado no client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summoner {
    /// Nome do Riot ID (antes do `#`).
    #[serde(default)]
    pub game_name: String,
    /// Tag do Riot ID (depois do `#`).
    #[serde(default)]
    pub tag_line: String,
    /// Nome antigo de invocador. Vazio nas contas que só têm Riot ID.
    #[serde(default)]
    pub display_name: String,
    pub summoner_level: u32,
    #[serde(default)]
    pub profile_icon_id: u32,
    pub puuid: String,
    pub summoner_id: u64,
}

impl Summoner {
    /// Nome para mostrar: o Riot ID `nome#tag`, ou o nome antigo de
    /// invocador quando o Riot ID não veio.
    pub fn name(&self) -> String {
        match (self.game_name.is_empty(), self.tag_line.is_empty()) {
            (false, false) => format!("{}#{}", self.game_name, self.tag_line),
            (false, true) => self.game_name.clone(),
            (true, _) => self.display_name.clone(),
        }
    }
}

impl fmt::Display for Summoner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, nível {}", self.name(), self.summoner_level)
    }
}

/// Busca o invocador logado.
///
/// Antes do login a LCU responde `404`; o erro volta como
/// [`LcuError::Status`].
pub async fn current_summoner(lcu: &LcuClient) -> Result<Summoner, LcuError> {
    lcu.get(CURRENT_SUMMONER_PATH).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resposta() -> serde_json::Value {
        json!({
            "accountId": 1001,
            "displayName": "",
            "gameName": "fake-name-0001",
            "internalName": "",
            "profileIconId": 3150,
            "puuid": "fake-puuid-0001",
            "rerollPoints": { "currentPoints": 432 },
            "summonerId": 1002,
            "summonerLevel": 122,
            "tagLine": "fake-tag-0001",
            "campoNovoDeUmPatch": true
        })
    }

    #[test]
    fn le_a_resposta_da_lcu() {
        let s: Summoner = serde_json::from_value(resposta()).unwrap();
        assert_eq!(s.game_name, "fake-name-0001");
        assert_eq!(s.tag_line, "fake-tag-0001");
        assert_eq!(s.summoner_level, 122);
        assert_eq!(s.profile_icon_id, 3150);
        assert_eq!(s.summoner_id, 1002);
        assert_eq!(s.to_string(), "fake-name-0001#fake-tag-0001, nível 122");
    }

    #[test]
    fn nome_sem_riot_id_usa_o_nome_antigo() {
        let mut body = resposta();
        body["gameName"] = json!("");
        body["tagLine"] = json!("");
        body["displayName"] = json!("fake-nome-antigo");
        let s: Summoner = serde_json::from_value(body).unwrap();
        assert_eq!(s.name(), "fake-nome-antigo");

        let mut body = resposta();
        body.as_object_mut().unwrap().remove("tagLine");
        let s: Summoner = serde_json::from_value(body).unwrap();
        assert_eq!(s.name(), "fake-name-0001");
    }

    #[test]
    fn sem_nivel_e_erro() {
        let mut body = resposta();
        body.as_object_mut().unwrap().remove("summonerLevel");
        assert!(serde_json::from_value::<Summoner>(body).is_err());
    }
}
