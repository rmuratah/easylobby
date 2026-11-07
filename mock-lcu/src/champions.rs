//! Campeões do jogo na LCU falsa (#28).
//!
//! `GET /lol-game-data/assets/v1/champion-summary.json` sai da gravação ou,
//! enquanto ela não tiver, do arquivo **sintético** em `mock-lcu/data/`. As
//! listas de campeões que dá para escolher e banir na seleção já saem da
//! gravação, como os outros `GET`.

use crate::state::{LcuState, Response};

/// Os campeões do jogo.
pub const CHAMPION_SUMMARY: &str = "/lol-game-data/assets/v1/champion-summary.json";

const SYNTHETIC_CHAMPION_SUMMARY: &str = include_str!("../data/champion-summary.json");

/// Campeões do jogador (#218).
pub const OWNED_CHAMPIONS: &str = "/lol-champions/v1/owned-champions-minimal";

/// `GET` de [`OWNED_CHAMPIONS`]: o gravado ou, sem gravação, todos os
/// campeões de `champion-summary.json` como comprados.
pub fn owned(state: &LcuState) -> Response {
    let recorded = state.get(OWNED_CHAMPIONS);
    if recorded.status == 200 {
        return recorded;
    }
    let summary = game_data(state).body;
    let owned: Vec<serde_json::Value> = summary
        .as_array()
        .map(|champions| {
            champions
                .iter()
                .filter(|c| c["id"].as_i64().is_some_and(|id| id > 0))
                .map(|c| {
                    serde_json::json!({
                        "id": c["id"], "name": c["name"], "alias": c["alias"],
                        "freeToPlay": false,
                        "ownership": {"owned": true, "rental": {"rented": false}},
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Response {
        status: 200,
        body: serde_json::Value::Array(owned),
    }
}

/// `GET` de `champion-summary.json`: o gravado, ou o sintético.
pub fn game_data(state: &LcuState) -> Response {
    let recorded = state.get(CHAMPION_SUMMARY);
    if recorded.status == 200 {
        return recorded;
    }
    Response {
        status: 200,
        body: serde_json::from_str(SYNTHETIC_CHAMPION_SUMMARY)
            .expect("dados sintéticos são JSON válido"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use easylobby_agent::champions::ChampionCatalog;

    #[test]
    fn dados_sinteticos_viram_catalogo() {
        let body: serde_json::Value = serde_json::from_str(SYNTHETIC_CHAMPION_SUMMARY).unwrap();
        let catalog = ChampionCatalog::from_json(&body).unwrap();
        assert!(catalog.champions.len() > 150);
        assert_eq!(catalog.get(266).unwrap().name, "Aatrox");
        assert!(catalog.get(u32::MAX).is_none());
    }

    #[test]
    fn todos_os_campeoes_sinteticos_sao_do_jogador() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
        let state = LcuState::new(&easylobby_agent::fixtures::FixtureSet::load(&dir).unwrap());
        let body = owned(&state).body;
        let ids = easylobby_agent::quick_play::playable_from_json(&body).unwrap();
        assert!(ids.len() > 150);
        assert!(ids.contains(&266));
    }
}
