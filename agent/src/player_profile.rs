//! Perfil de outro jogador, pelo `puuid` (#255): o que a tela de Perfil
//! mostra de um amigo.
//!
//! Tem o mesmo conteúdo do perfil do jogador logado (#171 a #175), lido por
//! rotas que recebem o `puuid`:
//!
//! - invocador: `GET /lol-summoner/v2/summoners/puuid/{puuid}` (Riot ID,
//!   nível, ícone e `privacy`);
//! - ranks: `GET /lol-ranked/v1/ranked-stats/{puuid}`, no mesmo formato de
//!   `current-ranked-stats`;
//! - maestria: `GET /lol-champion-mastery/v1/{puuid}/champion-mastery`, com a
//!   rota antiga `GET /lol-collections/v1/inventories/{summonerId}/champion-mastery`
//!   de reserva;
//! - histórico: `GET /lol-match-history/v1/products/lol/{puuid}/matches`, em
//!   páginas como o do jogador logado.
//!
//! A LCU não tem honra de outro jogador. Um jogador com perfil privado
//! (`privacy: PRIVATE`) pode vir com o histórico vazio.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::mastery::{self, ChampionMastery};
use crate::match_history::{self, MatchHistory, MatchSummary, PAGE_SIZE, PUUID_MATCHES_PREFIX};
use crate::ranked::{self, RankedStats};
use crate::rest::{LcuClient, LcuError};
use crate::summoner::Summoner;

/// Começo do invocador pelo puuid: `<prefixo>/<puuid>`.
pub const SUMMONER_BY_PUUID_PREFIX: &str = "/lol-summoner/v2/summoners/puuid";

/// Começo dos ranks pelo puuid: `<prefixo>/<puuid>`.
pub const RANKED_BY_PUUID_PREFIX: &str = "/lol-ranked/v1/ranked-stats";

/// Começo da maestria pelo puuid: `<prefixo>/<puuid>/champion-mastery`.
pub const MASTERY_BY_PUUID_PREFIX: &str = "/lol-champion-mastery/v1";

/// Começo da maestria pela rota antiga, pelo `summonerId`:
/// `<prefixo>/<summonerId>/champion-mastery`.
pub const MASTERY_BY_SUMMONER_PREFIX: &str = "/lol-collections/v1/inventories";

/// Perfil de um jogador, como a tela de Perfil mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerProfile {
    /// Riot ID, nível, ícone e ids do jogador.
    pub summoner: Summoner,
    /// O jogador escondeu o perfil (`privacy: PRIVATE` na LCU). O histórico
    /// e a maestria podem vir vazios.
    pub private_profile: bool,
    /// Ranks na Solo/Duo e na Flex, ou `null` quando a LCU não respondeu.
    pub ranked: Option<RankedStats>,
    /// Até 3 campeões com mais maestria, do que tem mais pontos para o que
    /// tem menos. Vazia quando a LCU não respondeu.
    pub champion_masteries: Vec<ChampionMastery>,
    /// Histórico já carregado, ou `null` quando a LCU não respondeu.
    pub match_history: Option<MatchHistory>,
}

/// Se `puuid` serve para montar um caminho da LCU: não vazio, só letras,
/// números e `-`.
pub fn valid_puuid(puuid: &str) -> bool {
    !puuid.is_empty()
        && puuid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Invocador do JSON de `/lol-summoner/v2/summoners/puuid/{puuid}` e se o
/// perfil é privado. `None` sem os campos obrigatórios.
pub fn summoner_from_json(value: &Value) -> Option<(Summoner, bool)> {
    let summoner = Summoner::deserialize(value).ok()?;
    let private = value
        .get("privacy")
        .and_then(Value::as_str)
        .is_some_and(|p| p.eq_ignore_ascii_case("PRIVATE"));
    Some((summoner, private))
}

/// Caminho da página do histórico de `puuid` que começa em `begin`.
pub fn page_path(begin: usize, puuid: &str) -> String {
    let end = begin + PAGE_SIZE - 1;
    format!("{PUUID_MATCHES_PREFIX}/{puuid}/matches?begIndex={begin}&endIndex={end}")
}

/// Busca o invocador `puuid`.
pub async fn summoner(lcu: &LcuClient, puuid: &str) -> Result<Option<(Summoner, bool)>, LcuError> {
    let body = lcu
        .get::<Value>(&format!("{SUMMONER_BY_PUUID_PREFIX}/{puuid}"))
        .await?;
    Ok(summoner_from_json(&body))
}

/// Busca os ranks de `puuid`.
pub async fn ranked(lcu: &LcuClient, puuid: &str) -> Result<Option<RankedStats>, LcuError> {
    let body = lcu
        .get::<Value>(&format!("{RANKED_BY_PUUID_PREFIX}/{puuid}"))
        .await?;
    Ok(ranked::from_json(&body))
}

/// Busca os campeões com mais maestria de `puuid`. Se a rota pelo puuid
/// falhar, tenta a antiga pelo `summoner_id` (quando ele não é `0`).
pub async fn masteries(
    lcu: &LcuClient,
    puuid: &str,
    summoner_id: u64,
) -> Result<Option<Vec<ChampionMastery>>, LcuError> {
    let by_puuid = lcu
        .get::<Value>(&format!(
            "{MASTERY_BY_PUUID_PREFIX}/{puuid}/champion-mastery"
        ))
        .await;
    let body = match by_puuid {
        Ok(body) => body,
        Err(_) if summoner_id != 0 => {
            lcu.get::<Value>(&format!(
                "{MASTERY_BY_SUMMONER_PREFIX}/{summoner_id}/champion-mastery"
            ))
            .await?
        }
        Err(error) => return Err(error),
    };
    Ok(mastery::top_from_json(&body))
}

/// Busca a página do histórico de `puuid` que começa em `begin`.
pub async fn page(
    lcu: &LcuClient,
    puuid: &str,
    begin: usize,
) -> Result<Option<(Vec<MatchSummary>, usize)>, LcuError> {
    let body = lcu.get::<Value>(&page_path(begin, puuid)).await?;
    Ok(match_history::page_at(&body, begin, Some(puuid)))
}

/// Lê o perfil inteiro de `puuid`: o invocador primeiro (sem ele não há
/// perfil) e depois ranks, maestria e a primeira página do histórico juntos.
/// Uma parte que falha fica vazia em vez de derrubar o perfil.
///
/// `Ok(None)` quando a resposta do invocador não tem o formato esperado.
pub async fn load(lcu: &LcuClient, puuid: &str) -> Result<Option<PlayerProfile>, LcuError> {
    let Some((summoner, private_profile)) = summoner(lcu, puuid).await? else {
        return Ok(None);
    };
    let (ranked, masteries, page) = tokio::join!(
        ranked(lcu, puuid),
        masteries(lcu, puuid, summoner.summoner_id),
        page(lcu, puuid, 0),
    );
    Ok(Some(PlayerProfile {
        summoner,
        private_profile,
        ranked: ranked.ok().flatten(),
        champion_masteries: masteries.ok().flatten().unwrap_or_default(),
        match_history: page
            .ok()
            .flatten()
            .map(|(matches, received)| MatchHistory::first_page(matches, received)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_history::tests::exemplo;
    use serde_json::json;

    /// O amigo da fixture `exemplo`.
    const AMIGO: &str = "fake-puuid-0002";

    #[test]
    fn puuid_valido() {
        assert!(valid_puuid("fake-puuid-0002"));
        assert!(valid_puuid("0a1B-2c"));
        assert!(!valid_puuid(""));
        assert!(!valid_puuid("../lol-summoner"));
        assert!(!valid_puuid("a?b=1"));
        assert!(!valid_puuid("a/b"));
    }

    #[test]
    fn caminho_da_pagina() {
        assert_eq!(
            page_path(0, AMIGO),
            "/lol-match-history/v1/products/lol/fake-puuid-0002/matches?begIndex=0&endIndex=19"
        );
        assert_eq!(
            page_path(20, AMIGO),
            "/lol-match-history/v1/products/lol/fake-puuid-0002/matches?begIndex=20&endIndex=39"
        );
    }

    #[test]
    fn invocador_e_privacidade() {
        let body = json!({
            "gameName": "Amigo", "tagLine": "BR1", "displayName": "",
            "summonerLevel": 120, "profileIconId": 29, "puuid": AMIGO,
            "summonerId": 1003, "privacy": "PRIVATE", "novo": 1
        });
        let (summoner, private) = summoner_from_json(&body).unwrap();
        assert_eq!(summoner.game_name, "Amigo");
        assert_eq!(summoner.summoner_level, 120);
        assert_eq!(summoner.summoner_id, 1003);
        assert!(private);

        let mut public = body.clone();
        public["privacy"] = json!("PUBLIC");
        assert!(!summoner_from_json(&public).unwrap().1);
        public.as_object_mut().unwrap().remove("privacy");
        assert!(!summoner_from_json(&public).unwrap().1);

        assert!(summoner_from_json(&json!({"gameName": "sem puuid"})).is_none());
    }

    #[test]
    fn perfil_do_amigo_na_fixture_exemplo() {
        let (summoner, private) =
            summoner_from_json(&exemplo(&format!("{SUMMONER_BY_PUUID_PREFIX}/{AMIGO}"))).unwrap();
        assert_eq!(summoner.puuid, AMIGO);
        assert!(!private);

        let ranked =
            ranked::from_json(&exemplo(&format!("{RANKED_BY_PUUID_PREFIX}/{AMIGO}"))).unwrap();
        assert!(ranked.solo_duo.tier.is_some());

        let top = mastery::top_from_json(&exemplo(&format!(
            "{MASTERY_BY_PUUID_PREFIX}/{AMIGO}/champion-mastery"
        )))
        .unwrap();
        assert_eq!(top.len(), mastery::TOP_MASTERIES);

        let (matches, _) =
            match_history::page_at(&exemplo(&page_path(0, AMIGO)), 0, Some(AMIGO)).unwrap();
        assert!(!matches.is_empty());
        assert!(matches.iter().all(|m| m.player.puuid == AMIGO));
    }
}
