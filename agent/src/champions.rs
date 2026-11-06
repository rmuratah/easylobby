//! Campeões para a grade da seleção (#28).
//!
//! - O catálogo ([`ChampionCatalog`]) sai do arquivo de dados do jogo que a
//!   própria LCU serve, já no idioma do client:
//!   `/lol-game-data/assets/v1/champion-summary.json` (id, nome, apelido em
//!   inglês e funções de cada campeão). O agente lê uma vez por conexão. Os
//!   ícones saem do `/assets` do agente ([`crate::assets`], tipo `champion`).
//! - Durante a seleção, a LCU diz quais campeões o jogador pode escolher
//!   (`/lol-champ-select/v1/pickable-champion-ids`: os dele e os da rotação
//!   gratuita) e quais pode banir
//!   (`/lol-champ-select/v1/bannable-champion-ids`). As duas listas chegam
//!   pelo WAMP quando a seleção abre (`Create`), mudam no meio (`Update`) e
//!   somem quando ela fecha (`Delete`). Elas ficam em
//!   [`ChampSelectChampions`].

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Id, nome e funções de cada campeão.
pub const CHAMPION_SUMMARY_PATH: &str = "/lol-game-data/assets/v1/champion-summary.json";

/// Campeões que o jogador pode escolher na seleção atual (endpoint e URI dos
/// eventos WAMP).
pub const PICKABLE_CHAMPIONS_PATH: &str = "/lol-champ-select/v1/pickable-champion-ids";

/// Campeões que o jogador pode banir na seleção atual (endpoint e URI dos
/// eventos WAMP).
pub const BANNABLE_CHAMPIONS_PATH: &str = "/lol-champ-select/v1/bannable-champion-ids";

/// Os campeões do jogo, para a grade da seleção. Lidos uma vez por conexão.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampionCatalog {
    /// Campeões em ordem alfabética do nome. Sem o "Nenhum" (`id` `-1`) que
    /// a LCU põe na lista.
    pub champions: Vec<Champion>,
}

/// Um campeão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Champion {
    /// O mesmo `championId` da seleção e do ícone em
    /// `/assets/champion/{id}`.
    pub id: u32,
    /// Nome no idioma do client.
    pub name: String,
    /// Nome interno em inglês, sem espaços (`MonkeyKing` para o Wukong).
    /// Serve para a busca achar o campeão também pelo nome em inglês.
    pub alias: String,
    /// Funções do campeão (`fighter`, `tank`, `mage`, `assassin`,
    /// `marksman`, `support`), como a LCU manda.
    pub roles: Vec<String>,
}

impl ChampionCatalog {
    /// Monta o catálogo a partir de `champion-summary.json`. `None` se o
    /// arquivo não for uma lista.
    pub fn from_json(summary: &Value) -> Option<Self> {
        let mut champions: Vec<Champion> = summary
            .as_array()?
            .iter()
            .filter_map(|raw| serde_json::from_value::<RawChampion>(raw.clone()).ok())
            .filter_map(Champion::from_raw)
            .collect();
        champions.sort_by_cached_key(|c| (c.name.to_lowercase(), c.id));
        Some(Self { champions })
    }

    pub fn get(&self, id: u32) -> Option<&Champion> {
        self.champions.iter().find(|c| c.id == id)
    }
}

impl Champion {
    fn from_raw(raw: RawChampion) -> Option<Self> {
        // `-1` é o "Nenhum" da LCU; `0` não é campeão.
        let id = u32::try_from(raw.id).ok().filter(|&id| id > 0)?;
        if raw.name.is_empty() {
            return None;
        }
        Some(Self {
            id,
            name: raw.name,
            alias: raw.alias,
            roles: raw.roles,
        })
    }
}

/// Um campeão como a LCU manda, só com os campos usados.
#[derive(Deserialize)]
struct RawChampion {
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    alias: String,
    #[serde(default)]
    roles: Vec<String>,
}

/// Lê os campeões do jogo, no idioma do client.
pub async fn catalog(lcu: &LcuClient) -> Result<Option<ChampionCatalog>, LcuError> {
    let summary = lcu.get::<Value>(CHAMPION_SUMMARY_PATH).await?;
    Ok(ChampionCatalog::from_json(&summary))
}

/// Quais campeões o jogador pode escolher e banir na seleção atual.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectChampions {
    /// Ids que o jogador pode escolher (os dele e os da rotação), ou `null`
    /// quando o agente ainda não sabe (fora da seleção, ou a LCU não
    /// mandou).
    pub pickable: Option<Vec<u32>>,
    /// Ids que o jogador pode banir, ou `null` quando o agente ainda não
    /// sabe.
    pub bannable: Option<Vec<u32>>,
}

/// Lê uma lista de ids (`[1, 10, 102...]`). `None` se não for uma lista;
/// itens que não são ids ficam de fora.
pub fn ids_from_json(value: &Value) -> Option<Vec<u32>> {
    let list = value.as_array()?;
    let mut ids: Vec<u32> = list
        .iter()
        .filter_map(Value::as_u64)
        .filter_map(|id| u32::try_from(id).ok())
        .filter(|&id| id > 0)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    Some(ids)
}

/// Lê os campeões que o jogador pode escolher e banir. Fora da seleção a LCU
/// responde 404, e a lista fica `None`.
pub async fn current(lcu: &LcuClient) -> ChampSelectChampions {
    let (pickable, bannable) = tokio::join!(
        lcu.get::<Value>(PICKABLE_CHAMPIONS_PATH),
        lcu.get::<Value>(BANNABLE_CHAMPIONS_PATH),
    );
    ChampSelectChampions {
        pickable: pickable.ok().as_ref().and_then(ids_from_json),
        bannable: bannable.ok().as_ref().and_then(ids_from_json),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn monta_o_catalogo_em_ordem_alfabetica() {
        let summary = json!([
            {"id": -1, "name": "Nenhum", "alias": "None", "roles": []},
            {"id": 62, "name": "Wukong", "alias": "MonkeyKing", "roles": ["fighter", "tank"],
             "squarePortraitPath": "/lol-game-data/assets/v1/champion-icons/62.png"},
            {"id": 266, "name": "Aatrox", "alias": "Aatrox", "roles": ["fighter"]},
            {"id": 103, "name": "Ahri", "alias": "Ahri"},
            {"id": 7, "name": ""},
            "lixo",
        ]);
        let catalog = ChampionCatalog::from_json(&summary).unwrap();
        let ids: Vec<u32> = catalog.champions.iter().map(|c| c.id).collect();
        assert_eq!(ids, [266, 103, 62]);
        assert_eq!(catalog.get(62).unwrap().alias, "MonkeyKing");
        assert_eq!(catalog.get(62).unwrap().roles, ["fighter", "tank"]);
        assert!(catalog.get(103).unwrap().roles.is_empty());
        assert!(ChampionCatalog::from_json(&json!({})).is_none());
    }

    #[test]
    fn le_listas_de_ids() {
        assert_eq!(
            ids_from_json(&json!([266, 1, 0, -1, "x", 1, 103])),
            Some(vec![1, 103, 266])
        );
        assert_eq!(ids_from_json(&json!([])), Some(vec![]));
        assert_eq!(ids_from_json(&json!(null)), None);
    }

    #[test]
    fn formato_para_a_ui() {
        let champions = ChampSelectChampions {
            pickable: Some(vec![1, 2]),
            bannable: None,
        };
        assert_eq!(
            serde_json::to_value(&champions).unwrap(),
            json!({"pickable": [1, 2], "bannable": null})
        );
    }
}
