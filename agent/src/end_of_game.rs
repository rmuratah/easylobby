//! Resultado da partida que acabou de terminar (#179).
//!
//! No fim da partida a LCU monta o bloco de estatísticas em
//! `GET /lol-end-of-game/v1/eog-stats-block` e avisa pelo WAMP no mesmo URI
//! (`Create` e `Update` com o bloco, `Delete` na volta ao lobby). Fora do
//! pós-jogo a LCU responde 404. Na gravação do LoL real (`16.22-pos-jogo`,
//! ranqueada Solo/Duo) o bloco já existe em `WaitingForStats` e fica até o
//! "Jogar de novo".
//!
//! O bloco traz os dois times (`teams`, com `isPlayerTeam` e
//! `isWinningTeam`) e cada jogador com campeão, feitiços, os sete itens e as
//! estatísticas em `stats`. As estatísticas vêm duas vezes, em maiúsculas
//! (`CHAMPIONS_KILLED`) e em camelCase (`kills`); o agente aceita as duas.
//!
//! O bloco não diz a fila pelo `queueId` (só `queueType`, vazio fora das
//! ranqueadas) nem a mudança de PDL. A fila sai da sessão do gameflow
//! ([`queue_from_session`]); a mudança de PDL sai da comparação dos ranks do
//! começo da partida com os de depois ([`ranked_change`]).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::live_game::RankedQueue;
use crate::match_history::{MatchPlayer, MatchResult};
use crate::ranked::{RankedEntry, RankedStats};
use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) do bloco de estatísticas.
pub const EOG_STATS_PATH: &str = "/lol-end-of-game/v1/eog-stats-block";

/// Dispensa as estatísticas e sai do pós-jogo (#180): o "fechar" da tela de
/// fim de partida do client oficial. A fase vai para `None`.
pub const DISMISS_STATS_PATH: &str = "/lol-end-of-game/v1/state/dismiss-stats";

/// Sessão do gameflow, de onde sai a fila da partida.
pub const GAMEFLOW_SESSION_PATH: &str = crate::live_game::GAMEFLOW_SESSION_PATH;

/// `queueType` da Ranqueada Solo/Duo.
const SOLO_DUO: &str = "RANKED_SOLO_5x5";
/// `queueType` da Ranqueada Flex.
const FLEX: &str = "RANKED_FLEX_SR";
/// `queueId` da Ranqueada Solo/Duo.
const SOLO_DUO_QUEUE_ID: u32 = 420;
/// `queueId` da Ranqueada Flex.
const FLEX_QUEUE_ID: u32 = 440;

/// O resultado da partida que acabou de terminar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GameResult {
    #[ts(type = "number")]
    pub game_id: u64,
    /// `queueId` da Riot (420 Solo/Duo, 440 Flex, 450 ARAM...), ou `0` se
    /// o agente não conseguiu ler a sessão do gameflow.
    pub queue_id: u32,
    /// Nome da fila no idioma do client (`Ranqueada Solo/Duo`). Pode vir
    /// vazio.
    pub queue_name: String,
    /// `CLASSIC`, `ARAM`, `CHERRY`...
    pub game_mode: String,
    /// Partida ranqueada.
    pub ranked: bool,
    pub duration_secs: u32,
    /// Do ponto de vista do jogador logado.
    pub result: MatchResult,
    /// `puuid` do jogador logado, para destacá-lo no placar. Vazio se o
    /// bloco não marcou ninguém.
    pub local_puuid: String,
    /// O time do jogador logado primeiro; os jogadores na ordem da LCU
    /// (por rota em Summoner's Rift).
    pub teams: Vec<GameResultTeam>,
    /// Mudança de PDL e de elo na ranqueada. `null` fora da ranqueada, num
    /// remake e enquanto a LCU não manda os ranks novos (chegam alguns
    /// segundos depois do bloco).
    pub ranked_change: Option<RankedChange>,
}

/// Um time no resultado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GameResultTeam {
    /// `100` (azul) ou `200` (vermelho).
    pub team_id: u32,
    /// O time do jogador logado.
    pub player_team: bool,
    pub win: bool,
    pub players: Vec<MatchPlayer>,
    /// Estatísticas avançadas (#586), na mesma ordem de `players`.
    pub advanced: Vec<AdvancedStats>,
}

/// Estatísticas avançadas de um jogador (#586), o "Exibir detalhes
/// avançados" do placar do client oficial. Saem do `stats` do bloco; `0`
/// quando a LCU não mandou (algumas não existem em todos os modos).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedStats {
    /// Maior sequência de abates.
    pub largest_killing_spree: u32,
    pub largest_multi_kill: u32,
    pub double_kills: u32,
    pub triple_kills: u32,
    pub quadra_kills: u32,
    pub penta_kills: u32,
    pub largest_critical_strike: u32,
    /// Dano a campeões por tipo; o total é o `damageToChampions` do jogador.
    pub physical_damage_to_champions: u32,
    pub magic_damage_to_champions: u32,
    pub true_damage_to_champions: u32,
    /// Dano causado a tudo (campeões, tropas, monstros, estruturas).
    pub damage_dealt: u32,
    pub physical_damage_dealt: u32,
    pub magic_damage_dealt: u32,
    pub true_damage_dealt: u32,
    pub damage_taken: u32,
    pub physical_damage_taken: u32,
    pub magic_damage_taken: u32,
    pub true_damage_taken: u32,
    pub damage_self_mitigated: u32,
    pub heal: u32,
    pub heal_on_teammates: u32,
    pub shield_on_teammates: u32,
    /// Segundos controlando inimigos (atordoar, enraizar...).
    pub time_ccing_others: u32,
    pub wards_placed: u32,
    pub wards_killed: u32,
    pub control_wards_bought: u32,
    pub gold_spent: u32,
    /// Tropas; com os monstros, dá o `cs` do jogador.
    pub minions_killed: u32,
    pub neutral_minions_killed: u32,
    pub ally_jungle_minions_killed: u32,
    pub enemy_jungle_minions_killed: u32,
    pub turret_kills: u32,
    pub turret_takedowns: u32,
    pub inhibitor_kills: u32,
    pub damage_to_turrets: u32,
    pub damage_to_objectives: u32,
    pub damage_to_epic_monsters: u32,
    pub dragon_kills: u32,
    pub baron_kills: u32,
    pub objectives_stolen: u32,
    pub longest_time_living_secs: u32,
    pub time_dead_secs: u32,
    /// Habilidades usadas (Q, W, E, R) e feitiços de invocador.
    pub spell1_casts: u32,
    pub spell2_casts: u32,
    pub spell3_casts: u32,
    pub spell4_casts: u32,
    pub summoner1_casts: u32,
    pub summoner2_casts: u32,
    /// Pings.
    pub all_in_pings: u32,
    pub assist_me_pings: u32,
    pub basic_pings: u32,
    pub command_pings: u32,
    pub danger_pings: u32,
    pub enemy_missing_pings: u32,
    pub enemy_vision_pings: u32,
    pub get_back_pings: u32,
    pub hold_pings: u32,
    pub need_vision_pings: u32,
    pub on_my_way_pings: u32,
    pub push_pings: u32,
    pub retreat_pings: u32,
    pub vision_cleared_pings: u32,
    /// As seis runas escolhidas (`PERK0` a `PERK5`: a principal, as três da
    /// árvore primária e as duas da secundária) com os valores do fim da
    /// partida. Vazia nos modos sem runas (ARAM: Desordem, Arena).
    pub perks: Vec<PerkStats>,
}

/// Uma runa e os valores dela no fim da partida (#586), que entram no lugar
/// de `@eogvar1@` a `@eogvar3@` nas descrições do catálogo de runas.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PerkStats {
    pub id: u32,
    pub vars: [u32; 3],
}

/// Rank antes e depois da partida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RankedChange {
    pub queue: RankedQueue,
    pub before: RankedEntry,
    pub after: RankedEntry,
    /// PDL ganhos (positivo) ou perdidos (negativo). `null` quando o tier
    /// ou a divisão mudaram, porque aí a conta de PDL recomeça.
    pub league_points_delta: Option<i32>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawBlock {
    game_id: u64,
    game_length: u32,
    game_mode: String,
    queue_type: String,
    ranked: bool,
    game_ended_in_early_surrender: bool,
    team_early_surrendered: bool,
    local_player: RawPlayer,
    teams: Vec<RawTeam>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawTeam {
    team_id: u32,
    is_player_team: bool,
    is_winning_team: bool,
    players: Vec<RawPlayer>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawPlayer {
    puuid: String,
    riot_id_game_name: String,
    riot_id_tag_line: String,
    summoner_name: String,
    profile_icon_id: u32,
    team_id: u32,
    champion_id: u32,
    spell1_id: u32,
    spell2_id: u32,
    items: Vec<u32>,
    is_local_player: bool,
    stats: Value,
}

impl RawPlayer {
    /// A estatística `keys[0]` (ou a primeira das outras grafias que vier),
    /// `0` se não veio.
    fn stat(&self, keys: &[&str]) -> u32 {
        keys.iter()
            .find_map(|key| self.stats.get(*key).and_then(Value::as_u64))
            .map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX))
    }

    fn to_player(&self, team_id: u32, win: bool) -> MatchPlayer {
        let game_name = if self.riot_id_game_name.is_empty() {
            self.summoner_name.clone()
        } else {
            self.riot_id_game_name.clone()
        };
        let mut items = self.items.clone();
        items.resize(7, 0);
        MatchPlayer {
            puuid: self.puuid.clone(),
            game_name,
            tag_line: self.riot_id_tag_line.clone(),
            profile_icon_id: self.profile_icon_id,
            team_id: if self.team_id == 0 {
                team_id
            } else {
                self.team_id
            },
            win,
            champion_id: self.champion_id,
            champion_level: self.stat(&["LEVEL", "champLevel"]),
            spell1_id: self.spell1_id,
            spell2_id: self.spell2_id,
            kills: self.stat(&["CHAMPIONS_KILLED", "kills"]),
            deaths: self.stat(&["NUM_DEATHS", "deaths"]),
            assists: self.stat(&["ASSISTS", "assists"]),
            cs: self.stat(&["MINIONS_KILLED", "totalMinionsKilled"])
                + self.stat(&["NEUTRAL_MINIONS_KILLED", "neutralMinionsKilled"]),
            gold: self.stat(&["GOLD_EARNED", "goldEarned"]),
            damage_to_champions: self.stat(&[
                "TOTAL_DAMAGE_DEALT_TO_CHAMPIONS",
                "totalDamageDealtToChampions",
            ]),
            vision_score: self.stat(&["VISION_SCORE", "visionScore"]),
            items,
            perk_id: self.stat(&["PERK0", "perk0"]),
            perk_style_id: self.stat(&["PERK_PRIMARY_STYLE", "perkPrimaryStyle"]),
            perk_sub_style_id: self.stat(&["PERK_SUB_STYLE", "perkSubStyle"]),
        }
    }
}

impl RawPlayer {
    fn to_advanced(&self) -> AdvancedStats {
        AdvancedStats {
            largest_killing_spree: self.stat(&["LARGEST_KILLING_SPREE", "largestKillingSpree"]),
            largest_multi_kill: self.stat(&["LARGEST_MULTI_KILL", "largestMultiKill"]),
            double_kills: self.stat(&["doubleKills"]),
            triple_kills: self.stat(&["tripleKills"]),
            quadra_kills: self.stat(&["quadraKills"]),
            penta_kills: self.stat(&["pentaKills"]),
            largest_critical_strike: self
                .stat(&["LARGEST_CRITICAL_STRIKE", "largestCriticalStrike"]),
            physical_damage_to_champions: self.stat(&[
                "PHYSICAL_DAMAGE_DEALT_TO_CHAMPIONS",
                "physicalDamageDealtToChampions",
            ]),
            magic_damage_to_champions: self.stat(&[
                "MAGIC_DAMAGE_DEALT_TO_CHAMPIONS",
                "magicDamageDealtToChampions",
            ]),
            true_damage_to_champions: self.stat(&[
                "TRUE_DAMAGE_DEALT_TO_CHAMPIONS",
                "trueDamageDealtToChampions",
            ]),
            damage_dealt: self.stat(&["TOTAL_DAMAGE_DEALT", "totalDamageDealt"]),
            physical_damage_dealt: self
                .stat(&["PHYSICAL_DAMAGE_DEALT_PLAYER", "physicalDamageDealt"]),
            magic_damage_dealt: self.stat(&["MAGIC_DAMAGE_DEALT_PLAYER", "magicDamageDealt"]),
            true_damage_dealt: self.stat(&["TRUE_DAMAGE_DEALT_PLAYER", "trueDamageDealt"]),
            damage_taken: self.stat(&["TOTAL_DAMAGE_TAKEN", "totalDamageTaken"]),
            physical_damage_taken: self.stat(&["PHYSICAL_DAMAGE_TAKEN", "physicalDamageTaken"]),
            magic_damage_taken: self.stat(&["MAGIC_DAMAGE_TAKEN", "magicDamageTaken"]),
            true_damage_taken: self.stat(&["TRUE_DAMAGE_TAKEN", "trueDamageTaken"]),
            damage_self_mitigated: self
                .stat(&["TOTAL_DAMAGE_SELF_MITIGATED", "damageSelfMitigated"]),
            heal: self.stat(&["TOTAL_HEAL", "totalHeal"]),
            heal_on_teammates: self.stat(&["TOTAL_HEAL_ON_TEAMMATES", "totalHealsOnTeammates"]),
            shield_on_teammates: self.stat(&[
                "TOTAL_DAMAGE_SHIELDED_ON_TEAMMATES",
                "totalDamageShieldedOnTeammates",
            ]),
            time_ccing_others: self.stat(&["TIME_CCING_OTHERS", "timeCCingOthers"]),
            wards_placed: self.stat(&["WARD_PLACED", "wardsPlaced"]),
            wards_killed: self.stat(&["WARD_KILLED", "wardsKilled"]),
            control_wards_bought: self
                .stat(&["VISION_WARDS_BOUGHT_IN_GAME", "visionWardsBoughtInGame"]),
            gold_spent: self.stat(&["goldSpent"]),
            minions_killed: self.stat(&["MINIONS_KILLED", "totalMinionsKilled"]),
            neutral_minions_killed: self.stat(&["NEUTRAL_MINIONS_KILLED", "neutralMinionsKilled"]),
            ally_jungle_minions_killed: self.stat(&[
                "NEUTRAL_MINIONS_KILLED_YOUR_JUNGLE",
                "totalAllyJungleMinionsKilled",
            ]),
            enemy_jungle_minions_killed: self.stat(&[
                "NEUTRAL_MINIONS_KILLED_ENEMY_JUNGLE",
                "totalEnemyJungleMinionsKilled",
            ]),
            turret_kills: self.stat(&["TURRETS_KILLED", "turretKills"]),
            turret_takedowns: self.stat(&["turretTakedowns"]),
            inhibitor_kills: self.stat(&["BARRACKS_KILLED", "inhibitorKills"]),
            damage_to_turrets: self
                .stat(&["TOTAL_DAMAGE_DEALT_TO_TURRETS", "damageDealtToTurrets"]),
            damage_to_objectives: self.stat(&[
                "TOTAL_DAMAGE_DEALT_TO_OBJECTIVES",
                "damageDealtToObjectives",
            ]),
            damage_to_epic_monsters: self.stat(&["damageDealtToEpicMonsters"]),
            dragon_kills: self.stat(&["dragonKills"]),
            baron_kills: self.stat(&["baronKills"]),
            objectives_stolen: self.stat(&["objectivesStolen"]),
            longest_time_living_secs: self.stat(&["longestTimeSpentLiving"]),
            time_dead_secs: self.stat(&["totalTimeSpentDead"]),
            spell1_casts: self.stat(&["spell1Casts"]),
            spell2_casts: self.stat(&["spell2Casts"]),
            spell3_casts: self.stat(&["spell3Casts"]),
            spell4_casts: self.stat(&["spell4Casts"]),
            summoner1_casts: self.stat(&["summoner1Casts"]),
            summoner2_casts: self.stat(&["summoner2Casts"]),
            all_in_pings: self.stat(&["allInPings"]),
            assist_me_pings: self.stat(&["assistMePings"]),
            basic_pings: self.stat(&["basicPings"]),
            command_pings: self.stat(&["commandPings"]),
            danger_pings: self.stat(&["dangerPings"]),
            enemy_missing_pings: self.stat(&["enemyMissingPings"]),
            enemy_vision_pings: self.stat(&["enemyVisionPings"]),
            get_back_pings: self.stat(&["getBackPings"]),
            hold_pings: self.stat(&["holdPings"]),
            need_vision_pings: self.stat(&["needVisionPings"]),
            on_my_way_pings: self.stat(&["onMyWayPings"]),
            push_pings: self.stat(&["pushPings"]),
            retreat_pings: self.stat(&["retreatPings"]),
            vision_cleared_pings: self.stat(&["visionClearedPings"]),
            perks: (0..6)
                .map(|i| PerkStats {
                    id: self.stat(&[&format!("PERK{i}"), &format!("perk{i}")]),
                    vars: [1, 2, 3].map(|v| {
                        self.stat(&[&format!("PERK{i}_VAR{v}"), &format!("perk{i}Var{v}")])
                    }),
                })
                .filter(|p| p.id != 0)
                .collect(),
        }
    }
}

impl GameResult {
    /// Lê o bloco de estatísticas (o `data` de um evento ou a resposta do
    /// REST). `None` sem partida (`gameId` 0) ou sem nenhum jogador, como
    /// numa partida encerrada com erro antes de a LCU montar o bloco.
    ///
    /// A fila (`queueId` e nome) e a mudança de PDL ficam vazias; veja
    /// [`queue_from_session`] e [`ranked_change`].
    pub fn from_json(value: &Value) -> Option<Self> {
        let raw = RawBlock::deserialize(value).ok()?;
        if raw.game_id == 0 || raw.teams.iter().all(|t| t.players.is_empty()) {
            return None;
        }
        let local_puuid = raw
            .teams
            .iter()
            .flat_map(|t| &t.players)
            .find(|p| p.is_local_player)
            .map(|p| p.puuid.clone())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| raw.local_player.puuid.clone());
        let local_team = raw
            .teams
            .iter()
            .find(|t| t.is_player_team)
            .or_else(|| {
                raw.teams
                    .iter()
                    .find(|t| t.players.iter().any(|p| p.is_local_player))
            })
            .map(|t| t.team_id)
            .unwrap_or(raw.local_player.team_id);

        let mut teams: Vec<GameResultTeam> = raw
            .teams
            .iter()
            .filter(|t| !t.players.is_empty())
            .map(|t| GameResultTeam {
                team_id: t.team_id,
                player_team: t.team_id == local_team,
                win: t.is_winning_team,
                players: t
                    .players
                    .iter()
                    .map(|p| p.to_player(t.team_id, t.is_winning_team))
                    .collect(),
                advanced: t.players.iter().map(RawPlayer::to_advanced).collect(),
            })
            .collect();
        // Estável: o time do jogador primeiro, os outros na ordem da LCU.
        teams.sort_by_key(|t| !t.player_team);

        let result = if raw.game_ended_in_early_surrender || raw.team_early_surrendered {
            MatchResult::Remake
        } else if teams.iter().any(|t| t.player_team && t.win) {
            MatchResult::Win
        } else {
            MatchResult::Loss
        };

        Some(GameResult {
            game_id: raw.game_id,
            queue_id: queue_id_from_type(&raw.queue_type),
            queue_name: String::new(),
            game_mode: raw.game_mode,
            ranked: raw.ranked,
            duration_secs: raw.game_length,
            result,
            local_puuid,
            teams,
            ranked_change: None,
        })
    }

    /// A fila ranqueada da partida, pelo `queueId`.
    pub fn ranked_queue(&self) -> Option<RankedQueue> {
        match self.queue_id {
            SOLO_DUO_QUEUE_ID => Some(RankedQueue::SoloDuo),
            FLEX_QUEUE_ID => Some(RankedQueue::Flex),
            _ => None,
        }
    }
}

/// `queueId` das ranqueadas pelo `queueType` do bloco, `0` nas outras (a
/// fila certa vem depois, da sessão do gameflow).
fn queue_id_from_type(queue_type: &str) -> u32 {
    match queue_type {
        SOLO_DUO => SOLO_DUO_QUEUE_ID,
        FLEX => FLEX_QUEUE_ID,
        _ => 0,
    }
}

/// A fila da sessão do gameflow: `(queueId, nome)`. `None` sem fila
/// (`queue.id` ausente ou negativo).
pub fn queue_from_session(value: &Value) -> Option<(u32, String)> {
    let queue = value.get("gameData")?.get("queue")?;
    let id = u32::try_from(queue.get("id")?.as_i64()?).ok()?;
    if id == 0 {
        return None;
    }
    let text = |key: &str| {
        queue
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let description = text("description");
    let name = if description.is_empty() {
        text("name")
    } else {
        description
    };
    Some((id, name))
}

/// A mudança de rank na fila `queue`, comparando os ranks do começo da
/// partida (`before`) com os atuais (`after`). `None` enquanto os ranks não
/// mudaram (a LCU ainda não mandou os novos).
pub fn ranked_change(
    queue: RankedQueue,
    before: &RankedStats,
    after: &RankedStats,
) -> Option<RankedChange> {
    let pick = |stats: &RankedStats| match queue {
        RankedQueue::SoloDuo => stats.solo_duo.clone(),
        RankedQueue::Flex => stats.flex.clone(),
    };
    let (before, after) = (pick(before), pick(after));
    if before == after {
        return None;
    }
    let same_rank = before.tier == after.tier && before.division == after.division;
    let league_points_delta = same_rank
        .then(|| i64::from(after.league_points) - i64::from(before.league_points))
        .and_then(|d| i32::try_from(d).ok());
    Some(RankedChange {
        queue,
        before,
        after,
        league_points_delta,
    })
}

/// O bloco atual (`GET .../eog-stats-block`). `Ok(None)` fora do pós-jogo
/// (404) ou com um bloco sem partida.
pub async fn current(client: &LcuClient) -> Result<Option<GameResult>, LcuError> {
    match client.get::<Value>(EOG_STATS_PATH).await {
        Ok(value) => Ok(GameResult::from_json(&value)),
        Err(LcuError::Status { status: 404, .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Sai do pós-jogo para o início, sem voltar ao lobby (#180). Primeiro
/// recusa o "Jogar de novo" do grupo (`POST /lol-lobby/v2/play-again-decline`),
/// para os outros jogadores do grupo saberem; a LCU pode recusar esse passo
/// (sem grupo, por exemplo) e o erro dele é ignorado. Depois dispensa as
/// estatísticas (`POST /lol-end-of-game/v1/state/dismiss-stats`), e a fase vai
/// para `None` pelo WAMP. No pós-jogo não há lobby: a LCU apaga o
/// `/lol-lobby/v2/lobby` quando a partida começa (gravação `16.22-pos-jogo`),
/// então sair do lobby não serve aqui.
pub async fn leave(client: &LcuClient) -> Result<(), LcuError> {
    let _ = client
        .send::<Option<Value>>(
            reqwest::Method::POST,
            crate::lobby::PLAY_AGAIN_DECLINE_PATH,
            None,
        )
        .await;
    client
        .send::<Option<Value>>(reqwest::Method::POST, DISMISS_STATS_PATH, None)
        .await
        .map(|_| ())
}

/// A fila da partida, pela sessão do gameflow.
pub async fn current_queue(client: &LcuClient) -> Result<Option<(u32, String)>, LcuError> {
    let value: Value = client.get(GAMEFLOW_SESSION_PATH).await?;
    Ok(queue_from_session(&value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// O bloco e a sessão gravados em `16.22-pos-jogo` (#177), na fase
    /// `phase`.
    fn gravado(path: &str, phase: &str) -> Value {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/16.22-pos-jogo/rest.json");
        let rest: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        rest.into_iter()
            .find(|r| r["path"] == path && r["phase"] == phase && r["status"] == 200)
            .unwrap()["body"]
            .clone()
    }

    #[test]
    fn le_o_bloco_gravado() {
        let result = GameResult::from_json(&gravado(EOG_STATS_PATH, "EndOfGame")).unwrap();
        assert_eq!(result.game_id, 1124);
        assert_eq!(result.queue_id, 420);
        assert_eq!(result.game_mode, "CLASSIC");
        assert!(result.ranked);
        assert_eq!(result.duration_secs, 1551);
        assert_eq!(result.result, MatchResult::Win);
        assert_eq!(result.local_puuid, "fake-puuid-0001");
        assert_eq!(result.ranked_change, None);

        assert_eq!(result.teams.len(), 2);
        let mine = &result.teams[0];
        assert!(mine.player_team && mine.win);
        assert_eq!(mine.team_id, 200);
        assert_eq!(mine.players.len(), 5);
        let other = &result.teams[1];
        assert!(!other.player_team && !other.win);
        assert_eq!(other.team_id, 100);
        assert_eq!(other.players.len(), 5);

        let ksante = &mine.players[0];
        assert_eq!(ksante.champion_id, 897);
        assert_eq!(ksante.game_name, "fake-name-0002");
        assert_eq!(ksante.tag_line, "fake-tag-0001");
        assert_eq!(ksante.team_id, 200);
        assert!(ksante.win);
        assert_eq!((ksante.kills, ksante.deaths, ksante.assists), (3, 4, 8));
        assert_eq!(ksante.champion_level, 15);
        assert_eq!(ksante.cs, 121);
        assert_eq!(ksante.gold, 9127);
        assert_eq!(ksante.damage_to_champions, 14268);
        assert_eq!(ksante.vision_score, 16);
        assert_eq!((ksante.spell1_id, ksante.spell2_id), (4, 12));
        assert_eq!(ksante.items, [6665, 6662, 3111, 1120, 2022, 0, 3340]);
        assert_eq!(ksante.perk_id, 8437);
        assert_eq!(ksante.perk_style_id, 8400);
        assert_eq!(ksante.perk_sub_style_id, 8000);

        // Detalhes avançados (#586), na ordem dos jogadores.
        assert_eq!(mine.advanced.len(), 5);
        let advanced = &mine.advanced[0];
        assert_eq!(advanced.physical_damage_to_champions, 12604);
        assert_eq!(advanced.magic_damage_to_champions, 1116);
        assert_eq!(advanced.true_damage_to_champions, 547);
        assert_eq!(advanced.damage_taken, 25107);
        assert_eq!(advanced.damage_self_mitigated, 32692);
        assert_eq!(advanced.shield_on_teammates, 771);
        assert_eq!(advanced.wards_placed, 5);
        assert_eq!(advanced.gold_spent, 8200);
        assert_eq!(advanced.minions_killed, 121);
        assert_eq!(advanced.turret_kills, 4);
        assert_eq!(advanced.longest_time_living_secs, 529);
        assert_eq!(advanced.time_dead_secs, 140);
        assert_eq!(advanced.spell1_casts, 158);
        assert_eq!(advanced.on_my_way_pings, 5);
        assert_eq!(mine.advanced[1].dragon_kills, 4);
        assert_eq!(advanced.perks.len(), 6);
        assert_eq!(
            advanced.perks[0],
            PerkStats {
                id: 8437,
                vars: [1116, 635, 0]
            }
        );
        assert_eq!(advanced.perks[5].id, 8299);

        let me = mine
            .players
            .iter()
            .find(|p| p.puuid == result.local_puuid)
            .unwrap();
        assert_eq!(me.champion_id, 432);
    }

    #[test]
    fn o_bloco_ja_vem_no_waiting_for_stats() {
        let result = GameResult::from_json(&gravado(EOG_STATS_PATH, "WaitingForStats")).unwrap();
        assert_eq!(result.game_id, 1124);
    }

    #[test]
    fn fila_da_sessao_gravada() {
        assert_eq!(
            queue_from_session(&gravado(GAMEFLOW_SESSION_PATH, "EndOfGame")),
            Some((420, "Ranqueada Solo/Duo".to_owned()))
        );
        assert_eq!(queue_from_session(&json!({})), None);
        assert_eq!(
            queue_from_session(&json!({"gameData": {"queue": {"id": -1}}})),
            None
        );
        assert_eq!(
            queue_from_session(&json!({"gameData": {"queue": {"id": 450, "name": "ARAM"}}})),
            Some((450, "ARAM".to_owned()))
        );
    }

    #[test]
    fn bloco_incompleto() {
        // Sem partida ou sem jogadores: nada para mostrar.
        assert_eq!(GameResult::from_json(&json!({})), None);
        assert_eq!(GameResult::from_json(&Value::Null), None);
        assert_eq!(
            GameResult::from_json(&json!({"gameId": 7, "teams": []})),
            None
        );
        assert_eq!(
            GameResult::from_json(&json!({"gameId": 7, "teams": [{"teamId": 100, "players": []}]})),
            None
        );

        // Um jogador só com o básico: o resto vem zerado, sem quebrar.
        let result = GameResult::from_json(&json!({
            "gameId": 7,
            "gameMode": "ARAM",
            "queueType": "",
            "teams": [
                {"teamId": 100, "isPlayerTeam": false, "isWinningTeam": true,
                 "players": [{"puuid": "fake-puuid-0002", "championId": 1,
                              "summonerName": "fake-name-0002", "items": [1001]}]},
                {"teamId": 200, "isPlayerTeam": true, "isWinningTeam": false,
                 "players": [{"puuid": "fake-puuid-0001", "isLocalPlayer": true,
                              "championId": 2, "stats": {"kills": 5, "deaths": 1}}]}
            ]
        }))
        .unwrap();
        assert_eq!(result.queue_id, 0);
        assert!(!result.ranked);
        assert_eq!(result.result, MatchResult::Loss);
        assert_eq!(result.local_puuid, "fake-puuid-0001");
        assert_eq!(result.teams[0].team_id, 200);
        let me = &result.teams[0].players[0];
        assert_eq!((me.kills, me.deaths, me.assists, me.gold), (5, 1, 0, 0));
        assert_eq!(me.items, [0; 7]);
        let other = &result.teams[1].players[0];
        // Sem Riot ID, vale o nome de invocador.
        assert_eq!(other.game_name, "fake-name-0002");
        assert_eq!(other.items, [1001, 0, 0, 0, 0, 0, 0]);
        assert!(other.win);
    }

    #[test]
    fn remake_nao_e_vitoria_nem_derrota() {
        let mut block = gravado(EOG_STATS_PATH, "EndOfGame");
        block["gameEndedInEarlySurrender"] = json!(true);
        let result = GameResult::from_json(&block).unwrap();
        assert_eq!(result.result, MatchResult::Remake);
    }

    #[test]
    fn derrota_pelo_time_do_jogador() {
        let mut block = gravado(EOG_STATS_PATH, "EndOfGame");
        for team in block["teams"].as_array_mut().unwrap() {
            let won = team["isWinningTeam"].as_bool().unwrap();
            team["isWinningTeam"] = json!(!won);
        }
        let result = GameResult::from_json(&block).unwrap();
        assert_eq!(result.result, MatchResult::Loss);
        assert!(result.teams[0].player_team && !result.teams[0].win);
    }

    fn entry(tier: &str, division: Option<&str>, lp: u32, wins: u32) -> RankedEntry {
        RankedEntry {
            tier: Some(tier.into()),
            division: division.map(Into::into),
            league_points: lp,
            wins,
            losses: 28,
            provisional: false,
        }
    }

    fn solo(entry: RankedEntry) -> RankedStats {
        RankedStats {
            solo_duo: entry,
            flex: RankedEntry::default(),
        }
    }

    #[test]
    fn pdl_ganhos_na_gravacao() {
        // De 804 para 868 PDL no Mestre, como na gravação.
        let before = solo(entry("MASTER", None, 804, 54));
        let after = solo(entry("MASTER", None, 868, 55));
        let change = ranked_change(RankedQueue::SoloDuo, &before, &after).unwrap();
        assert_eq!(change.league_points_delta, Some(64));
        assert_eq!(change.after.league_points, 868);
        // A outra fila não mudou.
        assert_eq!(ranked_change(RankedQueue::Flex, &before, &after), None);
    }

    #[test]
    fn pdl_perdidos_e_mudanca_de_divisao() {
        let before = solo(entry("GOLD", Some("II"), 20, 10));
        let lost = solo(entry("GOLD", Some("II"), 2, 10));
        assert_eq!(
            ranked_change(RankedQueue::SoloDuo, &before, &lost)
                .unwrap()
                .league_points_delta,
            Some(-18)
        );
        let promoted = solo(entry("GOLD", Some("I"), 0, 11));
        let change = ranked_change(RankedQueue::SoloDuo, &before, &promoted).unwrap();
        assert_eq!(change.league_points_delta, None);
        assert_eq!(change.after.division.as_deref(), Some("I"));
    }

    #[test]
    fn sem_mudanca_enquanto_os_ranks_nao_chegam() {
        let before = solo(entry("GOLD", Some("II"), 20, 10));
        assert_eq!(ranked_change(RankedQueue::SoloDuo, &before, &before), None);
    }

    #[test]
    fn fila_ranqueada_pelo_queue_id() {
        let mut result = GameResult::from_json(&gravado(EOG_STATS_PATH, "EndOfGame")).unwrap();
        assert_eq!(result.ranked_queue(), Some(RankedQueue::SoloDuo));
        result.queue_id = 440;
        assert_eq!(result.ranked_queue(), Some(RankedQueue::Flex));
        result.queue_id = 450;
        assert_eq!(result.ranked_queue(), None);
    }
}
