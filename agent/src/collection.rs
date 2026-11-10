//! Coleção de campeões e skins da conta (#369), para a aba Coleção.
//!
//! Tudo é lido sob demanda, quando a UI abre a aba (`openCollection`) ou um
//! campeão (`openChampionSkins`), pelas rotas do inventário do jogador
//! logado (`/help` 16.21, a confirmar no LoL real):
//!
//! - campeões: `GET /lol-champions/v1/inventories/{summonerId}/champions-minimal`,
//!   com todos os campeões do jogo e, em cada um, `ownership.owned`,
//!   `freeToPlay` e `ownership.rental.rented`;
//! - todas as skins da conta: `GET .../{summonerId}/skins-minimal`, só para
//!   contar as skins de cada campeão (a skin padrão não conta);
//! - skins de um campeão: `GET .../{summonerId}/champions/{championId}/skins`,
//!   com os cromas de cada skin em `chromas`.
//!
//! As imagens das skins e dos cromas saem do `/assets` do agente (tipos
//! `skin` e `skinSplash`, veja [`crate::assets`]).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::champions::ChampionCatalog;
use crate::lobby::Position;
use crate::rest::{LcuClient, LcuError};

/// Começo das rotas do inventário: `<prefixo>/<summonerId>/...`.
pub const INVENTORIES_PREFIX: &str = "/lol-champions/v1/inventories";

/// A coleção de campeões da conta, como a aba Coleção mostra.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    /// Todos os campeões do jogo, os que a conta tem e os que não tem, em
    /// ordem alfabética do nome.
    pub champions: Vec<CollectionChampion>,
    /// Quantos campeões a conta tem (sem contar a rotação gratuita nem os
    /// alugados).
    pub owned_champions: u32,
    /// Quantos campeões o jogo tem.
    pub total_champions: u32,
    /// Quantas skins a conta tem, sem as skins padrão.
    pub owned_skins: u32,
    /// Quantas skins o jogo tem, sem as skins padrão.
    pub total_skins: u32,
}

/// Um campeão na grade da Coleção.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CollectionChampion {
    /// O mesmo `championId` da seleção e do ícone em
    /// `/assets/champion/{id}`.
    pub id: u32,
    /// Nome no idioma do client.
    pub name: String,
    /// Nome interno em inglês (`MonkeyKing` para o Wukong), para a busca.
    pub alias: String,
    /// Título do campeão ("a Espada Darkin"). Vazio quando a LCU não manda.
    pub title: String,
    /// Funções do campeão (`fighter`, `tank`, `mage`, `assassin`,
    /// `marksman`, `support`), como a LCU manda.
    pub roles: Vec<String>,
    /// Rotas em que o campeão costuma jogar, as mesmas da grade da seleção
    /// (#250). Vazia quando a LCU não mandou.
    pub positions: Vec<Position>,
    /// A conta tem o campeão.
    pub owned: bool,
    /// Está na rotação gratuita desta semana.
    pub free_to_play: bool,
    /// Está alugado (conta nova, recompensa temporária).
    pub rented: bool,
    /// Quando o campeão foi liberado na conta, em milissegundos desde 1970,
    /// ou `null` quando a conta não tem ou a LCU não mandou.
    #[ts(type = "number | null")]
    pub purchased: Option<u64>,
    /// Quantas skins do campeão a conta tem, sem a padrão.
    pub owned_skins: u32,
    /// Quantas skins o campeão tem, sem a padrão.
    pub total_skins: u32,
}

/// As skins de um campeão, pedidas com `openChampionSkins`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampionSkins {
    pub champion_id: u32,
    /// A skin padrão primeiro e depois as outras na ordem de lançamento
    /// (pelo `id`).
    pub skins: Vec<CollectionSkin>,
}

/// Uma skin. A imagem sai de `/assets/skin/{id}` (retrato) e
/// `/assets/skinSplash/{id}` (splash art).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSkin {
    pub id: u32,
    pub name: String,
    /// A skin padrão do campeão.
    pub is_base: bool,
    /// A conta tem a skin. A padrão vem com o campeão.
    pub owned: bool,
    /// Raridade, pelo nome do ícone de gema da LCU (`epic`, `legendary`,
    /// `mythic`, `ultimate`...), ou `null` nas skins sem gema.
    pub rarity: Option<String>,
    /// Cromas da skin, na ordem do `id`. Vazia quando ela não tem cromas.
    pub chromas: Vec<SkinChroma>,
}

/// Um croma de skin. A imagem sai de `/assets/skin/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkinChroma {
    pub id: u32,
    pub name: String,
    pub owned: bool,
    /// Cores do croma em `#rrggbb`, para a bolinha de cor. Pode vir vazia.
    pub colors: Vec<String>,
}

/// Caminho dos campeões do inventário de `summoner_id`.
pub fn champions_path(summoner_id: u64) -> String {
    format!("{INVENTORIES_PREFIX}/{summoner_id}/champions-minimal")
}

/// Caminho de todas as skins do inventário de `summoner_id`.
pub fn skins_path(summoner_id: u64) -> String {
    format!("{INVENTORIES_PREFIX}/{summoner_id}/skins-minimal")
}

/// Caminho das skins do campeão `champion_id` no inventário de
/// `summoner_id`.
pub fn champion_skins_path(summoner_id: u64, champion_id: u32) -> String {
    format!("{INVENTORIES_PREFIX}/{summoner_id}/champions/{champion_id}/skins")
}

/// Monta a coleção a partir de `champions-minimal` e `skins-minimal`. As
/// rotas de cada campeão saem do `catalog`, quando houver. `None` se
/// `champions` não for uma lista.
pub fn collection_from_json(
    champions: &Value,
    skins: &Value,
    catalog: Option<&ChampionCatalog>,
) -> Option<Collection> {
    let raw_champions = champions.as_array()?;

    // (tem, total) de cada campeão, sem a skin padrão.
    let mut skin_counts: HashMap<u32, (u32, u32)> = HashMap::new();
    for skin in skins.as_array().into_iter().flatten() {
        if skin["isBase"] == true || skin["isBase"].is_null() && is_base_id(skin) {
            continue;
        }
        let champion_id = as_u32(&skin["championId"]);
        if champion_id == 0 || as_u32(&skin["id"]) == 0 {
            continue;
        }
        let entry = skin_counts.entry(champion_id).or_default();
        entry.1 += 1;
        if is_owned(skin) {
            entry.0 += 1;
        }
    }

    let mut list: Vec<CollectionChampion> = raw_champions
        .iter()
        .filter_map(|raw| {
            let id = as_u32(&raw["id"]);
            let name = raw["name"].as_str().unwrap_or_default();
            // `-1` é o "Nenhum" da LCU; `0` não é campeão.
            if id == 0 || name.is_empty() {
                return None;
            }
            let owned = is_owned(raw);
            let purchased = raw["purchased"]
                .as_u64()
                .or_else(|| raw["ownership"]["rental"]["purchaseDate"].as_u64())
                .filter(|&ms| ms > 0 && owned);
            let (owned_skins, total_skins) = skin_counts.get(&id).copied().unwrap_or_default();
            Some(CollectionChampion {
                id,
                name: name.to_owned(),
                alias: raw["alias"].as_str().unwrap_or_default().to_owned(),
                title: raw["title"].as_str().unwrap_or_default().to_owned(),
                roles: raw["roles"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                positions: catalog
                    .and_then(|c| c.get(id))
                    .map(|c| c.positions.clone())
                    .unwrap_or_default(),
                owned,
                free_to_play: raw["freeToPlay"] == true,
                rented: raw["ownership"]["rental"]["rented"] == true,
                purchased,
                owned_skins,
                total_skins,
            })
        })
        .collect();
    list.sort_by_cached_key(|c| (c.name.to_lowercase(), c.id));
    list.dedup_by_key(|c| c.id);

    let owned_champions = count(list.iter().filter(|c| c.owned));
    let total_champions = count(list.iter());
    let owned_skins = skin_counts.values().map(|(owned, _)| owned).sum();
    let total_skins = skin_counts.values().map(|(_, total)| total).sum();
    Some(Collection {
        champions: list,
        owned_champions,
        total_champions,
        owned_skins,
        total_skins,
    })
}

/// Monta as skins do campeão `champion_id` a partir da resposta de
/// `.../champions/{championId}/skins`. Skins de outro campeão ficam de fora.
/// `None` se a resposta não for uma lista.
pub fn champion_skins_from_json(champion_id: u32, value: &Value) -> Option<ChampionSkins> {
    let mut skins: Vec<CollectionSkin> = value
        .as_array()?
        .iter()
        .filter_map(|raw| {
            let id = as_u32(&raw["id"]);
            let of_champion =
                raw["championId"].is_null() || as_u32(&raw["championId"]) == champion_id;
            if id == 0 || !of_champion {
                return None;
            }
            let is_base = raw["isBase"] == true || raw["isBase"].is_null() && is_base_id(raw);
            let mut chromas: Vec<SkinChroma> = raw["chromas"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|chroma| {
                    let id = as_u32(&chroma["id"]);
                    (id != 0).then(|| SkinChroma {
                        id,
                        name: chroma["name"].as_str().unwrap_or_default().to_owned(),
                        owned: is_owned(chroma),
                        colors: chroma["colors"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .filter(|c| is_hex_color(c))
                            .map(str::to_owned)
                            .collect(),
                    })
                })
                .collect();
            chromas.sort_by_key(|c| c.id);
            chromas.dedup_by_key(|c| c.id);
            Some(CollectionSkin {
                id,
                name: raw["name"].as_str().unwrap_or_default().to_owned(),
                is_base,
                // A padrão vem com o campeão, mesmo quando a LCU não marca.
                owned: is_owned(raw) || is_base && raw["ownership"].is_null(),
                rarity: rarity(raw),
                chromas,
            })
        })
        .collect();
    skins.sort_by_key(|s| (!s.is_base, s.id));
    skins.dedup_by_key(|s| s.id);
    Some(ChampionSkins { champion_id, skins })
}

/// Lê a coleção da conta `summoner_id`.
pub async fn load(
    lcu: &LcuClient,
    summoner_id: u64,
    catalog: Option<&ChampionCatalog>,
) -> Result<Option<Collection>, LcuError> {
    let champions_path = champions_path(summoner_id);
    let skins_path = skins_path(summoner_id);
    let (champions, skins) = tokio::join!(
        lcu.get::<Value>(&champions_path),
        lcu.get::<Value>(&skins_path),
    );
    Ok(collection_from_json(&champions?, &skins?, catalog))
}

/// Lê as skins do campeão `champion_id` na conta `summoner_id`.
pub async fn champion_skins(
    lcu: &LcuClient,
    summoner_id: u64,
    champion_id: u32,
) -> Result<Option<ChampionSkins>, LcuError> {
    let body = lcu
        .get::<Value>(&champion_skins_path(summoner_id, champion_id))
        .await?;
    Ok(champion_skins_from_json(champion_id, &body))
}

/// `epic` de `/lol-game-data/assets/v1/rarity-gem-icons/epic.png`. Nada
/// quando a skin não tem gema.
fn rarity(raw: &Value) -> Option<String> {
    let path = raw["rarityGemPath"].as_str()?;
    let file = path.rsplit('/').next()?;
    let stem = file.split('.').next()?.to_ascii_lowercase();
    let valid = !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    valid.then_some(stem)
}

/// A skin padrão tem o `id` do campeão vezes 1000 (`266000` para o Aatrox).
fn is_base_id(skin: &Value) -> bool {
    as_u32(&skin["id"]).is_multiple_of(1000)
}

fn is_owned(value: &Value) -> bool {
    value["ownership"]["owned"] == true
}

fn is_hex_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn count<T>(items: impl Iterator<Item = T>) -> u32 {
    u32::try_from(items.count()).unwrap_or(u32::MAX)
}

fn as_u32(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::champions::Champion;
    use serde_json::json;

    fn champions() -> Value {
        json!([
            {"id": -1, "name": "Nenhum", "ownership": {"owned": false}},
            {"id": 266, "name": "Aatrox", "alias": "Aatrox", "title": "a Espada Darkin",
             "roles": ["fighter", "tank"], "freeToPlay": false, "purchased": 1567890000000u64,
             "ownership": {"owned": true, "rental": {"rented": false, "purchaseDate": 1}}},
            {"id": 103, "name": "Ahri", "alias": "Ahri", "roles": ["mage"], "freeToPlay": true,
             "purchased": 0, "ownership": {"owned": false, "rental": {"rented": false}}},
            {"id": 62, "name": "Wukong", "alias": "MonkeyKing", "roles": ["fighter"],
             "ownership": {"owned": false, "rental": {"rented": true}}},
            {"id": 1, "name": "", "ownership": {"owned": true}}
        ])
    }

    fn skins() -> Value {
        json!([
            {"id": 266000, "championId": 266, "isBase": true, "ownership": {"owned": true}},
            {"id": 266001, "championId": 266, "isBase": false, "ownership": {"owned": true}},
            {"id": 266002, "championId": 266, "isBase": false, "ownership": {"owned": false}},
            {"id": 103000, "championId": 103, "isBase": true, "ownership": {"owned": false}},
            {"id": 103001, "championId": 103, "isBase": false, "ownership": {"owned": false}},
            {"id": 62000, "championId": 62, "ownership": {"owned": false}}
        ])
    }

    #[test]
    fn caminhos_do_inventario() {
        assert_eq!(
            champions_path(1002),
            "/lol-champions/v1/inventories/1002/champions-minimal"
        );
        assert_eq!(
            skins_path(1002),
            "/lol-champions/v1/inventories/1002/skins-minimal"
        );
        assert_eq!(
            champion_skins_path(1002, 266),
            "/lol-champions/v1/inventories/1002/champions/266/skins"
        );
    }

    #[test]
    fn colecao_com_contagens() {
        let catalog = ChampionCatalog {
            champions: vec![Champion {
                id: 266,
                name: "Aatrox".into(),
                alias: "Aatrox".into(),
                roles: Vec::new(),
                positions: vec![Position::Top],
            }],
        };
        let collection = collection_from_json(&champions(), &skins(), Some(&catalog)).unwrap();
        let names: Vec<&str> = collection
            .champions
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, ["Aatrox", "Ahri", "Wukong"]);
        assert_eq!(collection.owned_champions, 1);
        assert_eq!(collection.total_champions, 3);
        assert_eq!(collection.owned_skins, 1);
        assert_eq!(collection.total_skins, 3);

        let aatrox = &collection.champions[0];
        assert!(aatrox.owned && !aatrox.free_to_play && !aatrox.rented);
        assert_eq!(aatrox.title, "a Espada Darkin");
        assert_eq!(aatrox.roles, ["fighter", "tank"]);
        assert_eq!(aatrox.positions, [Position::Top]);
        assert_eq!(aatrox.purchased, Some(1_567_890_000_000));
        assert_eq!((aatrox.owned_skins, aatrox.total_skins), (1, 2));

        let ahri = &collection.champions[1];
        assert!(!ahri.owned && ahri.free_to_play);
        assert_eq!(ahri.purchased, None);
        assert!(ahri.positions.is_empty());
        assert_eq!((ahri.owned_skins, ahri.total_skins), (0, 1));

        let wukong = &collection.champions[2];
        assert!(wukong.rented && !wukong.owned);
        assert_eq!(wukong.alias, "MonkeyKing");
        assert_eq!((wukong.owned_skins, wukong.total_skins), (0, 0));
    }

    #[test]
    fn colecao_sem_lista_ou_sem_skins() {
        assert!(collection_from_json(&json!({}), &skins(), None).is_none());
        let collection = collection_from_json(&champions(), &json!(null), None).unwrap();
        assert_eq!(collection.total_champions, 3);
        assert_eq!(collection.total_skins, 0);
    }

    #[test]
    fn skins_de_um_campeao() {
        let body = json!([
            {"id": 266002, "championId": 266, "name": "Aatrox Lua de Sangue", "isBase": false,
             "ownership": {"owned": false},
             "rarityGemPath": "/lol-game-data/assets/v1/rarity-gem-icons/Legendary.png",
             "chromas": [
                {"id": 266010, "name": "Rubi", "ownership": {"owned": true},
                 "colors": ["#C82B2B", "#C82B2B", "vermelho"]},
                {"id": 266009, "name": "Safira", "ownership": {"owned": false}}
             ]},
            {"id": 266000, "championId": 266, "name": "Aatrox", "isBase": true,
             "ownership": {"owned": true}, "rarityGemPath": "", "chromas": []},
            {"id": 103001, "championId": 103, "name": "De outro campeão"},
            {"id": 266001, "championId": 266, "name": "Aatrox Justiceiro",
             "ownership": {"owned": true}}
        ]);
        let skins = champion_skins_from_json(266, &body).unwrap();
        assert_eq!(skins.champion_id, 266);
        let ids: Vec<u32> = skins.skins.iter().map(|s| s.id).collect();
        assert_eq!(ids, [266000, 266001, 266002]);

        let base = &skins.skins[0];
        assert!(base.is_base && base.owned);
        assert_eq!(base.rarity, None);

        let lua = &skins.skins[2];
        assert!(!lua.is_base && !lua.owned);
        assert_eq!(lua.rarity.as_deref(), Some("legendary"));
        let chromas: Vec<(u32, bool)> = lua.chromas.iter().map(|c| (c.id, c.owned)).collect();
        assert_eq!(chromas, [(266009, false), (266010, true)]);
        assert_eq!(lua.chromas[1].colors, ["#C82B2B", "#C82B2B"]);

        assert!(champion_skins_from_json(266, &json!({})).is_none());
    }

    #[test]
    fn skin_padrao_sem_ownership_conta_como_da_conta() {
        let body = json!([{"id": 266000, "championId": 266, "name": "Aatrox"}]);
        let skins = champion_skins_from_json(266, &body).unwrap();
        assert!(skins.skins[0].is_base && skins.skins[0].owned);
    }

    #[test]
    fn raridade_pelo_icone() {
        let skin = |path: &str| json!({"rarityGemPath": path});
        assert_eq!(
            rarity(&skin(
                "/lol-game-data/assets/v1/rarity-gem-icons/mythic.png"
            ))
            .as_deref(),
            Some("mythic")
        );
        assert_eq!(rarity(&skin("")), None);
        assert_eq!(rarity(&json!({})), None);
        assert_eq!(rarity(&skin("/x/a b.png")), None);
    }
}
