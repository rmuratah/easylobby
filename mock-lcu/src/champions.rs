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
}
