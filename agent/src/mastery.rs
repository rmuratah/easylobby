//! Campeões com mais maestria do jogador logado (#175).
//!
//! Lê `GET /lol-champion-mastery/v1/local-player/champion-mastery` ao
//! conectar e de novo depois de cada partida (a maestria só muda quando uma
//! partida termina). A UI recebe só os [`TOP_MASTERIES`] primeiros, por
//! pontos.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint da maestria de todos os campeões do jogador logado.
pub const CHAMPION_MASTERY_PATH: &str = "/lol-champion-mastery/v1/local-player/champion-mastery";

/// Quantos campeões vão para a UI.
pub const TOP_MASTERIES: usize = 3;

/// Maestria num campeão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampionMastery {
    pub champion_id: u32,
    /// Nível de maestria.
    pub level: u32,
    /// Pontos de maestria.
    #[ts(type = "number")]
    pub points: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMastery {
    champion_id: u32,
    #[serde(default)]
    champion_level: u32,
    #[serde(default)]
    champion_points: u64,
}

/// Os [`TOP_MASTERIES`] campeões com mais pontos, do JSON da LCU. `None` se
/// não for uma lista.
pub fn top_from_json(value: &Value) -> Option<Vec<ChampionMastery>> {
    let mut list: Vec<ChampionMastery> = value
        .as_array()?
        .iter()
        .filter_map(|raw| RawMastery::deserialize(raw).ok())
        .filter(|raw| raw.champion_id > 0)
        .map(|raw| ChampionMastery {
            champion_id: raw.champion_id,
            level: raw.champion_level,
            points: raw.champion_points,
        })
        .collect();
    list.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then(a.champion_id.cmp(&b.champion_id))
    });
    list.dedup_by_key(|m| m.champion_id);
    list.truncate(TOP_MASTERIES);
    Some(list)
}

/// Busca os campeões com mais maestria.
pub async fn top(lcu: &LcuClient) -> Result<Option<Vec<ChampionMastery>>, LcuError> {
    let body = lcu.get::<Value>(CHAMPION_MASTERY_PATH).await?;
    Ok(top_from_json(&body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tres_com_mais_pontos() {
        let body = json!([
            {"championId": 1, "championLevel": 5, "championPoints": 30000},
            {"championId": 2, "championLevel": 10, "championPoints": 150000},
            {"championId": 3, "championLevel": 2, "championPoints": 900},
            {"championId": 4, "championLevel": 7, "championPoints": 60000, "novo": true},
            {"sem": "campeão"}
        ]);
        let top = top_from_json(&body).unwrap();
        let ids: Vec<u32> = top.iter().map(|m| m.champion_id).collect();
        assert_eq!(ids, [2, 4, 1]);
        assert_eq!(top[0].level, 10);
        assert_eq!(top[0].points, 150_000);
        assert!(top_from_json(&json!({})).is_none());
    }

    #[test]
    fn maestria_do_exemplo() {
        let body = crate::match_history::tests::exemplo(CHAMPION_MASTERY_PATH);
        let top = top_from_json(&body).unwrap();
        assert_eq!(top.len(), TOP_MASTERIES);
        assert_eq!(top[0].champion_id, 103);
    }

    #[test]
    fn maestria_da_gravacao_real() {
        let body = crate::match_history::tests::gravacao_perfil(CHAMPION_MASTERY_PATH);
        let top = top_from_json(&body).unwrap();
        let ids: Vec<u32> = top.iter().map(|m| m.champion_id).collect();
        assert_eq!(ids, [121, 64, 104]);
        assert_eq!(top[0].level, 122);
        assert_eq!(top[0].points, 1_312_362);
    }
}
