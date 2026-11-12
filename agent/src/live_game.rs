//! Partida ao vivo (#268): as duas equipes da partida em andamento, no
//! estilo das telas "live" de sites como OP.GG.
//!
//! Tudo vem de fontes locais, sem chave da Riot:
//!
//! - `GET /lol-gameflow/v1/session` ([`GAMEFLOW_SESSION_PATH`]): fila, mapa,
//!   as duas equipes (`gameData.teamOne`/`teamTwo`, com `puuid`, campeão e
//!   cargo) e os feitiços (`gameData.playerChampionSelections`). Existe a
//!   partir de `GameStart`.
//! - Por jogador: Riot ID (`/lol-summoner/v2/summoners/puuid/{puuid}`),
//!   ranks e elo da temporada passada (`/lol-ranked/v1/ranked-stats/{puuid}`)
//!   e a primeira página do histórico
//!   (`/lol-match-history/v1/products/lol/{puuid}/matches`), de onde saem a
//!   taxa de vitória com o campeão, a rota mais jogada e as sequências. O
//!   histórico da LCU só tem as partidas recentes, então esses números são
//!   **das últimas partidas**, não da temporada.
//! - Runas e tempo de jogo: a Live Client Data API do jogo
//!   ([`crate::live_client`]), que só responde depois da tela de
//!   carregamento.
//!
//! Quem está em `playerChampionSelections` mas não aparece em nenhuma equipe
//! (jogador anônimo/modo streamer) entra só com o campeão e os feitiços: o
//! agente nunca busca nome, elo ou histórico dele.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::lobby::Position;
use crate::match_history::{self, MatchResult, MatchSummary};
use crate::player_profile::{self, RANKED_BY_PUUID_PREFIX};
use crate::ranked::{self, RankedEntry};
use crate::rest::{LcuClient, LcuError};

/// Sessão do gameflow: fila, mapa e as equipes da partida.
pub const GAMEFLOW_SESSION_PATH: &str = "/lol-gameflow/v1/session";

/// Fila da Ranqueada Flex: nela o elo mostrado é o da Flex.
const FLEX_QUEUE_ID: i32 = 440;

/// Mapa do Summoner's Rift, o único em que a rota das partidas recentes
/// conta para a rota principal.
const SUMMONERS_RIFT: u32 = 11;

/// Menos partidas com rota conhecida que isto: sem rota principal.
const MIN_LANE_GAMES: u32 = 3;

/// A partida em andamento, como a tela "Partida ao vivo" mostra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LiveGame {
    #[ts(type = "number")]
    pub game_id: u64,
    /// `queueId` da Riot (420 Solo/Duo, 450 ARAM...). `0` na personalizada.
    pub queue_id: i32,
    /// Nome da fila como a LCU manda, no idioma do client (`Ranqueada
    /// Solo/Duo`, `ARAM: Desordem`). Pode vir vazio.
    pub queue_name: String,
    pub map_id: u32,
    /// Nome do mapa no idioma do client (`Summoner's Rift`). Pode vir vazio.
    pub map_name: String,
    /// `gameMode` da LCU (`CLASSIC`, `ARAM`, `KIWI`, `CHERRY`...).
    pub game_mode: String,
    /// Fila cujo elo as linhas mostram: a Flex na Flex, a Solo/Duo nas
    /// outras.
    pub ranked_queue: RankedQueue,
    /// Segundos de jogo na última leitura da Live Client Data API. `null`
    /// enquanto o jogo carrega. A UI conta os segundos a partir daqui; o
    /// agente só manda de novo quando o relógio do jogo se afasta da conta.
    pub game_time_secs: Option<u32>,
    /// A equipe azul (`100`) e depois a vermelha (`200`). Na Arena e em
    /// outros modos com mais equipes, só as duas da LCU.
    pub teams: Vec<LiveTeam>,
}

/// Fila ranqueada cujo elo a partida ao vivo mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RankedQueue {
    SoloDuo,
    Flex,
}

/// Uma equipe da partida ao vivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LiveTeam {
    /// `100` (azul) ou `200` (vermelha).
    pub team_id: u32,
    /// Elo médio de quem tem classificação na fila de
    /// [`LiveGame::ranked_queue`]. `null` se ninguém tem (ou ainda não
    /// chegou).
    pub average_tier: Option<TierDivision>,
    /// Na ordem das rotas (topo, selva, meio, atirador, suporte) quando a
    /// fila tem cargos; senão, na ordem da LCU.
    pub players: Vec<LivePlayer>,
}

/// Um tier com divisão (`GOLD` `II`). A divisão é `null` de Mestre para
/// cima.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TierDivision {
    pub tier: String,
    pub division: Option<String>,
}

/// Um jogador da partida ao vivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LivePlayer {
    /// Vazio para bots e jogadores anônimos.
    pub puuid: String,
    /// É o jogador logado.
    pub me: bool,
    /// Bot (sem `puuid`, partida contra IA ou personalizada).
    pub bot: bool,
    /// Jogador anônimo (modo streamer): a LCU não o pôs em nenhuma equipe.
    /// Só campeão e feitiços; o agente não busca mais nada dele.
    pub hidden: bool,
    pub champion_id: u32,
    /// `0` quando a LCU não mandou.
    pub spell1_id: u32,
    pub spell2_id: u32,
    /// Cargo nesta partida, nas filas com cargos. `unselected` nas outras.
    pub position: Position,
    /// Riot ID. Vazio enquanto o agente busca, para bots e anônimos.
    pub game_name: String,
    pub tag_line: String,
    /// Runa principal (keystone), árvore principal e secundária. `null`
    /// até a Live Client Data API responder (depois da tela de
    /// carregamento).
    pub runes: Option<LiveRunes>,
    /// Elo e partidas recentes. `null` enquanto o agente busca, e para bots
    /// e anônimos.
    pub stats: Option<LivePlayerStats>,
}

/// Runas de um jogador na partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LiveRunes {
    /// `/assets/perk/{id}`.
    pub keystone_id: u32,
    /// `/assets/perkStyle/{id}`.
    pub primary_style_id: u32,
    pub sub_style_id: u32,
}

/// O que o agente achou de um jogador. Cada parte que a LCU não respondeu
/// fica vazia (`null` ou zero), sem esconder as outras.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LivePlayerStats {
    /// Elo atual na fila de [`LiveGame::ranked_queue`], com vitórias e
    /// derrotas da temporada (a taxa de vitória ranqueada). `null` se a
    /// LCU não respondeu os ranks.
    pub rank: Option<RankedEntry>,
    /// Elo do fim da temporada passada na mesma fila, ou `null` sem
    /// classificação nela.
    pub previous_season: Option<TierDivision>,
    /// Partidas lidas do histórico recente, sem os remakes. `0` com o
    /// histórico privado ou indisponível.
    pub recent_games: u32,
    /// Partidas e vitórias com o campeão desta partida, entre as recentes.
    pub champion_games: u32,
    pub champion_wins: u32,
    /// Rota mais jogada nas partidas recentes do Summoner's Rift, ou
    /// `unselected` com menos de 3 partidas de rota conhecida. Aproximada:
    /// a rota vem da estimativa da própria Riot em cada partida.
    pub main_position: Position,
    /// Sequência atual nas partidas recentes: positiva em vitórias,
    /// negativa em derrotas, `0` sem partidas.
    pub streak: i32,
}

/// Um jogador como a sessão do gameflow manda, antes das buscas.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionPlayer {
    team_id: u32,
    puuid: String,
    champion_id: u32,
    spell1_id: u32,
    spell2_id: u32,
    position: Position,
    bot: bool,
    hidden: bool,
    order: u32,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawSession {
    phase: String,
    game_data: RawGameData,
    map: RawMap,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawGameData {
    game_id: u64,
    queue: RawQueue,
    team_one: Vec<RawTeamMember>,
    team_two: Vec<RawTeamMember>,
    player_champion_selections: Vec<RawSelection>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawQueue {
    id: i32,
    description: String,
    name: String,
    game_mode: String,
    map_id: u32,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawMap {
    id: u32,
    name: String,
    game_mode: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawTeamMember {
    puuid: String,
    champion_id: u32,
    selected_position: Option<String>,
    bot_difficulty: Option<String>,
    team_participant_id: u32,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawSelection {
    puuid: String,
    champion_id: u32,
    spell1_id: u32,
    spell2_id: u32,
}

/// A partida da sessão do gameflow, sem os dados buscados por jogador.
/// `None` fora de partida ou sem nenhum jogador nas equipes (a LCU ainda
/// não montou a partida).
pub fn from_session(value: &Value, my_puuid: Option<&str>) -> Option<LiveGame> {
    let raw = RawSession::deserialize(value).ok()?;
    if !matches!(
        raw.phase.as_str(),
        "GameStart" | "InProgress" | "Reconnect" | ""
    ) {
        return None;
    }
    let data = &raw.game_data;
    let players = session_players(data);
    if players.is_empty() {
        return None;
    }
    let queue_id = data.queue.id.max(0);
    let ranked_queue = if queue_id == FLEX_QUEUE_ID {
        RankedQueue::Flex
    } else {
        RankedQueue::SoloDuo
    };
    let mut teams: Vec<LiveTeam> = [100, 200]
        .into_iter()
        .map(|team_id| {
            let mut members: Vec<&SessionPlayer> =
                players.iter().filter(|p| p.team_id == team_id).collect();
            members.sort_by_key(|p| (position_order(p.position), p.order));
            LiveTeam {
                team_id,
                average_tier: None,
                players: members
                    .into_iter()
                    .map(|p| LivePlayer {
                        puuid: p.puuid.clone(),
                        me: !p.puuid.is_empty() && Some(p.puuid.as_str()) == my_puuid,
                        bot: p.bot,
                        hidden: p.hidden,
                        champion_id: p.champion_id,
                        spell1_id: p.spell1_id,
                        spell2_id: p.spell2_id,
                        position: p.position,
                        game_name: String::new(),
                        tag_line: String::new(),
                        runes: None,
                        stats: None,
                    })
                    .collect(),
            }
        })
        .collect();
    teams.retain(|t| !t.players.is_empty());
    let queue_name = if data.queue.description.is_empty() {
        data.queue.name.clone()
    } else {
        data.queue.description.clone()
    };
    let game_mode = if raw.map.game_mode.is_empty() {
        data.queue.game_mode.clone()
    } else {
        raw.map.game_mode.clone()
    };
    Some(LiveGame {
        game_id: data.game_id,
        queue_id,
        queue_name,
        map_id: if raw.map.id == 0 {
            data.queue.map_id
        } else {
            raw.map.id
        },
        map_name: raw.map.name,
        game_mode,
        ranked_queue,
        game_time_secs: None,
        teams,
    })
}

/// A partida em andamento de outro jogador (#436), aberta no perfil dele.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerLiveGame {
    /// O jogador do perfil.
    pub puuid: String,
    pub game: LiveGame,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawSpectatorGame {
    game_id: u64,
    game_mode: String,
    map_id: u32,
    game_queue_config_id: i32,
    game_length: i64,
    participants: Vec<RawSpectatorParticipant>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawSpectatorParticipant {
    puuid: Option<String>,
    team_id: u32,
    champion_id: u32,
    spell1_id: u32,
    spell2_id: u32,
    bot: bool,
    riot_id: Option<String>,
    perks: Option<RawPerks>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawPerks {
    perk_ids: Vec<u32>,
    perk_style: u32,
    perk_sub_style: u32,
}

/// A partida do `spectator-v5` da API da Riot (#436), sem os dados que a
/// LCU completa (elo e partidas recentes). A API não manda os cargos: os
/// jogadores ficam na ordem dela, com `unselected`. O nome da fila e do
/// mapa ficam vazios. `None` sem nenhum jogador nas duas equipes.
pub fn from_spectator(value: &Value, my_puuid: Option<&str>) -> Option<LiveGame> {
    let raw = RawSpectatorGame::deserialize(value).ok()?;
    let queue_id = raw.game_queue_config_id.max(0);
    let ranked_queue = if queue_id == FLEX_QUEUE_ID {
        RankedQueue::Flex
    } else {
        RankedQueue::SoloDuo
    };
    let mut teams: Vec<LiveTeam> = [100, 200]
        .into_iter()
        .map(|team_id| LiveTeam {
            team_id,
            average_tier: None,
            players: raw
                .participants
                .iter()
                .filter(|p| p.team_id == team_id)
                .map(|p| {
                    let puuid = p
                        .puuid
                        .clone()
                        .filter(|id| !p.bot && player_profile::valid_puuid(id))
                        .unwrap_or_default();
                    let (game_name, tag_line) = p
                        .riot_id
                        .as_deref()
                        .filter(|_| !p.bot)
                        .map(|id| match id.split_once('#') {
                            Some((name, tag)) => (name.to_owned(), tag.to_owned()),
                            None => (id.to_owned(), String::new()),
                        })
                        .unwrap_or_default();
                    let runes = p.perks.as_ref().and_then(|perks| {
                        let keystone_id = perks.perk_ids.first().copied().unwrap_or(0);
                        (keystone_id != 0 || perks.perk_style != 0).then_some(LiveRunes {
                            keystone_id,
                            primary_style_id: perks.perk_style,
                            sub_style_id: perks.perk_sub_style,
                        })
                    });
                    LivePlayer {
                        me: !puuid.is_empty() && Some(puuid.as_str()) == my_puuid,
                        hidden: !p.bot && puuid.is_empty(),
                        puuid,
                        bot: p.bot,
                        champion_id: p.champion_id,
                        spell1_id: p.spell1_id,
                        spell2_id: p.spell2_id,
                        position: Position::Unselected,
                        game_name,
                        tag_line,
                        runes,
                        stats: None,
                    }
                })
                .collect(),
        })
        .collect();
    teams.retain(|t| !t.players.is_empty());
    if teams.is_empty() {
        return None;
    }
    Some(LiveGame {
        game_id: raw.game_id,
        queue_id,
        queue_name: String::new(),
        map_id: raw.map_id,
        map_name: String::new(),
        game_mode: raw.game_mode,
        ranked_queue,
        game_time_secs: u32::try_from(raw.game_length).ok().filter(|&secs| secs > 0),
        teams,
    })
}

/// Os jogadores das duas equipes com os feitiços, mais quem só está nas
/// seleções (anônimo), posto na equipe que tem menos gente.
fn session_players(data: &RawGameData) -> Vec<SessionPlayer> {
    let mut players = Vec::new();
    let mut used = vec![false; data.player_champion_selections.len()];
    for (team_id, members) in [(100, &data.team_one), (200, &data.team_two)] {
        for member in members {
            // Pelo puuid; sem ele (bot), pelo campeão.
            let selection = data
                .player_champion_selections
                .iter()
                .enumerate()
                .find(|(i, s)| {
                    !used[*i]
                        && if member.puuid.is_empty() {
                            s.champion_id == member.champion_id
                        } else {
                            s.puuid == member.puuid
                        }
                });
            let (spell1_id, spell2_id, champion_id) = match selection {
                Some((i, s)) => {
                    used[i] = true;
                    let champion = if member.champion_id == 0 {
                        s.champion_id
                    } else {
                        member.champion_id
                    };
                    (s.spell1_id, s.spell2_id, champion)
                }
                None => (0, 0, member.champion_id),
            };
            let bot = member.puuid.is_empty()
                || member
                    .bot_difficulty
                    .as_deref()
                    .is_some_and(|d| !d.is_empty() && d != "NONE");
            players.push(SessionPlayer {
                team_id,
                puuid: if bot {
                    String::new()
                } else {
                    member.puuid.clone()
                },
                champion_id,
                spell1_id,
                spell2_id,
                position: Position::from_lcu(member.selected_position.as_deref()),
                bot,
                hidden: false,
                order: member.team_participant_id,
            });
        }
    }
    for (i, s) in data.player_champion_selections.iter().enumerate() {
        if used[i] || s.champion_id == 0 {
            continue;
        }
        let blue = players.iter().filter(|p| p.team_id == 100).count();
        let red = players.iter().filter(|p| p.team_id == 200).count();
        players.push(SessionPlayer {
            team_id: if red < blue { 200 } else { 100 },
            // Anônimo: o puuid não sai daqui.
            puuid: String::new(),
            champion_id: s.champion_id,
            spell1_id: s.spell1_id,
            spell2_id: s.spell2_id,
            position: Position::Unselected,
            bot: false,
            hidden: true,
            order: u32::MAX,
        });
    }
    players
}

fn position_order(position: Position) -> usize {
    Position::ALL
        .iter()
        .position(|p| *p == position)
        .unwrap_or(Position::ALL.len())
}

/// Ordem dos tiers com divisão; de Mestre para cima, um degrau por tier.
const TIERS: [&str; 10] = [
    "IRON",
    "BRONZE",
    "SILVER",
    "GOLD",
    "PLATINUM",
    "EMERALD",
    "DIAMOND",
    "MASTER",
    "GRANDMASTER",
    "CHALLENGER",
];
const DIVISIONS: [&str; 4] = ["IV", "III", "II", "I"];
/// Primeiro tier sem divisão.
const MASTER_INDEX: usize = 7;

/// Degrau do elo: Ferro IV é 0, Diamante I é 27, Mestre 28...
fn tier_score(tier: &str, division: Option<&str>) -> Option<usize> {
    let index = TIERS.iter().position(|t| *t == tier)?;
    if index >= MASTER_INDEX {
        return Some(MASTER_INDEX * 4 + index - MASTER_INDEX);
    }
    let division = DIVISIONS.iter().position(|d| Some(*d) == division)?;
    Some(index * 4 + division)
}

fn tier_from_score(score: usize) -> TierDivision {
    if score >= MASTER_INDEX * 4 {
        let index = (MASTER_INDEX + score - MASTER_INDEX * 4).min(TIERS.len() - 1);
        return TierDivision {
            tier: TIERS[index].to_owned(),
            division: None,
        };
    }
    TierDivision {
        tier: TIERS[score / 4].to_owned(),
        division: Some(DIVISIONS[score % 4].to_owned()),
    }
}

/// Elo médio de quem tem classificação, arredondado para o degrau mais
/// perto. `None` se ninguém tem.
pub fn average_tier<'a>(
    entries: impl IntoIterator<Item = &'a RankedEntry>,
) -> Option<TierDivision> {
    let scores: Vec<usize> = entries
        .into_iter()
        .filter_map(|e| tier_score(e.tier.as_deref()?, e.division.as_deref()))
        .collect();
    if scores.is_empty() {
        return None;
    }
    let sum: usize = scores.iter().sum();
    let average = (sum + scores.len() / 2) / scores.len();
    Some(tier_from_score(average))
}

impl LiveGame {
    /// Recalcula o elo médio de cada equipe.
    pub fn refresh_averages(&mut self) {
        for team in &mut self.teams {
            team.average_tier = average_tier(
                team.players
                    .iter()
                    .filter_map(|p| p.stats.as_ref()?.rank.as_ref()),
            );
        }
    }

    /// Todos os jogadores, das duas equipes.
    pub fn players_mut(&mut self) -> impl Iterator<Item = &mut LivePlayer> {
        self.teams.iter_mut().flat_map(|t| t.players.iter_mut())
    }

    pub fn players(&self) -> impl Iterator<Item = &LivePlayer> {
        self.teams.iter().flat_map(|t| t.players.iter())
    }
}

/// `queueType` da fila na LCU.
fn queue_type(queue: RankedQueue) -> &'static str {
    match queue {
        RankedQueue::SoloDuo => "RANKED_SOLO_5x5",
        RankedQueue::Flex => "RANKED_FLEX_SR",
    }
}

/// Elo atual e da temporada passada na fila `queue`, do JSON de
/// `/lol-ranked/v1/ranked-stats/{puuid}`.
pub fn rank_from_json(
    value: &Value,
    queue: RankedQueue,
) -> Option<(RankedEntry, Option<TierDivision>)> {
    let stats = ranked::from_json(value)?;
    let entry = match queue {
        RankedQueue::SoloDuo => stats.solo_duo,
        RankedQueue::Flex => stats.flex,
    };
    Some((entry, previous_season(value, queue)))
}

fn previous_season(value: &Value, queue: RankedQueue) -> Option<TierDivision> {
    let wanted = queue_type(queue);
    let from_list = value
        .get("queues")
        .and_then(Value::as_array)
        .and_then(|list| list.iter().find(|q| q["queueType"] == wanted));
    let raw = from_list.or_else(|| value.get("queueMap")?.get(wanted))?;
    let tier = raw
        .get("previousSeasonEndTier")?
        .as_str()?
        .trim()
        .to_ascii_uppercase();
    if !TIERS.contains(&tier.as_str()) {
        return None;
    }
    let division = raw
        .get("previousSeasonEndDivision")
        .and_then(Value::as_str)
        .map(|d| d.trim().to_ascii_uppercase())
        .filter(|d| {
            DIVISIONS.contains(&d.as_str()) && !TIERS[MASTER_INDEX..].contains(&tier.as_str())
        });
    Some(TierDivision { tier, division })
}

/// Rota de um jogador numa partida do histórico, pela estimativa da Riot
/// (`timeline.lane` e `timeline.role`).
fn lane_position(lane: &str, role: &str) -> Position {
    match lane {
        "TOP" => Position::Top,
        "JUNGLE" => Position::Jungle,
        "MIDDLE" | "MID" => Position::Middle,
        "BOTTOM" | "BOT" if role.contains("SUPPORT") => Position::Utility,
        "BOTTOM" | "BOT" => Position::Bottom,
        _ if role.contains("SUPPORT") => Position::Utility,
        _ => Position::Unselected,
    }
}

/// Rota de `puuid` em cada partida do Summoner's Rift da página do
/// histórico, por `gameId`.
fn lanes_from_json(value: &Value, puuid: &str) -> HashMap<u64, Position> {
    let mut lanes = HashMap::new();
    let Some(games) = value
        .get("games")
        .and_then(|g| g.get("games"))
        .and_then(Value::as_array)
    else {
        return lanes;
    };
    for game in games {
        if game["mapId"].as_u64() != Some(u64::from(SUMMONERS_RIFT)) {
            continue;
        }
        let Some(game_id) = game["gameId"].as_u64() else {
            continue;
        };
        let participant_id = game["participantIdentities"]
            .as_array()
            .and_then(|ids| ids.iter().find(|i| i["player"]["puuid"] == puuid))
            .and_then(|i| i["participantId"].as_u64());
        let participants = game["participants"].as_array();
        let participant = participants.and_then(|list| match participant_id {
            Some(id) => list
                .iter()
                .find(|p| p["participantId"].as_u64() == Some(id)),
            // Página de um jogador só: ele é o único participante.
            None if list.len() == 1 => list.first(),
            None => None,
        });
        let Some(timeline) = participant.map(|p| &p["timeline"]) else {
            continue;
        };
        let lane = timeline["lane"].as_str().unwrap_or_default();
        let role = timeline["role"].as_str().unwrap_or_default();
        let position = lane_position(lane, role);
        if position != Position::Unselected {
            lanes.insert(game_id, position);
        }
    }
    lanes
}

/// Números das partidas recentes: com o campeão, rota principal e
/// sequência.
pub fn recent_stats(
    matches: &[MatchSummary],
    lanes: &HashMap<u64, Position>,
    champion_id: u32,
) -> LivePlayerStats {
    // `matches` vem da mais nova para a mais antiga.
    let counted: Vec<&MatchSummary> = matches
        .iter()
        .filter(|m| m.result != MatchResult::Remake)
        .collect();
    let mut stats = LivePlayerStats {
        recent_games: u32::try_from(counted.len()).unwrap_or(u32::MAX),
        ..LivePlayerStats::default()
    };
    for m in &counted {
        if m.player.champion_id == champion_id {
            stats.champion_games += 1;
            if m.result == MatchResult::Win {
                stats.champion_wins += 1;
            }
        }
    }
    if let Some(first) = counted.first() {
        let won = first.result == MatchResult::Win;
        let run = counted
            .iter()
            .take_while(|m| (m.result == MatchResult::Win) == won)
            .count();
        let run = i32::try_from(run).unwrap_or(i32::MAX);
        stats.streak = if won { run } else { -run };
    }
    let mut counts: HashMap<Position, u32> = HashMap::new();
    for m in &counted {
        if let Some(position) = lanes.get(&m.game_id) {
            *counts.entry(*position).or_default() += 1;
        }
    }
    let known: u32 = counts.values().sum();
    if known >= MIN_LANE_GAMES {
        stats.main_position = counts
            .into_iter()
            .max_by_key(|(position, n)| (*n, std::cmp::Reverse(position_order(*position))))
            .map(|(position, _)| position)
            .unwrap_or_default();
    }
    stats
}

/// Riot ID de `puuid`. Sem Riot ID (conta antiga), o nome antigo de
/// invocador.
pub async fn riot_id(lcu: &LcuClient, puuid: &str) -> Result<Option<(String, String)>, LcuError> {
    Ok(player_profile::summoner(lcu, puuid).await?.map(|(s, _)| {
        if s.game_name.is_empty() {
            (s.display_name, String::new())
        } else {
            (s.game_name, s.tag_line)
        }
    }))
}

/// Busca o elo e as partidas recentes de `puuid`, jogando `champion_id`.
/// Uma parte que falha fica vazia.
pub async fn player_stats(
    lcu: &LcuClient,
    puuid: &str,
    champion_id: u32,
    queue: RankedQueue,
) -> LivePlayerStats {
    let ranked_path = format!("{RANKED_BY_PUUID_PREFIX}/{puuid}");
    let page_path = player_profile::page_path(0, puuid);
    let (ranked, page) =
        tokio::join!(lcu.get::<Value>(&ranked_path), lcu.get::<Value>(&page_path),);
    let mut stats = match &page {
        Ok(body) => {
            let matches = match_history::page_from_json(body, Some(puuid))
                .map(|(m, _)| m)
                .unwrap_or_default();
            recent_stats(&matches, &lanes_from_json(body, puuid), champion_id)
        }
        Err(_) => LivePlayerStats::default(),
    };
    if let Some((rank, previous)) = ranked.ok().and_then(|body| rank_from_json(&body, queue)) {
        stats.rank = Some(rank);
        stats.previous_season = previous;
    }
    stats
}

/// Lê a sessão do gameflow e monta a partida.
pub async fn current(
    lcu: &LcuClient,
    my_puuid: Option<&str>,
) -> Result<Option<LiveGame>, LcuError> {
    let body = lcu.get::<Value>(GAMEFLOW_SESSION_PATH).await?;
    Ok(from_session(&body, my_puuid))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::match_history::tests::{game, page_json};
    use serde_json::json;

    /// A sessão em `InProgress` da gravação real da Ranqueada Solo/Duo.
    pub(crate) fn gravacao_em_jogo() -> Value {
        let rest: Value =
            serde_json::from_str(include_str!("../tests/fixtures/16.22/rest.json")).unwrap();
        rest.as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == GAMEFLOW_SESSION_PATH && e["phase"] == "InProgress")
            .map(|e| e["body"].clone())
            .expect("a gravação tem a sessão em jogo")
    }

    #[test]
    fn partida_da_gravacao_real() {
        let game = from_session(&gravacao_em_jogo(), Some("fake-puuid-0001")).unwrap();
        assert_eq!(game.queue_id, 420);
        assert_eq!(game.queue_name, "Ranqueada Solo/Duo");
        assert_eq!(game.map_id, 11);
        assert_eq!(game.map_name, "Summoner's Rift");
        assert_eq!(game.game_mode, "CLASSIC");
        assert_eq!(game.ranked_queue, RankedQueue::SoloDuo);
        assert_eq!(game.teams.len(), 2);
        let blue = &game.teams[0];
        assert_eq!(blue.team_id, 100);
        // Na ordem das rotas.
        let positions: Vec<Position> = blue.players.iter().map(|p| p.position).collect();
        assert_eq!(
            positions,
            vec![
                Position::Top,
                Position::Jungle,
                Position::Middle,
                Position::Bottom,
                Position::Utility
            ]
        );
        let me = &blue.players[1];
        assert!(me.me);
        assert_eq!(me.champion_id, 121);
        assert_eq!((me.spell1_id, me.spell2_id), (4, 11));
        assert_eq!(game.players().filter(|p| p.me).count(), 1);
        // O inimigo que a LCU não pôs na equipe entra anônimo, na equipe
        // que tem menos gente.
        let red = &game.teams[1];
        assert_eq!(red.players.len(), 5);
        let hidden: Vec<&LivePlayer> = red.players.iter().filter(|p| p.hidden).collect();
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].champion_id, 26);
        assert_eq!(hidden[0].puuid, "");
        assert_eq!((hidden[0].spell1_id, hidden[0].spell2_id), (4, 7));
        assert!(game
            .players()
            .all(|p| p.stats.is_none() && p.runes.is_none()));
    }

    /// Uma resposta do `spectator-v5` no formato da documentação da Riot.
    pub(crate) fn spectator_json() -> Value {
        let player = |puuid: &str, team: u32, champion: u32, riot_id: &str| {
            json!({
                "puuid": puuid, "teamId": team, "championId": champion,
                "spell1Id": 4, "spell2Id": 14, "bot": false, "riotId": riot_id,
                "profileIconId": 1, "summonerId": "x",
                "perks": {"perkIds": [8112, 8126, 8138, 8135, 8226, 8237], "perkStyle": 8100, "perkSubStyle": 8200},
                "gameCustomizationObjects": []
            })
        };
        json!({
            "gameId": 3123456789u64, "gameType": "MATCHED", "gameStartTime": 1793841299452u64,
            "mapId": 11, "gameLength": 754, "platformId": "BR1", "gameMode": "CLASSIC",
            "gameQueueConfigId": 440,
            "bannedChampions": [],
            "observers": {"encryptionKey": "segredo"},
            "participants": [
                player("fake-puuid-0002", 100, 103, "Amigo#BR1"),
                player("fake-puuid-0001", 100, 121, "Eu#BR1"),
                player("fake-puuid-0003", 200, 26, "SemTag"),
                {"puuid": null, "teamId": 200, "championId": 1, "spell1Id": 0, "spell2Id": 0, "bot": true, "riotId": "Annie Bot"},
                {"puuid": null, "teamId": 200, "championId": 64, "spell1Id": 11, "spell2Id": 4, "bot": false}
            ]
        })
    }

    #[test]
    fn partida_do_espectador() {
        let game = from_spectator(&spectator_json(), Some("fake-puuid-0001")).unwrap();
        assert_eq!(game.game_id, 3_123_456_789);
        assert_eq!(game.queue_id, 440);
        assert_eq!(game.ranked_queue, RankedQueue::Flex);
        assert_eq!(game.map_id, 11);
        assert_eq!(game.game_mode, "CLASSIC");
        assert_eq!(game.game_time_secs, Some(754));
        assert_eq!(game.teams.len(), 2);
        let blue = &game.teams[0];
        assert_eq!(blue.team_id, 100);
        let friend = &blue.players[0];
        assert_eq!(
            (friend.game_name.as_str(), friend.tag_line.as_str()),
            ("Amigo", "BR1")
        );
        assert_eq!(friend.champion_id, 103);
        assert_eq!((friend.spell1_id, friend.spell2_id), (4, 14));
        assert_eq!(friend.position, Position::Unselected);
        assert_eq!(
            friend.runes,
            Some(LiveRunes {
                keystone_id: 8112,
                primary_style_id: 8100,
                sub_style_id: 8200
            })
        );
        assert!(!friend.me);
        assert!(blue.players[1].me);
        let red = &game.teams[1];
        assert_eq!(red.players[0].game_name, "SemTag");
        assert_eq!(red.players[0].tag_line, "");
        let bot = &red.players[1];
        assert!(bot.bot && !bot.hidden && bot.puuid.is_empty() && bot.game_name.is_empty());
        let hidden = &red.players[2];
        assert!(hidden.hidden && !hidden.bot && hidden.runes.is_none());
        assert!(game.players().all(|p| p.stats.is_none()));
    }

    #[test]
    fn espectador_sem_jogadores() {
        assert!(from_spectator(&json!({"gameId": 1, "participants": []}), None).is_none());
        assert!(from_spectator(&json!("x"), None).is_none());
        let loading = json!({"gameId": 1, "gameLength": 0, "participants": [
            {"puuid": "fake-puuid-0002", "teamId": 100, "championId": 1}
        ]});
        assert_eq!(from_spectator(&loading, None).unwrap().game_time_secs, None);
    }

    #[test]
    fn fora_de_partida_nao_tem_partida() {
        assert!(from_session(&json!({"phase": "Lobby", "gameData": {}}), None).is_none());
        assert!(from_session(&json!({"phase": "InProgress", "gameData": {}}), None).is_none());
        assert!(from_session(&json!([1]), None).is_none());
    }

    #[test]
    fn bots_e_flex() {
        let session = json!({
            "phase": "InProgress",
            "gameData": {
                "gameId": 9,
                "queue": {"id": 440, "description": "Ranqueada Flex", "gameMode": "CLASSIC", "mapId": 11},
                "teamOne": [{"puuid": "eu", "championId": 1, "selectedPosition": "MIDDLE"}],
                "teamTwo": [{"puuid": "", "championId": 2, "botDifficulty": "EASY"}],
                "playerChampionSelections": [
                    {"puuid": "eu", "championId": 1, "spell1Id": 4, "spell2Id": 14},
                    {"championId": 2, "spell1Id": 3, "spell2Id": 4}
                ]
            },
            "map": {"id": 11, "name": "Summoner's Rift", "gameMode": "CLASSIC"}
        });
        let game = from_session(&session, Some("eu")).unwrap();
        assert_eq!(game.ranked_queue, RankedQueue::Flex);
        let bot = &game.teams[1].players[0];
        assert!(bot.bot);
        assert!(!bot.hidden);
        assert_eq!((bot.spell1_id, bot.spell2_id), (3, 4));
        assert_eq!(game.teams[0].players[0].position, Position::Middle);
    }

    fn entry(tier: &str, division: Option<&str>) -> RankedEntry {
        RankedEntry {
            tier: Some(tier.into()),
            division: division.map(Into::into),
            ..RankedEntry::default()
        }
    }

    #[test]
    fn elo_medio() {
        assert_eq!(average_tier([]), None);
        assert_eq!(average_tier([&RankedEntry::default()]), None);
        let gold2 = entry("GOLD", Some("II"));
        let plat4 = entry("PLATINUM", Some("IV"));
        assert_eq!(
            average_tier([&gold2, &plat4, &RankedEntry::default()]),
            Some(TierDivision {
                tier: "GOLD".into(),
                division: Some("I".into())
            })
        );
        let master = entry("MASTER", None);
        let challenger = entry("CHALLENGER", None);
        assert_eq!(
            average_tier([&master, &challenger]),
            Some(TierDivision {
                tier: "GRANDMASTER".into(),
                division: None
            })
        );
        let iron = entry("IRON", Some("IV"));
        assert_eq!(
            average_tier([&iron]),
            Some(TierDivision {
                tier: "IRON".into(),
                division: Some("IV".into())
            })
        );
    }

    #[test]
    fn elo_e_temporada_passada() {
        let body = crate::match_history::tests::gravacao_perfil(ranked::RANKED_STATS_PATH);
        let (rank, previous) = rank_from_json(&body, RankedQueue::SoloDuo).unwrap();
        assert_eq!(rank.tier.as_deref(), Some("DIAMOND"));
        assert_eq!(rank.wins, 144);
        assert_eq!(
            previous,
            Some(TierDivision {
                tier: "DIAMOND".into(),
                division: Some("II".into())
            })
        );
        let body = json!({"queueMap": {"RANKED_FLEX_SR": {
            "tier": "GOLD", "division": "I", "previousSeasonEndTier": "MASTER",
            "previousSeasonEndDivision": "I"
        }}});
        let (rank, previous) = rank_from_json(&body, RankedQueue::Flex).unwrap();
        assert_eq!(rank.tier.as_deref(), Some("GOLD"));
        assert_eq!(
            previous,
            Some(TierDivision {
                tier: "MASTER".into(),
                division: None
            })
        );
        let body = json!({"queues": [{"queueType": "RANKED_SOLO_5x5",
            "previousSeasonEndTier": "NONE", "previousSeasonEndDivision": "NA"}]});
        assert_eq!(rank_from_json(&body, RankedQueue::SoloDuo).unwrap().1, None);
    }

    #[test]
    fn partidas_recentes() {
        // Da mais nova para a mais antiga: V V D V, com o campeão 7 em duas.
        let jogo = |id: u64, win: bool, champion: u32| {
            let mut g = game(id, id * 10, win, 1);
            g["participants"][0]["championId"] = json!(champion);
            g
        };
        let body = page_json(vec![
            jogo(5, true, 7),
            jogo(4, true, 1),
            jogo(3, false, 7),
            jogo(1, true, 2),
        ]);
        let (matches, _) = match_history::page_from_json(&body, None).unwrap();
        let mut lanes = HashMap::new();
        lanes.insert(5, Position::Middle);
        lanes.insert(4, Position::Middle);
        lanes.insert(3, Position::Top);
        let stats = recent_stats(&matches, &lanes, 7);
        assert_eq!(stats.recent_games, 4);
        assert_eq!(stats.champion_games, 2);
        assert_eq!(stats.champion_wins, 1);
        assert_eq!(stats.streak, 2);
        assert_eq!(stats.main_position, Position::Middle);

        lanes.remove(&3);
        assert_eq!(
            recent_stats(&matches, &lanes, 7).main_position,
            Position::Unselected,
            "menos de 3 partidas com rota"
        );
        let empty = recent_stats(&[], &HashMap::new(), 7);
        assert_eq!(empty, LivePlayerStats::default());
    }

    #[test]
    fn rotas_do_historico() {
        assert_eq!(lane_position("BOTTOM", "DUO_SUPPORT"), Position::Utility);
        assert_eq!(lane_position("BOTTOM", "CARRY"), Position::Bottom);
        assert_eq!(lane_position("NONE", "SUPPORT"), Position::Utility);
        assert_eq!(lane_position("NONE", "DUO"), Position::Unselected);
        let body = crate::match_history::tests::gravacao_perfil(
            "/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex=19",
        );
        let lanes = lanes_from_json(&body, "fake-puuid-0001");
        // Só as partidas do Summoner's Rift com rota estimada.
        assert!(!lanes.is_empty());
        assert!(lanes.values().all(|p| *p != Position::Unselected));
    }
}
