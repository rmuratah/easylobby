//! Recomendador de runas da seleção de campeões (#596).
//!
//! No client oficial, com o campeão escolhido ou marcado, o botão
//! "Recomendador" mostra algumas páginas sugeridas para o campeão na rota
//! ("Executar inimigos e Ouro", "Dano de Habilidade e Mana"...), cada uma com
//! a runa principal, as runas das duas árvores e os feitiços. Escolher uma
//! cria a página **temporária** "Campeão – Runa principal", que não ocupa
//! espaço de página comprada e some depois da partida.
//!
//! - As sugestões vêm de
//!   `GET /lol-perks/v1/recommended-pages/champion/{id}/position/{posição}/map/11`
//!   (o mesmo endpoint que o Jogo Dinâmico usa, veja
//!   [`crate::quick_play::recommended_pages_path`]): uma lista de páginas com
//!   `primaryPerkStyleId`, `secondaryPerkStyleId`, `perks` (objetos com
//!   `id`), `summonerSpellIds` e o nome em `primaryRecommendationAttribute`
//!   e `secondaryRecommendationAttribute`.
//! - A página aplicada vai com `isTemporary: true` e os campos de
//!   recomendação que a LCU guarda nas páginas temporárias gravadas
//!   (`recommendationChampionId`, `recommendationIndex`,
//!   `runeRecommendationId`). Se já existe uma temporária, ela é editada e
//!   aplicada em vez de criar outra.
//!
//! Os nomes dos campos de recomendação e o corpo da página temporária ainda
//! não foram conferidos no LoL real.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::champions::ChampionCatalog;
use crate::lobby::Position;
use crate::quick_play::recommended_pages_path;
use crate::rest::{LcuClient, LcuError};
use crate::runes::{
    check_page, page_path, select_page, RuneCatalog, RunePageError, RunePageInput, RunePages,
    PAGES_PATH,
};

/// As páginas sugeridas para um campeão numa rota.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedRunes {
    pub champion_id: u32,
    /// A rota pedida em `openRecommendedRunes`.
    pub position: Position,
    /// As sugestões na ordem do client oficial. Vazia quando a LCU não tem
    /// nenhuma para o campeão na rota.
    pub pages: Vec<RecommendedRunePage>,
}

/// Uma página sugerida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedRunePage {
    /// O nome da sugestão no client oficial, como "Executar inimigos e
    /// Ouro". Vazio quando a LCU não manda; a UI usa a runa principal.
    pub title: String,
    pub primary_style_id: u32,
    pub sub_style_id: u32,
    /// Na ordem de [`crate::runes::RunePage::selected_perk_ids`]: a runa
    /// principal, as 3 da árvore primária, as 2 da secundária e os 3
    /// fragmentos.
    pub selected_perk_ids: Vec<u32>,
    /// Os feitiços sugeridos (D e F). Vazia quando a LCU não manda. Só
    /// para mostrar: aplicar a página não troca os feitiços.
    pub summoner_spell_ids: Vec<u32>,
    /// Id da recomendação na LCU, devolvido na página temporária. Vazio
    /// quando a LCU não manda.
    pub recommendation_id: String,
}

impl RecommendedRunes {
    /// Lê a resposta das páginas recomendadas. Páginas sem árvore ou sem
    /// runas ficam de fora. `None` se a resposta não for uma lista.
    pub fn from_json(champion_id: u32, position: Position, value: &Value) -> Option<Self> {
        let pages = value
            .as_array()?
            .iter()
            .filter_map(RecommendedRunePage::from_json)
            .collect();
        Some(RecommendedRunes {
            champion_id,
            position,
            pages,
        })
    }
}

impl RecommendedRunePage {
    fn from_json(value: &Value) -> Option<Self> {
        let primary_style_id = as_u32(&value["primaryPerkStyleId"]);
        let sub_style_id = as_u32(&value["secondaryPerkStyleId"]);
        let selected_perk_ids: Vec<u32> = value["perks"]
            .as_array()?
            .iter()
            .map(|perk| as_u32(&perk["id"]))
            .filter(|&id| id > 0)
            .collect();
        if primary_style_id == 0 || selected_perk_ids.is_empty() {
            return None;
        }
        let attribute = |key: &str| value[key].as_str().unwrap_or_default().trim().to_owned();
        let title = [
            attribute("primaryRecommendationAttribute"),
            attribute("secondaryRecommendationAttribute"),
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" e ");
        let summoner_spell_ids = value["summonerSpellIds"]
            .as_array()
            .map(|ids| ids.iter().map(as_u32).filter(|&id| id > 0).collect())
            .unwrap_or_default();
        let recommendation_id = ["runeRecommendationId", "recommendationId"]
            .iter()
            .find_map(|key| value[*key].as_str())
            .unwrap_or_default()
            .to_owned();
        Some(RecommendedRunePage {
            title,
            primary_style_id,
            sub_style_id,
            selected_perk_ids,
            summoner_spell_ids,
            recommendation_id,
        })
    }
}

/// Rotas que o recomendador aceita: as cinco do Summoner's Rift.
pub fn is_lane(position: Position) -> bool {
    !matches!(position, Position::Fill | Position::Unselected)
}

/// Lê as sugestões para o campeão na rota.
pub async fn recommended(
    lcu: &LcuClient,
    champion_id: u32,
    position: Position,
) -> Result<Option<RecommendedRunes>, LcuError> {
    let body: Value = lcu
        .get(&recommended_pages_path(champion_id, position))
        .await?;
    Ok(RecommendedRunes::from_json(champion_id, position, &body))
}

/// Nome da página temporária, como o client oficial: "Campeão – Runa
/// principal". Sem os nomes, "Recomendada".
pub fn page_name(
    champion_id: u32,
    page: &RecommendedRunePage,
    champions: Option<&ChampionCatalog>,
    catalog: Option<&RuneCatalog>,
) -> String {
    let champion = champions
        .and_then(|c| c.champions.iter().find(|c| c.id == champion_id))
        .map(|c| c.name.as_str());
    let keystone = catalog
        .zip(page.selected_perk_ids.first())
        .and_then(|(catalog, id)| catalog.perks.iter().find(|p| p.id == *id))
        .map(|p| p.name.as_str());
    match (champion, keystone) {
        (Some(champion), Some(keystone)) => format!("{champion} – {keystone}"),
        (Some(name), None) | (None, Some(name)) => name.to_owned(),
        (None, None) => "Recomendada".to_owned(),
    }
}

/// A página editável que a sugestão vai virar.
pub fn page_input(name: String, page: &RecommendedRunePage) -> RunePageInput {
    RunePageInput {
        name,
        primary_style_id: page.primary_style_id,
        sub_style_id: page.sub_style_id,
        selected_perk_ids: page.selected_perk_ids.clone(),
    }
}

/// A página temporária que já existe e pode ser trocada pela sugestão.
pub fn temporary_page(pages: Option<&RunePages>) -> Option<u64> {
    pages?
        .pages
        .iter()
        .find(|p| p.is_temporary && p.is_editable)
        .map(|p| p.id)
}

/// Campos que acompanham a página temporária.
fn recommendation_fields(champion_id: u32, index: usize, page: &RecommendedRunePage) -> Value {
    json!({
        "isTemporary": true,
        "isRecommendationOverride": false,
        "recommendationChampionId": champion_id,
        "recommendationIndex": index,
        "runeRecommendationId": page.recommendation_id,
    })
}

/// Aplica a sugestão `index` como página temporária: edita a temporária
/// `existing` e a aplica, ou cria uma nova já em uso. Confere a página com
/// [`check_page`] antes.
pub async fn apply(
    lcu: &LcuClient,
    existing: Option<u64>,
    input: &RunePageInput,
    champion_id: u32,
    index: usize,
    page: &RecommendedRunePage,
    catalog: Option<&RuneCatalog>,
) -> Result<(), RunePageError> {
    check_page(input, catalog).map_err(RunePageError::Invalid)?;
    let mut body = recommendation_fields(champion_id, index, page);
    let fields = body.as_object_mut().expect("os campos são um objeto");
    fields.insert("name".into(), json!(input.name.trim()));
    fields.insert("primaryStyleId".into(), json!(input.primary_style_id));
    fields.insert("subStyleId".into(), json!(input.sub_style_id));
    fields.insert("selectedPerkIds".into(), json!(input.selected_perk_ids));
    match existing {
        Some(id) => {
            let path = page_path(id);
            let mut current: Value = lcu.get(&path).await?;
            if let (Some(current), Some(fields)) = (current.as_object_mut(), body.as_object()) {
                for (key, value) in fields {
                    current.insert(key.clone(), value.clone());
                }
            }
            lcu.put::<_, Option<Value>>(&path, &current).await?;
            select_page(lcu, id).await?;
        }
        None => {
            fields.insert("current".into(), json!(true));
            lcu.post::<_, Option<Value>>(PAGES_PATH, &body).await?;
        }
    }
    Ok(())
}

fn as_u32(value: &Value) -> u32 {
    value
        .as_u64()
        .or_else(|| value.as_i64().map(|v| v.max(0) as u64))
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::champions::Champion;
    use crate::runes::{Perk, RunePage};

    fn response() -> Value {
        json!([
            {
                "isDefaultPosition": true,
                "position": "MIDDLE",
                "primaryPerkStyleId": 8100,
                "secondaryPerkStyleId": 8200,
                "primaryRecommendationAttribute": "Executar inimigos",
                "secondaryRecommendationAttribute": "Ouro",
                "recommendationChampionId": 63,
                "runeRecommendationId": "2026-11-18.000001",
                "keystone": {"id": 8128, "name": "Colheita Sombria"},
                "perks": [
                    {"id": 8128}, {"id": 8126}, {"id": 8138}, {"id": 8135},
                    {"id": 8226}, {"id": 8210}, {"id": 5008}, {"id": 5008}, {"id": 5011}
                ],
                "summonerSpellIds": [4, 14]
            },
            {"primaryPerkStyleId": 8200, "secondaryPerkStyleId": 8300, "perks": [{"id": 8229}]},
            {"primaryPerkStyleId": 0, "perks": [{"id": 1}]},
            {"primaryPerkStyleId": 8000}
        ])
    }

    #[test]
    fn le_as_sugestoes() {
        let runes = RecommendedRunes::from_json(63, Position::Middle, &response()).unwrap();
        assert_eq!(runes.champion_id, 63);
        assert_eq!(runes.position, Position::Middle);
        assert_eq!(
            runes.pages.len(),
            2,
            "páginas sem árvore ou runas ficam de fora"
        );
        let first = &runes.pages[0];
        assert_eq!(first.title, "Executar inimigos e Ouro");
        assert_eq!(first.primary_style_id, 8100);
        assert_eq!(first.sub_style_id, 8200);
        assert_eq!(first.selected_perk_ids.len(), 9);
        assert_eq!(first.selected_perk_ids[0], 8128);
        assert_eq!(first.summoner_spell_ids, vec![4, 14]);
        assert_eq!(first.recommendation_id, "2026-11-18.000001");
        let second = &runes.pages[1];
        assert_eq!(second.title, "");
        assert!(second.summoner_spell_ids.is_empty());
        assert!(RecommendedRunes::from_json(63, Position::Middle, &json!({})).is_none());
    }

    #[test]
    fn nome_da_pagina_como_o_client() {
        let runes = RecommendedRunes::from_json(63, Position::Middle, &response()).unwrap();
        let page = &runes.pages[0];
        let champions = ChampionCatalog {
            champions: vec![Champion {
                id: 63,
                name: "Brand".into(),
                alias: "Brand".into(),
                roles: Vec::new(),
                positions: Vec::new(),
            }],
        };
        let catalog = RuneCatalog {
            styles: Vec::new(),
            perks: vec![Perk {
                id: 8128,
                name: "Colheita Sombria".into(),
                short_desc: String::new(),
                long_desc: String::new(),
                end_of_game_stat_descs: Vec::new(),
            }],
        };
        assert_eq!(
            page_name(63, page, Some(&champions), Some(&catalog)),
            "Brand – Colheita Sombria"
        );
        assert_eq!(page_name(63, page, Some(&champions), None), "Brand");
        assert_eq!(page_name(63, page, None, None), "Recomendada");
    }

    #[test]
    fn acha_a_pagina_temporaria() {
        let page = |id, is_temporary, is_editable| RunePage {
            id,
            name: String::new(),
            primary_style_id: 8000,
            sub_style_id: 8100,
            selected_perk_ids: Vec::new(),
            is_editable,
            is_deletable: true,
            is_temporary,
            is_valid: true,
        };
        let mut pages = RunePages {
            pages: vec![page(1, false, true), page(2, true, false)],
            current_page_id: Some(1),
            can_create_page: false,
        };
        assert_eq!(temporary_page(Some(&pages)), None);
        pages.pages.push(page(3, true, true));
        assert_eq!(temporary_page(Some(&pages)), Some(3));
        assert_eq!(temporary_page(None), None);
    }

    #[test]
    fn so_rotas_do_rift() {
        assert!(is_lane(Position::Utility));
        assert!(!is_lane(Position::Fill));
        assert!(!is_lane(Position::Unselected));
    }
}
