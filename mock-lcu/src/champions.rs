//! Campeões do jogo na LCU falsa (#28).
//!
//! `GET /lol-game-data/assets/v1/champion-summary.json` sai da gravação ou,
//! enquanto ela não tiver, do arquivo **sintético** em `mock-lcu/data/`. As
//! listas de campeões que dá para escolher e banir na seleção já saem da
//! gravação, como os outros `GET`.
//!
//! `GET /lol-perks/v1/recommended-champion-positions` (#250) também sai da
//! gravação ou, sem ela, do arquivo **sintético** em `mock-lcu/data/`.

use crate::state::{LcuState, Response};

/// Os campeões do jogo.
pub const CHAMPION_SUMMARY: &str = "/lol-game-data/assets/v1/champion-summary.json";

const SYNTHETIC_CHAMPION_SUMMARY: &str = include_str!("../data/champion-summary.json");

const SYNTHETIC_RECOMMENDED_POSITIONS: &str =
    include_str!("../data/recommended-champion-positions.json");

/// Rotas recomendadas de cada campeão (#250).
pub const RECOMMENDED_POSITIONS: &str = "/lol-perks/v1/recommended-champion-positions";

/// Campeões que o jogador pode escolher na seleção (os dele e os da rotação).
pub const PICKABLE: &str = "/lol-champ-select/v1/pickable-champion-ids";

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

/// `GET` de [`RECOMMENDED_POSITIONS`]: o gravado, ou o sintético.
pub fn recommended_positions(state: &LcuState) -> Response {
    let recorded = state.get(RECOMMENDED_POSITIONS);
    if recorded.status == 200 {
        return recorded;
    }
    Response {
        status: 200,
        body: serde_json::from_str(SYNTHETIC_RECOMMENDED_POSITIONS)
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

    #[test]
    fn rotas_sinteticas_cobrem_as_cinco_rotas() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
        let state = LcuState::new(&easylobby_agent::fixtures::FixtureSet::load(&dir).unwrap());
        let catalog = ChampionCatalog::from_json(&game_data(&state).body)
            .unwrap()
            .with_positions(&recommended_positions(&state).body);
        use easylobby_agent::lobby::Position;
        for position in [
            Position::Top,
            Position::Jungle,
            Position::Middle,
            Position::Bottom,
            Position::Utility,
        ] {
            assert!(
                catalog
                    .champions
                    .iter()
                    .any(|c| c.positions.contains(&position)),
                "nenhum campeão em {position:?}"
            );
        }
    }
}
