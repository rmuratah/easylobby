//! Coleção de campeões e skins na LCU falsa (#369).
//!
//! `GET /lol-champions/v1/inventories/{summonerId}/champions-minimal`,
//! `.../skins-minimal` e `.../champions/{championId}/skins` saem da gravação
//! ou, enquanto ela não tiver, de dados **sintéticos** montados a partir de
//! `champion-summary.json`:
//!
//! - a conta tem todos os campeões, menos os de `id` múltiplo de 4; dos que
//!   ela não tem, os de `id` múltiplo de 3 estão na rotação gratuita;
//! - cada campeão tem a skin padrão (`id * 1000`) e mais 3 skins
//!   (`id * 1000 + 1` a `+ 3`). A conta tem a skin 1 dos campeões dela de
//!   `id` ímpar. A skin 2 tem 3 cromas (`id * 1000 + 20` a `+ 22`), e a
//!   conta tem o primeiro quando tem a skin 2 (só nos campeões de `id`
//!   múltiplo de 5).
//!
//! Os nomes das skins sintéticas são de mentira, e os `id`s nem sempre
//! existem no jogo (a imagem pode faltar). O `summonerId` do caminho não é
//! conferido.

use serde_json::{json, Value};

use crate::champions;
use crate::state::{LcuState, Response};

/// Começo das rotas do inventário.
pub const INVENTORIES_PREFIX: &str = "/lol-champions/v1/inventories/";

/// Qual rota do inventário foi pedida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Champions,
    Skins,
    ChampionSkins(u32),
}

/// Lê `/lol-champions/v1/inventories/{summonerId}/...`. `None` fora das
/// três rotas da coleção.
pub fn parse_route(path: &str) -> Option<Route> {
    let rest = path.strip_prefix(INVENTORIES_PREFIX)?;
    let (summoner_id, rest) = rest.split_once('/')?;
    summoner_id.parse::<u64>().ok()?;
    match rest.split('/').collect::<Vec<_>>().as_slice() {
        ["champions-minimal"] => Some(Route::Champions),
        ["skins-minimal"] => Some(Route::Skins),
        ["champions", id, "skins"] => id.parse().ok().map(Route::ChampionSkins),
        _ => None,
    }
}

/// `GET` de uma rota da coleção: o gravado ou o sintético.
pub fn get(state: &LcuState, path: &str, route: Route) -> Response {
    let recorded = state.get(path);
    if recorded.status == 200 {
        return recorded;
    }
    let summary = champions::game_data(state).body;
    let list = summary.as_array().cloned().unwrap_or_default();
    let champions = list.iter().filter(|c| champion_id(c) > 0);
    let body = match route {
        Route::Champions => Value::Array(champions.map(synthetic_champion).collect()),
        Route::Skins => Value::Array(champions.flat_map(|c| synthetic_skins(c, false)).collect()),
        Route::ChampionSkins(id) => {
            let Some(champion) = list.iter().find(|c| champion_id(c) == id) else {
                return Response::error(404, "mock-lcu: campeão não encontrado");
            };
            Value::Array(synthetic_skins(champion, true))
        }
    };
    Response { status: 200, body }
}

fn champion_id(champion: &Value) -> u32 {
    champion["id"]
        .as_i64()
        .and_then(|id| u32::try_from(id).ok())
        .unwrap_or(0)
}

fn owns_champion(id: u32) -> bool {
    !id.is_multiple_of(4)
}

fn synthetic_champion(champion: &Value) -> Value {
    let id = champion_id(champion);
    let owned = owns_champion(id);
    let purchased = if owned {
        1_600_000_000_000u64 + u64::from(id) * 86_400_000
    } else {
        0
    };
    json!({
        "id": id,
        "name": champion["name"],
        "alias": champion["alias"],
        "title": "",
        "roles": champion["roles"],
        "active": true,
        "freeToPlay": !owned && id.is_multiple_of(3),
        "purchased": purchased,
        "ownership": {
            "owned": owned,
            "loyaltyReward": false,
            "xboxGPReward": false,
            "rental": {"rented": false, "purchaseDate": purchased, "endDate": 0, "winCountRemaining": 0},
        },
    })
}

/// A skin padrão e as 3 sintéticas de `champion`. Com `chromas`, como a
/// rota de um campeão manda; sem, como `skins-minimal`.
fn synthetic_skins(champion: &Value, chromas: bool) -> Vec<Value> {
    const NAMES: [&str; 3] = ["Fliperama", "Projeto", "Guardião Estelar"];
    let id = champion_id(champion);
    let name = champion["name"].as_str().unwrap_or_default();
    let owned_champion = owns_champion(id);
    let base = id * 1000;
    let mut skins = vec![skin(id, base, name, true, owned_champion, None)];
    for (n, label) in (1..).zip(NAMES) {
        let owned = match n {
            1 => owned_champion && id % 2 == 1,
            2 => owned_champion && id.is_multiple_of(5),
            _ => false,
        };
        let rarity = ["epic", "legendary", "mythic"][usize::try_from(n - 1).unwrap_or(0)];
        let mut value = skin(
            id,
            base + n,
            &format!("{name} {label}"),
            false,
            owned,
            Some(rarity),
        );
        if chromas && n == 2 {
            value["chromas"] = json!([
                chroma(id, base + 20, "Rubi", "#c82b2b", owned),
                chroma(id, base + 21, "Safira", "#2b5dc8", false),
                chroma(id, base + 22, "Esmeralda", "#2bc86a", false),
            ]);
        }
        skins.push(value);
    }
    skins
}

fn skin(
    champion_id: u32,
    id: u32,
    name: &str,
    is_base: bool,
    owned: bool,
    rarity: Option<&str>,
) -> Value {
    json!({
        "id": id,
        "championId": champion_id,
        "name": name,
        "isBase": is_base,
        "disabled": false,
        "stillObtainable": true,
        "ownership": {"owned": owned, "rental": {"rented": false}},
        "rarityGemPath": rarity
            .map(|r| format!("/lol-game-data/assets/v1/rarity-gem-icons/{r}.png"))
            .unwrap_or_default(),
        "chromas": [],
    })
}

fn chroma(champion_id: u32, id: u32, name: &str, color: &str, owned: bool) -> Value {
    json!({
        "id": id,
        "championId": champion_id,
        "name": name,
        "colors": [color, color],
        "ownership": {"owned": owned, "rental": {"rented": false}},
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use easylobby_agent::collection;

    fn state() -> LcuState {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
        LcuState::new(&easylobby_agent::fixtures::FixtureSet::load(&dir).unwrap())
    }

    #[test]
    fn rotas_da_colecao() {
        let base = "/lol-champions/v1/inventories/1002";
        assert_eq!(
            parse_route(&format!("{base}/champions-minimal")),
            Some(Route::Champions)
        );
        assert_eq!(
            parse_route(&format!("{base}/skins-minimal")),
            Some(Route::Skins)
        );
        assert_eq!(
            parse_route(&format!("{base}/champions/266/skins")),
            Some(Route::ChampionSkins(266))
        );
        assert_eq!(parse_route(&format!("{base}/champion-mastery")), None);
        assert_eq!(parse_route(&format!("{base}/champions/x/skins")), None);
        assert_eq!(
            parse_route("/lol-champions/v1/inventories/abc/skins-minimal"),
            None
        );
    }

    #[test]
    fn colecao_sintetica_vira_colecao_do_agente() {
        let state = state();
        let champions = get(&state, "/c", Route::Champions).body;
        let skins = get(&state, "/s", Route::Skins).body;
        let collection = collection::collection_from_json(&champions, &skins, None).unwrap();
        assert!(collection.total_champions > 150);
        assert!(collection.owned_champions < collection.total_champions);
        assert!(collection.owned_champions > 0);
        assert_eq!(collection.total_skins, collection.total_champions * 3);
        assert!(collection.owned_skins > 0);
        assert!(collection.champions.iter().any(|c| c.free_to_play));
    }

    #[test]
    fn skins_sinteticas_de_um_campeao() {
        let state = state();
        let body = get(&state, "/x", Route::ChampionSkins(266)).body;
        let skins = collection::champion_skins_from_json(266, &body).unwrap();
        assert_eq!(skins.skins.len(), 4);
        assert!(skins.skins[0].is_base);
        assert_eq!(skins.skins[2].chromas.len(), 3);
        assert_eq!(skins.skins[2].rarity.as_deref(), Some("legendary"));
        assert_eq!(get(&state, "/x", Route::ChampionSkins(999_999)).status, 404);
    }
}
