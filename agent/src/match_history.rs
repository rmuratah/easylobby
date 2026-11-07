//! Histórico de partidas do jogador logado (#172) e o placar de uma partida
//! (#174).
//!
//! A lista sai de
//! `GET /lol-match-history/v1/products/lol/current-summoner/matches?begIndex=..&endIndex=..`,
//! [`PAGE_SIZE`] partidas por vez (os índices são inclusivos e contam da mais
//! nova). Nessa resposta cada partida só traz o jogador logado em
//! `participants`. O placar com os 10 jogadores sai de
//! `GET /lol-match-history/v1/games/{gameId}`.
//!
//! Os dois têm o mesmo formato de partida: `participantIdentities` liga o
//! `participantId` ao jogador (puuid e Riot ID) e `participants` traz o
//! campeão, os feitiços e as estatísticas (`stats`) de cada um.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Partidas por página.
pub const PAGE_SIZE: usize = 20;

/// Caminho da lista do jogador logado, sem a query.
pub const MATCHES_PATH: &str = "/lol-match-history/v1/products/lol/current-summoner/matches";

/// Prefixo do placar de uma partida: `<prefixo>/<gameId>`.
pub const GAME_PATH_PREFIX: &str = "/lol-match-history/v1/games";

/// O histórico já carregado.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MatchHistory {
    /// Da mais nova para a mais antiga, sem repetidas.
    pub matches: Vec<MatchSummary>,
    /// Se a última página veio cheia, ou seja, se vale pedir mais
    /// (`loadMoreMatches`).
    pub has_more: bool,
}

/// Uma partida da lista, do ponto de vista do jogador logado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummary {
    #[ts(type = "number")]
    pub game_id: u64,
    /// `queueId` da Riot (420 Solo/Duo, 440 Flex, 450 ARAM...).
    pub queue_id: u32,
    pub game_mode: String,
    pub map_id: u32,
    /// Início da partida, em milissegundos desde 1970.
    #[ts(type = "number")]
    pub created_at: u64,
    pub duration_secs: u32,
    pub result: MatchResult,
    pub player: MatchPlayer,
}

/// O placar de uma partida, com os 10 jogadores (#174).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetails {
    #[ts(type = "number")]
    pub game_id: u64,
    pub queue_id: u32,
    pub game_mode: String,
    pub map_id: u32,
    #[ts(type = "number")]
    pub created_at: u64,
    pub duration_secs: u32,
    /// Partida encerrada no começo (remake).
    pub remake: bool,
    /// Na ordem de `participantId` (time 100 e depois o 200).
    pub players: Vec<MatchPlayer>,
}

/// Resultado do jogador logado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MatchResult {
    Win,
    Loss,
    /// Partida encerrada no começo; não conta vitória nem derrota.
    Remake,
}

/// Um jogador numa partida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MatchPlayer {
    /// Vazio quando a LCU não mandou (jogador com perfil privado).
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
    pub profile_icon_id: u32,
    /// `100` (azul) ou `200` (vermelho).
    pub team_id: u32,
    pub win: bool,
    pub champion_id: u32,
    pub champion_level: u32,
    pub spell1_id: u32,
    pub spell2_id: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    /// Tropas e monstros abatidos.
    pub cs: u32,
    pub gold: u32,
    pub damage_to_champions: u32,
    pub vision_score: u32,
    /// Os sete espaços (`item0` a `item6`, o último é o amuleto), `0` no
    /// espaço vazio.
    pub items: Vec<u32>,
    /// Runa principal (keystone), `0` se não veio.
    pub perk_id: u32,
    pub perk_style_id: u32,
    pub perk_sub_style_id: u32,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawGame {
    game_id: u64,
    queue_id: u32,
    game_mode: String,
    map_id: u32,
    game_creation: u64,
    game_duration: u32,
    participant_identities: Vec<RawIdentity>,
    participants: Vec<RawParticipant>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawIdentity {
    participant_id: u32,
    player: RawPlayer,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawPlayer {
    puuid: String,
    game_name: String,
    tag_line: String,
    summoner_name: String,
    profile_icon: u32,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawParticipant {
    participant_id: u32,
    team_id: u32,
    champion_id: u32,
    spell1_id: u32,
    spell2_id: u32,
    stats: RawStats,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawStats {
    win: bool,
    game_ended_in_early_surrender: bool,
    champ_level: u32,
    kills: u32,
    deaths: u32,
    assists: u32,
    total_minions_killed: u32,
    neutral_minions_killed: u32,
    gold_earned: u32,
    total_damage_dealt_to_champions: u32,
    vision_score: u32,
    item0: u32,
    item1: u32,
    item2: u32,
    item3: u32,
    item4: u32,
    item5: u32,
    item6: u32,
    perk0: u32,
    perk_primary_style: u32,
    perk_sub_style: u32,
}

impl RawGame {
    fn players(&self) -> Vec<MatchPlayer> {
        let mut participants: Vec<&RawParticipant> = self.participants.iter().collect();
        participants.sort_by_key(|p| p.participant_id);
        participants
            .into_iter()
            .map(|p| {
                let identity = self
                    .participant_identities
                    .iter()
                    .find(|i| i.participant_id == p.participant_id)
                    .map(|i| &i.player);
                player(p, identity)
            })
            .collect()
    }

    fn remake(&self) -> bool {
        self.participants
            .iter()
            .any(|p| p.stats.game_ended_in_early_surrender)
    }
}

fn player(p: &RawParticipant, identity: Option<&RawPlayer>) -> MatchPlayer {
    let s = &p.stats;
    let (puuid, game_name, tag_line, profile_icon_id) = match identity {
        Some(i) => {
            let name = if i.game_name.is_empty() {
                i.summoner_name.clone()
            } else {
                i.game_name.clone()
            };
            (i.puuid.clone(), name, i.tag_line.clone(), i.profile_icon)
        }
        None => Default::default(),
    };
    MatchPlayer {
        puuid,
        game_name,
        tag_line,
        profile_icon_id,
        team_id: p.team_id,
        win: s.win,
        champion_id: p.champion_id,
        champion_level: s.champ_level,
        spell1_id: p.spell1_id,
        spell2_id: p.spell2_id,
        kills: s.kills,
        deaths: s.deaths,
        assists: s.assists,
        cs: s.total_minions_killed + s.neutral_minions_killed,
        gold: s.gold_earned,
        damage_to_champions: s.total_damage_dealt_to_champions,
        vision_score: s.vision_score,
        items: vec![
            s.item0, s.item1, s.item2, s.item3, s.item4, s.item5, s.item6,
        ],
        perk_id: s.perk0,
        perk_style_id: s.perk_primary_style,
        perk_sub_style_id: s.perk_sub_style,
    }
}

/// Uma partida da lista. `puuid` escolhe o jogador logado quando a resposta
/// traz mais de um; sem ele (ou sem achar), vale o primeiro.
fn summary(raw: &RawGame, puuid: Option<&str>) -> Option<MatchSummary> {
    if raw.game_id == 0 {
        return None;
    }
    let players = raw.players();
    let me = puuid
        .and_then(|puuid| players.iter().find(|p| p.puuid == puuid))
        .or_else(|| players.first())?
        .clone();
    let result = if raw.remake() {
        MatchResult::Remake
    } else if me.win {
        MatchResult::Win
    } else {
        MatchResult::Loss
    };
    Some(MatchSummary {
        game_id: raw.game_id,
        queue_id: raw.queue_id,
        game_mode: raw.game_mode.clone(),
        map_id: raw.map_id,
        created_at: raw.game_creation,
        duration_secs: raw.game_duration,
        result,
        player: me,
    })
}

/// Uma página da lista do JSON da LCU: as partidas (da mais nova para a mais
/// antiga) e quantas vieram na resposta, contando as que não deu para ler.
/// `None` se a resposta não tiver a lista.
pub fn page_from_json(value: &Value, puuid: Option<&str>) -> Option<(Vec<MatchSummary>, usize)> {
    let games = value.get("games")?.get("games")?.as_array()?;
    let mut matches: Vec<MatchSummary> = games
        .iter()
        .filter_map(|g| RawGame::deserialize(g).ok())
        .filter_map(|g| summary(&g, puuid))
        .collect();
    sort_newest_first(&mut matches);
    Some((matches, games.len()))
}

/// O placar de uma partida do JSON da LCU. `None` se não for uma partida.
pub fn details_from_json(value: &Value) -> Option<MatchDetails> {
    let raw = RawGame::deserialize(value).ok()?;
    if raw.game_id == 0 {
        return None;
    }
    Some(MatchDetails {
        game_id: raw.game_id,
        queue_id: raw.queue_id,
        game_mode: raw.game_mode.clone(),
        map_id: raw.map_id,
        created_at: raw.game_creation,
        duration_secs: raw.game_duration,
        remake: raw.remake(),
        players: raw.players(),
    })
}

fn sort_newest_first(matches: &mut Vec<MatchSummary>) {
    matches.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then(b.game_id.cmp(&a.game_id))
    });
    let mut seen = std::collections::HashSet::new();
    matches.retain(|m| seen.insert(m.game_id));
}

impl MatchHistory {
    /// Primeira página lida de uma vez.
    pub fn first_page(matches: Vec<MatchSummary>, received: usize) -> Self {
        Self {
            matches,
            has_more: received >= PAGE_SIZE,
        }
    }

    /// Acrescenta a página seguinte (`loadMoreMatches`).
    pub fn append(&mut self, page: Vec<MatchSummary>, received: usize) {
        self.matches.extend(page);
        sort_newest_first(&mut self.matches);
        self.has_more = received >= PAGE_SIZE;
    }

    /// Junta uma primeira página relida (depois de uma partida) com o que já
    /// estava carregado, sem perder as páginas a mais.
    pub fn merge_first_page(&mut self, page: Vec<MatchSummary>, received: usize) {
        let loaded_more = self.matches.len() > page.len();
        let page_has_more = received >= PAGE_SIZE;
        self.matches.extend(page);
        sort_newest_first(&mut self.matches);
        if !loaded_more {
            self.has_more = page_has_more;
        }
    }
}

/// Caminho com a query da página que começa em `begin`.
pub fn page_path(begin: usize) -> String {
    let end = begin + PAGE_SIZE - 1;
    format!("{MATCHES_PATH}?begIndex={begin}&endIndex={end}")
}

/// Busca a página que começa em `begin` (0 para as mais novas).
pub async fn page(
    lcu: &LcuClient,
    begin: usize,
    puuid: Option<&str>,
) -> Result<Option<(Vec<MatchSummary>, usize)>, LcuError> {
    let body = lcu.get::<Value>(&page_path(begin)).await?;
    Ok(page_from_json(&body, puuid))
}

/// Busca o placar da partida `game_id`.
pub async fn details(lcu: &LcuClient, game_id: u64) -> Result<Option<MatchDetails>, LcuError> {
    let body = lcu
        .get::<Value>(&format!("{GAME_PATH_PREFIX}/{game_id}"))
        .await?;
    Ok(details_from_json(&body))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// Uma partida no formato da LCU, com `players` jogadores (o 1 é o
    /// `fake-puuid-me`).
    pub(crate) fn game(game_id: u64, created: u64, win: bool, players: u32) -> Value {
        let identities: Vec<Value> = (1..=players)
            .map(|id| {
                let puuid = if id == 1 {
                    "fake-puuid-me".to_owned()
                } else {
                    format!("fake-puuid-{id}")
                };
                json!({"participantId": id, "player": {
                    "puuid": puuid, "gameName": format!("fake-name-{id}"),
                    "tagLine": "fake-tag", "profileIcon": 29, "summonerName": ""
                }})
            })
            .collect();
        let participants: Vec<Value> = (1..=players)
            .rev()
            .map(|id| {
                let team = if id <= 5 { 100 } else { 200 };
                json!({"participantId": id, "teamId": team, "championId": 266,
                "spell1Id": 4, "spell2Id": 14, "stats": {
                    "win": (team == 100) == win, "champLevel": 15, "kills": 7, "deaths": 2,
                    "assists": 9, "totalMinionsKilled": 150, "neutralMinionsKilled": 12,
                    "goldEarned": 11000, "totalDamageDealtToChampions": 20000,
                    "visionScore": 25, "item0": 3071, "item1": 0, "item6": 3340,
                    "perk0": 8010, "perkPrimaryStyle": 8000, "perkSubStyle": 8400,
                    "gameEndedInEarlySurrender": false, "campoNovo": 1
                }})
            })
            .collect();
        json!({
            "gameId": game_id, "queueId": 420, "gameMode": "CLASSIC", "mapId": 11,
            "gameCreation": created, "gameDuration": 1800,
            "participantIdentities": identities, "participants": participants
        })
    }

    pub(crate) fn page_json(games: Vec<Value>) -> Value {
        json!({"accountId": 1, "games": {"gameCount": games.len(), "games": games}})
    }

    #[test]
    fn le_a_lista_da_mais_nova_para_a_mais_antiga() {
        let body = page_json(vec![
            game(1, 1000, true, 1),
            game(2, 2000, false, 1),
            json!("lixo"),
        ]);
        let (matches, received) = page_from_json(&body, Some("fake-puuid-me")).unwrap();
        assert_eq!(received, 3);
        let ids: Vec<u64> = matches.iter().map(|m| m.game_id).collect();
        assert_eq!(ids, [2, 1]);
        let m = &matches[1];
        assert_eq!(m.result, MatchResult::Win);
        assert_eq!(m.duration_secs, 1800);
        assert_eq!(m.player.cs, 162);
        assert_eq!(m.player.items, [3071, 0, 0, 0, 0, 0, 3340]);
        assert_eq!(m.player.perk_id, 8010);
        assert_eq!(m.player.game_name, "fake-name-1");
        assert_eq!(matches[0].result, MatchResult::Loss);
        assert!(page_from_json(&json!({"games": {}}), None).is_none());
    }

    #[test]
    fn remake_nao_e_vitoria_nem_derrota() {
        let mut g = game(3, 3000, true, 1);
        g["participants"][0]["stats"]["gameEndedInEarlySurrender"] = json!(true);
        let (matches, _) = page_from_json(&page_json(vec![g]), None).unwrap();
        assert_eq!(matches[0].result, MatchResult::Remake);
    }

    #[test]
    fn placar_com_os_dez_jogadores_em_ordem() {
        let details = details_from_json(&game(9, 5000, true, 10)).unwrap();
        assert_eq!(details.players.len(), 10);
        assert_eq!(details.players[0].puuid, "fake-puuid-me");
        assert_eq!(details.players[0].team_id, 100);
        assert!(details.players[0].win);
        assert_eq!(details.players[9].team_id, 200);
        assert!(!details.players[9].win);
        assert!(!details.remake);
        assert!(details_from_json(&json!({"gameId": 0})).is_none());
    }

    #[test]
    fn jogador_logado_pelo_puuid() {
        let (matches, _) =
            page_from_json(&page_json(vec![game(4, 1, true, 10)]), Some("fake-puuid-7")).unwrap();
        assert_eq!(matches[0].player.puuid, "fake-puuid-7");
        assert_eq!(matches[0].result, MatchResult::Loss);
    }

    fn summaries(ids: &[(u64, u64)]) -> Vec<MatchSummary> {
        let games = ids.iter().map(|&(id, at)| game(id, at, true, 1)).collect();
        page_from_json(&page_json(games), None).unwrap().0
    }

    /// Resposta gravada no conjunto sintético `exemplo` para `path`.
    pub(crate) fn exemplo(path: &str) -> Value {
        let rest: Value =
            serde_json::from_str(include_str!("../tests/fixtures/exemplo/rest.json")).unwrap();
        rest.as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == path)
            .map(|e| e["body"].clone())
            .unwrap_or_else(|| panic!("o exemplo tem {path}"))
    }

    #[test]
    fn historico_do_exemplo() {
        let (first, received) =
            page_from_json(&exemplo(&page_path(0)), Some("fake-puuid-0001")).unwrap();
        assert_eq!((first.len(), received), (PAGE_SIZE, PAGE_SIZE));
        assert!(first.iter().all(|m| m.player.puuid == "fake-puuid-0001"));
        assert_eq!(
            first
                .iter()
                .filter(|m| m.result == MatchResult::Remake)
                .count(),
            1
        );
        let mut history = MatchHistory::first_page(first, received);
        let (second, received) = page_from_json(&exemplo(&page_path(20)), None).unwrap();
        history.append(second, received);
        assert_eq!(history.matches.len(), 26);
        assert!(!history.has_more);

        let newest = history.matches[0].game_id;
        let details = details_from_json(&exemplo(&format!("{GAME_PATH_PREFIX}/{newest}"))).unwrap();
        assert_eq!(details.players.len(), 10);
        let me = details
            .players
            .iter()
            .find(|p| p.puuid == "fake-puuid-0001")
            .unwrap();
        assert_eq!(*me, history.matches[0].player);
    }

    #[test]
    fn paginas_e_releitura() {
        let mut history = MatchHistory::first_page(summaries(&[(3, 30), (2, 20)]), PAGE_SIZE);
        assert!(history.has_more);
        history.append(summaries(&[(2, 20), (1, 10)]), 2);
        let ids: Vec<u64> = history.matches.iter().map(|m| m.game_id).collect();
        assert_eq!(ids, [3, 2, 1]);
        assert!(!history.has_more);
        // Uma partida nova depois do jogo: entra no topo e mantém o resto.
        history.merge_first_page(summaries(&[(4, 40), (3, 30)]), 2);
        let ids: Vec<u64> = history.matches.iter().map(|m| m.game_id).collect();
        assert_eq!(ids, [4, 3, 2, 1]);
        assert!(!history.has_more);
        assert_eq!(
            page_path(20),
            "/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=20&endIndex=39"
        );
    }
}
