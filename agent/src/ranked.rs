//! Ranks do jogador logado na Ranqueada Solo/Duo e na Flex (#171).
//!
//! Lê `GET /lol-ranked/v1/current-ranked-stats` ao conectar e acompanha o
//! mesmo URI pelo WAMP, que muda quando termina uma ranqueada. Só os campos
//! que a tela de Perfil usa vão para a UI.
//!
//! A LCU manda cada fila duas vezes, em `queues` (lista) e em `queueMap`
//! (por `queueType`); o agente aceita as duas formas. Uma conta sem
//! classificação vem com `tier` vazio ou `NONE` e `division` `NA`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) dos ranks do jogador logado.
pub const RANKED_STATS_PATH: &str = "/lol-ranked/v1/current-ranked-stats";

/// `queueType` da Ranqueada Solo/Duo.
const SOLO_DUO: &str = "RANKED_SOLO_5x5";
/// `queueType` da Ranqueada Flex.
const FLEX: &str = "RANKED_FLEX_SR";

/// Ranks do jogador logado nas duas filas ranqueadas de Summoner's Rift.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RankedStats {
    pub solo_duo: RankedEntry,
    pub flex: RankedEntry,
}

/// Rank numa fila.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RankedEntry {
    /// Tier em maiúsculas, como a LCU manda (`IRON`, `BRONZE`, `SILVER`,
    /// `GOLD`, `PLATINUM`, `EMERALD`, `DIAMOND`, `MASTER`, `GRANDMASTER`,
    /// `CHALLENGER`). `null` sem classificação nesta fila.
    pub tier: Option<String>,
    /// Divisão (`I` a `IV`). `null` sem classificação e nos tiers sem
    /// divisão (Mestre para cima).
    pub division: Option<String>,
    /// Pontos de liga (PDL).
    pub league_points: u32,
    pub wins: u32,
    pub losses: u32,
    /// Ainda nas partidas de classificação da temporada.
    pub provisional: bool,
}

/// Uma fila como a LCU manda, só com os campos usados.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawEntry {
    queue_type: String,
    tier: String,
    division: String,
    league_points: u32,
    wins: u32,
    losses: u32,
    is_provisional: bool,
}

impl From<RawEntry> for RankedEntry {
    fn from(raw: RawEntry) -> Self {
        let tier = raw.tier.trim().to_ascii_uppercase();
        let ranked = !tier.is_empty() && tier != "NONE" && tier != "UNRANKED";
        if !ranked {
            return RankedEntry {
                wins: raw.wins,
                losses: raw.losses,
                provisional: raw.is_provisional,
                ..RankedEntry::default()
            };
        }
        let division = raw.division.trim().to_ascii_uppercase();
        let division = match division.as_str() {
            "I" | "II" | "III" | "IV" => Some(division),
            _ => None,
        };
        RankedEntry {
            tier: Some(tier),
            division,
            league_points: raw.league_points,
            wins: raw.wins,
            losses: raw.losses,
            provisional: raw.is_provisional,
        }
    }
}

/// Lê os ranks do JSON da LCU (resposta do `GET` ou `data` de um evento).
/// `None` se não for um objeto. Uma fila que não veio fica sem
/// classificação.
pub fn from_json(value: &Value) -> Option<RankedStats> {
    let object = value.as_object()?;
    let mut entries: Vec<RawEntry> = Vec::new();
    if let Some(list) = object.get("queues").and_then(Value::as_array) {
        entries.extend(list.iter().filter_map(|q| RawEntry::deserialize(q).ok()));
    }
    if let Some(map) = object.get("queueMap").and_then(Value::as_object) {
        for (queue_type, q) in map {
            if let Ok(mut entry) = RawEntry::deserialize(q) {
                if entry.queue_type.is_empty() {
                    entry.queue_type = queue_type.clone();
                }
                entries.push(entry);
            }
        }
    }
    let mut stats = RankedStats::default();
    let mut seen = (false, false);
    for entry in entries {
        // `queues` vem primeiro; a mesma fila repetida no `queueMap` não
        // sobrescreve.
        match entry.queue_type.as_str() {
            SOLO_DUO if !seen.0 => {
                seen.0 = true;
                stats.solo_duo = entry.into();
            }
            FLEX if !seen.1 => {
                seen.1 = true;
                stats.flex = entry.into();
            }
            _ => {}
        }
    }
    Some(stats)
}

/// Busca os ranks do jogador logado.
pub async fn current(lcu: &LcuClient) -> Result<Option<RankedStats>, LcuError> {
    let body = lcu.get::<Value>(RANKED_STATS_PATH).await?;
    Ok(from_json(&body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_solo_duo_e_flex() {
        let body = json!({
            "queues": [
                {"queueType": "RANKED_SOLO_5x5", "tier": "GOLD", "division": "II",
                 "leaguePoints": 47, "wins": 30, "losses": 25, "isProvisional": false,
                 "campoNovo": 1},
                {"queueType": "RANKED_FLEX_SR", "tier": "", "division": "NA",
                 "leaguePoints": 0, "wins": 0, "losses": 0, "isProvisional": true},
                {"queueType": "RANKED_TFT", "tier": "SILVER", "division": "I"}
            ]
        });
        let stats = from_json(&body).unwrap();
        assert_eq!(
            stats.solo_duo,
            RankedEntry {
                tier: Some("GOLD".into()),
                division: Some("II".into()),
                league_points: 47,
                wins: 30,
                losses: 25,
                provisional: false,
            }
        );
        assert_eq!(stats.flex.tier, None);
        assert_eq!(stats.flex.division, None);
        assert!(stats.flex.provisional);
    }

    #[test]
    fn aceita_queue_map() {
        let body = json!({
            "queueMap": {
                "RANKED_FLEX_SR": {"tier": "master", "division": "NA", "leaguePoints": 120,
                                    "wins": 80, "losses": 60}
            }
        });
        let stats = from_json(&body).unwrap();
        assert_eq!(stats.flex.tier.as_deref(), Some("MASTER"));
        assert_eq!(stats.flex.division, None);
        assert_eq!(stats.flex.league_points, 120);
        assert_eq!(stats.solo_duo, RankedEntry::default());
    }

    #[test]
    fn ranks_do_exemplo() {
        let body = crate::match_history::tests::exemplo(RANKED_STATS_PATH);
        let stats = from_json(&body).unwrap();
        assert_eq!(stats.solo_duo.tier.as_deref(), Some("GOLD"));
        assert_eq!(stats.flex.tier.as_deref(), Some("SILVER"));
    }

    #[test]
    fn none_e_sem_classificacao() {
        let body = json!({"queues": [
            {"queueType": "RANKED_SOLO_5x5", "tier": "NONE", "division": "NA", "wins": 3}
        ]});
        let stats = from_json(&body).unwrap();
        assert_eq!(stats.solo_duo.tier, None);
        assert_eq!(stats.solo_duo.wins, 3);
        assert!(from_json(&json!([1, 2])).is_none());
    }
}
