//! Jogadores para convidar além dos amigos (#206), como no pop-up de convite
//! do client oficial:
//!
//! - **Recentes**: quem jogou com o jogador logado nas últimas partidas
//!   (`GET /lol-match-history/v1/recently-played-summoners/{puuid}`);
//! - **Convidar por nome**: acha um jogador pelo Riot ID (`nome#tag`) para
//!   mandar o convite com o `summonerId` e o `puuid` dele
//!   (`GET /lol-summoner/v1/summoners?name=`, e o
//!   `GET /lol-summoner/v1/alias/lookup` quando o primeiro não acha).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::chat::encode;
use crate::invitations::Invitee;
use crate::rest::{LcuClient, LcuError};

/// Quantos jogadores recentes vão para a UI.
pub const MAX_RECENT_PLAYERS: usize = 20;

/// Um jogador das partidas recentes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecentPlayer {
    pub puuid: String,
    /// `summonerId` do LoL, para o convite. `0` quando a LCU não manda.
    #[ts(type = "number")]
    pub summoner_id: u64,
    /// Nome do Riot ID (antes do `#`). Vazio quando a LCU não manda; nesse
    /// caso use `summonerName`.
    pub game_name: String,
    pub tag_line: String,
    /// Nome antigo de invocador, quando a LCU manda.
    pub summoner_name: String,
    /// Campeão que ele jogou na partida, ou `null`.
    pub champion_id: Option<u32>,
    /// Quando foi a partida, como a LCU manda (ISO 8601). Vazio sem
    /// horário.
    pub played_at: String,
}

/// A lista como a LCU manda, da partida mais nova para a mais velha. Pula o
/// jogador logado (`my_puuid`), entradas sem `puuid` e repetidos (fica a
/// partida mais nova), e corta em [`MAX_RECENT_PLAYERS`].
pub fn recent_from_json(value: &Value, my_puuid: &str) -> Vec<RecentPlayer> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let mut players: Vec<RecentPlayer> = items
        .iter()
        .filter_map(|item| {
            let raw = RawRecent::deserialize(item).ok()?;
            if raw.puuid.is_empty() || raw.puuid == my_puuid {
                return None;
            }
            Some(RecentPlayer {
                puuid: raw.puuid,
                summoner_id: raw.summoner_id,
                game_name: raw.game_name,
                tag_line: raw.tag_line,
                summoner_name: raw.summoner_name,
                champion_id: raw.champion_id.filter(|id| *id > 0),
                played_at: match raw.game_creation_date {
                    Value::String(date) => date,
                    Value::Number(n) => n.to_string(),
                    _ => String::new(),
                },
            })
        })
        .collect();
    // Mais nova primeiro: o horário da LCU tem sempre o mesmo formato.
    players.sort_by(|a, b| b.played_at.cmp(&a.played_at));
    let mut seen = std::collections::HashSet::new();
    players.retain(|p| seen.insert(p.puuid.clone()));
    players.truncate(MAX_RECENT_PLAYERS);
    players
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRecent {
    #[serde(default)]
    puuid: String,
    #[serde(default)]
    summoner_id: u64,
    #[serde(default)]
    game_name: String,
    #[serde(default)]
    tag_line: String,
    #[serde(default)]
    summoner_name: String,
    #[serde(default)]
    champion_id: Option<u32>,
    /// ISO 8601; algumas versões mandam milissegundos.
    #[serde(default)]
    game_creation_date: Value,
}

/// Os jogadores das partidas recentes de `my_puuid`.
pub async fn recent(lcu: &LcuClient, my_puuid: &str) -> Result<Vec<RecentPlayer>, LcuError> {
    let path = format!(
        "/lol-match-history/v1/recently-played-summoners/{}",
        encode(my_puuid)
    );
    let body: Value = lcu.get(&path).await?;
    Ok(recent_from_json(&body, my_puuid))
}

/// Separa um Riot ID digitado: tira espaços das pontas e o `#` da tag.
/// `None` sem nome ou sem tag.
pub fn parse_riot_id(game_name: &str, tag_line: &str) -> Option<(String, String)> {
    let name = game_name.trim();
    let tag = tag_line.trim().trim_start_matches('#').trim();
    (!name.is_empty() && !tag.is_empty()).then(|| (name.to_owned(), tag.to_owned()))
}

/// O convidado de um invocador ou de um alias da LCU: `summonerId` e
/// `puuid`, o que vier. `None` sem nenhum dos dois.
pub fn invitee_from_json(value: &Value) -> Option<Invitee> {
    let summoner_id = value.get("summonerId").and_then(Value::as_u64).unwrap_or(0);
    let puuid = value
        .get("puuid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    (summoner_id != 0 || !puuid.is_empty()).then_some(Invitee { summoner_id, puuid })
}

/// Acha o jogador `nome#tag`. `Ok(None)` quando a LCU não acha ninguém.
pub async fn lookup(
    lcu: &LcuClient,
    game_name: &str,
    tag_line: &str,
) -> Result<Option<Invitee>, LcuError> {
    let riot_id = format!("{game_name}#{tag_line}");
    let by_name = lcu
        .get::<Value>(&format!(
            "/lol-summoner/v1/summoners?name={}",
            encode(&riot_id)
        ))
        .await;
    if let Some(found) = by_name.as_ref().ok().and_then(invitee_from_json) {
        if found.summoner_id != 0 {
            return Ok(Some(found));
        }
    }
    let alias = lcu
        .get::<Value>(&format!(
            "/lol-summoner/v1/alias/lookup?gameName={}&tagLine={}",
            encode(game_name),
            encode(tag_line)
        ))
        .await;
    match alias {
        Ok(value) => {
            // O alias às vezes vem numa lista.
            let value = value
                .as_array()
                .and_then(|a| a.first().cloned())
                .unwrap_or(value);
            Ok(invitee_from_json(&value)
                .or_else(|| by_name.ok().and_then(|v| invitee_from_json(&v))))
        }
        Err(LcuError::Status { status: 404, .. }) => Ok(None),
        Err(error) => match by_name {
            Err(LcuError::Status { status: 404, .. }) => Ok(None),
            _ => Err(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn recente(puuid: &str, quando: &str) -> Value {
        json!({
            "championId": 64, "gameCreationDate": quando, "gameId": 1,
            "gameName": format!("Nome {puuid}"), "tagLine": "BR1", "puuid": puuid,
            "summonerId": 10, "summonerName": "",
        })
    }

    #[test]
    fn recentes_sem_mim_sem_repetir_e_mais_novos_primeiro() {
        let lista = json!([
            recente("a", "2026-11-06T20:00:00.000Z"),
            recente("eu", "2026-11-07T20:00:00.000Z"),
            recente("b", "2026-11-07T19:00:00.000Z"),
            recente("a", "2026-11-07T18:00:00.000Z"),
            {"gameName": "sem puuid"},
        ]);
        let jogadores = recent_from_json(&lista, "eu");
        let ids: Vec<_> = jogadores.iter().map(|p| p.puuid.as_str()).collect();
        assert_eq!(ids, ["b", "a"]);
        assert_eq!(jogadores[1].played_at, "2026-11-07T18:00:00.000Z");
        assert_eq!(jogadores[0].champion_id, Some(64));
        assert!(recent_from_json(&json!({}), "eu").is_empty());
    }

    #[test]
    fn le_o_riot_id_digitado() {
        assert_eq!(
            parse_riot_id(" árabe ", " #arabi"),
            Some(("árabe".into(), "arabi".into()))
        );
        assert_eq!(parse_riot_id("nome", ""), None);
        assert_eq!(parse_riot_id("  ", "BR1"), None);
    }

    #[test]
    fn convidado_pelo_summoner_id_ou_puuid() {
        let i = invitee_from_json(&json!({"summonerId": 5, "puuid": "p"})).unwrap();
        assert_eq!((i.summoner_id, i.puuid.as_str()), (5, "p"));
        let i = invitee_from_json(&json!({"puuid": "p", "alias": {}})).unwrap();
        assert_eq!(i.summoner_id, 0);
        assert!(invitee_from_json(&json!({})).is_none());
    }
}
