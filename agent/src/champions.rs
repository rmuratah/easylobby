//! Campeões para a grade da seleção (#28).
//!
//! - O catálogo ([`ChampionCatalog`]) sai do arquivo de dados do jogo que a
//!   própria LCU serve, já no idioma do client:
//!   `/lol-game-data/assets/v1/champion-summary.json` (id, nome, apelido em
//!   inglês e funções de cada campeão). O agente lê uma vez por conexão. Os
//!   ícones saem do `/assets` do agente ([`crate::assets`], tipo `champion`).
//! - As rotas de cada campeão (#250) saem das posições recomendadas que o
//!   client usa para sugerir runas:
//!   `/lol-perks/v1/recommended-champion-positions`. Se a LCU não mandar, as
//!   listas ficam vazias e a UI esconde o filtro de rota. As versões de modo
//!   dos campeões, como a `Jade_Aatrox` (id 60266) do Rift Clássico, não vêm
//!   nessa lista: ficam com as rotas do campeão base (#279), achado pelo
//!   apelido sem o prefixo do modo.
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

use crate::lobby::Position;
use crate::rest::{LcuClient, LcuError};

/// Id, nome e funções de cada campeão.
pub const CHAMPION_SUMMARY_PATH: &str = "/lol-game-data/assets/v1/champion-summary.json";

/// Rotas recomendadas de cada campeão (#250), por id.
pub const RECOMMENDED_POSITIONS_PATH: &str = "/lol-perks/v1/recommended-champion-positions";

/// Campeões que o jogador pode escolher na seleção atual (endpoint e URI dos
/// eventos WAMP).
pub const PICKABLE_CHAMPIONS_PATH: &str = "/lol-champ-select/v1/pickable-champion-ids";

/// Campeões que o jogador pode banir na seleção atual (endpoint e URI dos
/// eventos WAMP).
pub const BANNABLE_CHAMPIONS_PATH: &str = "/lol-champ-select/v1/bannable-champion-ids";

/// As cartas de campeão do ARAM (#201): as 2 ou 3 opções que a LCU sorteia
/// para o jogador escolher no início da seleção (endpoint e URI dos eventos
/// WAMP). Fora do ARAM, e fora da seleção, 404.
pub const SUBSET_CHAMPIONS_PATH: &str =
    "/lol-lobby-team-builder/champ-select/v1/subset-champion-list";

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
    /// Rotas em que o campeão costuma jogar (#250), como o client recomenda
    /// nas runas. Só `top`, `jungle`, `middle`, `bottom` e `utility`. Vazia
    /// quando a LCU não mandou.
    pub positions: Vec<Position>,
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

    /// Põe em cada campeão as rotas de `recommended-champion-positions`
    /// (#250). Aceita o valor de cada id como objeto
    /// (`{"recommendedPositions": ["TOP"]}`) ou como a lista direto. Nomes
    /// desconhecidos, `FILL` e repetidos ficam de fora. As versões de modo
    /// sem rotas próprias ficam com as do campeão base (#279).
    pub fn with_positions(mut self, recommended: &Value) -> Self {
        let Some(by_id) = recommended.as_object() else {
            return self;
        };
        for champion in &mut self.champions {
            let Some(entry) = by_id.get(&champion.id.to_string()) else {
                continue;
            };
            let list = entry
                .get("recommendedPositions")
                .or_else(|| entry.get("positions"))
                .unwrap_or(entry);
            champion.positions = positions_from_json(list);
        }
        self.inherit_base_positions();
        self
    }

    /// Dá a cada versão de modo sem rotas (`Jade_Aatrox`, id 60266) as rotas
    /// do campeão base (#279). O base é o do apelido sem o prefixo
    /// (`Aatrox`) ou, se o apelido não bater, o do id sem o milhar do modo
    /// (`60266 % 1000` = 266). Sem isso, o filtro de rota da seleção do Rift
    /// Clássico esvazia a grade, se a seleção oferecer essas versões.
    fn inherit_base_positions(&mut self) {
        let inherited: Vec<(usize, Vec<Position>)> = self
            .champions
            .iter()
            .enumerate()
            .filter(|(_, champion)| champion.positions.is_empty())
            .filter_map(|(index, champion)| {
                let (_, base_alias) = champion.alias.split_once('_')?;
                let base = self
                    .champions
                    .iter()
                    .find(|c| c.alias == base_alias)
                    .or_else(|| {
                        (champion.id >= 1000)
                            .then(|| self.get(champion.id % 1000))
                            .flatten()
                            .filter(|c| !c.alias.contains('_'))
                    })?;
                (!base.positions.is_empty()).then(|| (index, base.positions.clone()))
            })
            .collect();
        for (index, positions) in inherited {
            self.champions[index].positions = positions;
        }
    }
}

/// As rotas de uma lista de nomes da LCU (`["MIDDLE", "UTILITY"]`), na ordem
/// recebida.
fn positions_from_json(list: &Value) -> Vec<Position> {
    let mut positions = Vec::new();
    for name in list
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let position = Position::from_lcu(Some(&name.to_uppercase()));
        let lane = matches!(
            position,
            Position::Top
                | Position::Jungle
                | Position::Middle
                | Position::Bottom
                | Position::Utility
        );
        if lane && !positions.contains(&position) {
            positions.push(position);
        }
    }
    positions
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
            positions: Vec::new(),
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
/// As rotas vêm junto (#250); sem elas, o catálogo sai igual, sem rotas.
pub async fn catalog(lcu: &LcuClient) -> Result<Option<ChampionCatalog>, LcuError> {
    let (summary, recommended) = tokio::join!(
        lcu.get::<Value>(CHAMPION_SUMMARY_PATH),
        lcu.get::<Value>(RECOMMENDED_POSITIONS_PATH),
    );
    let catalog = ChampionCatalog::from_json(&summary?);
    Ok(match recommended {
        Ok(recommended) => catalog.map(|c| c.with_positions(&recommended)),
        Err(_) => catalog,
    })
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
    /// As cartas de campeão do ARAM (#201): os campeões que a LCU sorteou
    /// para o jogador escolher um, na ordem da LCU. `null` quando o agente
    /// ainda não sabe; vazia fora do ARAM.
    pub subset: Option<Vec<u32>>,
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
    let (pickable, bannable, subset) = tokio::join!(
        lcu.get::<Value>(PICKABLE_CHAMPIONS_PATH),
        lcu.get::<Value>(BANNABLE_CHAMPIONS_PATH),
        champion_offers(lcu),
    );
    ChampSelectChampions {
        pickable: pickable.ok().as_ref().and_then(ids_from_json),
        bannable: bannable.ok().as_ref().and_then(ids_from_json),
        subset,
    }
}

/// Lê as cartas de campeão do ARAM (#201). Sem cartas (404, fora do ARAM),
/// lista vazia; `None` só com outro erro da LCU.
pub async fn champion_offers(lcu: &LcuClient) -> Option<Vec<u32>> {
    match lcu.get::<Value>(SUBSET_CHAMPIONS_PATH).await {
        Ok(body) => offers_from_json(&body),
        Err(error) if error.status() == Some(404) => Some(Vec::new()),
        Err(_) => None,
    }
}

/// Como [`ids_from_json`], mas mantém a ordem da LCU (a das cartas).
pub fn offers_from_json(value: &Value) -> Option<Vec<u32>> {
    let mut ids: Vec<u32> = Vec::new();
    for id in value
        .as_array()?
        .iter()
        .filter_map(Value::as_u64)
        .filter_map(|id| u32::try_from(id).ok())
        .filter(|&id| id > 0)
    {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Some(ids)
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
    fn junta_as_rotas_recomendadas() {
        let summary = json!([
            {"id": 1, "name": "Annie", "alias": "Annie"},
            {"id": 62, "name": "Wukong", "alias": "MonkeyKing"},
            {"id": 266, "name": "Aatrox", "alias": "Aatrox"},
            {"id": 103, "name": "Ahri", "alias": "Ahri"},
        ]);
        let recommended = json!({
            "1": {"recommendedPositions": ["MIDDLE", "UTILITY"]},
            "62": ["JUNGLE", "TOP", "FILL", "JUNGLE", "lixo"],
            "266": {"recommendedPositions": ["top"]},
            "999": ["TOP"],
        });
        let catalog = ChampionCatalog::from_json(&summary)
            .unwrap()
            .with_positions(&recommended);
        assert_eq!(
            catalog.get(1).unwrap().positions,
            [Position::Middle, Position::Utility]
        );
        assert_eq!(
            catalog.get(62).unwrap().positions,
            [Position::Jungle, Position::Top]
        );
        assert_eq!(catalog.get(266).unwrap().positions, [Position::Top]);
        assert!(catalog.get(103).unwrap().positions.is_empty());
        // Resposta fora do formato não estraga o catálogo.
        let same = catalog.clone().with_positions(&json!([1, 2]));
        assert_eq!(same, catalog);
        assert_eq!(
            serde_json::to_value(catalog.get(1).unwrap()).unwrap()["positions"],
            json!(["middle", "utility"])
        );
    }

    #[test]
    fn versoes_de_modo_herdam_as_rotas_do_campeao_base() {
        let summary = json!([
            {"id": 266, "name": "Aatrox", "alias": "Aatrox"},
            {"id": 60266, "name": "Aatrox", "alias": "Jade_Aatrox"},
            {"id": 4, "name": "Twisted Fate", "alias": "TwistedFate"},
            {"id": 60004, "name": "Twisted Fate", "alias": "Jade_TwistedFate"},
            // Apelido que não bate: vale o id sem o milhar do modo.
            {"id": 1, "name": "Annie", "alias": "Annie"},
            {"id": 60001, "name": "Annie", "alias": "Jade_AnnieNova"},
            // Com rotas próprias, a versão de modo fica com as dela.
            {"id": 103, "name": "Ahri", "alias": "Ahri"},
            {"id": 60103, "name": "Ahri", "alias": "Jade_Ahri"},
            // Base sem rotas: continua vazia.
            {"id": 99, "name": "Lux", "alias": "Lux"},
            {"id": 60099, "name": "Lux", "alias": "Jade_Lux"},
            // Sem campeão base no catálogo.
            {"id": 60950, "name": "Novo", "alias": "Jade_Novo"},
        ]);
        let recommended = json!({
            "266": ["TOP"],
            "4": ["MIDDLE", "BOTTOM"],
            "1": ["MIDDLE"],
            "103": ["MIDDLE"],
            "60103": ["UTILITY"],
        });
        let catalog = ChampionCatalog::from_json(&summary)
            .unwrap()
            .with_positions(&recommended);
        assert_eq!(catalog.get(60266).unwrap().positions, [Position::Top]);
        assert_eq!(
            catalog.get(60004).unwrap().positions,
            [Position::Middle, Position::Bottom]
        );
        assert_eq!(catalog.get(60001).unwrap().positions, [Position::Middle]);
        assert_eq!(catalog.get(60103).unwrap().positions, [Position::Utility]);
        assert!(catalog.get(60099).unwrap().positions.is_empty());
        assert!(catalog.get(60950).unwrap().positions.is_empty());
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
            subset: Some(vec![77, 432, 126]),
        };
        assert_eq!(
            serde_json::to_value(&champions).unwrap(),
            json!({"pickable": [1, 2], "bannable": null, "subset": [77, 432, 126]})
        );
        // As cartas ficam na ordem da LCU.
        assert_eq!(
            offers_from_json(&json!([77, 432, 0, 126, 77])),
            Some(vec![77, 432, 126])
        );
        assert_eq!(offers_from_json(&json!({})), None);
    }
}
