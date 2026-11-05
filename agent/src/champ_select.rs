//! Estado da seleção de campeões (#22): sessão, times, timer e ações
//! pendentes, num único [`ChampSelect`] que vai para a UI pelo `/ws`.
//!
//! A LCU guarda a sessão em `GET /lol-champ-select/v1/session` (404 fora da
//! seleção) e manda cada mudança pelo WAMP no mesmo URI: quem passou o mouse
//! num campeão, baniu, escolheu, trocou de feitiço, e o timer a cada troca de
//! vez. Quando a seleção acaba (partida abrindo, alguém saiu, dodge), chega
//! um `Delete`.
//!
//! Da sessão o agente guarda só o que a tela de seleção usa:
//!
//! - os dois times, com cargo, campeão escolhido, campeão em hover e
//!   feitiços de cada um;
//! - o timer da etapa (`PLANNING`, `BAN_PICK`, `FINALIZATION`,
//!   `GAME_STARTING`), com a hora em que ela acaba;
//! - as ações (bans e picks) na ordem do draft e a ação que é a vez do
//!   jogador agora ([`ChampSelect::pending_action`]);
//! - os campeões banidos de cada time.
//!
//! Trocas, banco do ARAM e skins ficam para as issues que as usam (#25...).
//!
//! As ações do jogador (#23) usam o `id` de uma ação da sessão:
//!
//! - [`hover`]: `PATCH /lol-champ-select/v1/session/actions/{id}` com o
//!   campeão. Na vez do jogador, marca o campeão sem confirmar; fora da vez
//!   (etapa `PLANNING`, ou durante os bans), a LCU guarda o campeão do pick
//!   como intenção (`championPickIntent`), que o time vê.
//! - [`confirm`]: o mesmo `PATCH` e depois
//!   `POST /lol-champ-select/v1/session/actions/{id}/complete`, que confirma
//!   o ban ou o pick.
//!
//! [`check_action`] confere o pedido contra a sessão atual antes de falar
//! com a LCU, com as regras do client oficial.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;

use serde_json::json;

use crate::lobby::{Position, Queue};
use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) da sessão de seleção de campeões.
pub const CHAMP_SELECT_SESSION_PATH: &str = "/lol-champ-select/v1/session";

/// A seleção de campeões em andamento, como a tela precisa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelect {
    /// Id da sessão na LCU. Muda a cada seleção (por exemplo, depois de um
    /// dodge e de uma partida nova).
    pub id: String,
    /// `queueId` da Riot, `0` quando a LCU não manda.
    pub queue_id: u32,
    /// A fila, ou `null` quando não é uma das [`Queue`] do easylobby.
    pub queue: Option<Queue>,
    /// Partida personalizada.
    pub is_custom_game: bool,
    /// `cellId` do jogador logado, para achar ele em `myTeam` e nas ações.
    pub local_player_cell_id: i32,
    /// Bans ao mesmo tempo (ranqueada): todos os bans do time ficam em
    /// andamento juntos, e os do outro time só aparecem no fim (ação
    /// `ten_bans_reveal`).
    pub has_simultaneous_bans: bool,
    /// Picks ao mesmo tempo (escolha às cegas).
    pub has_simultaneous_picks: bool,
    /// Etapa atual e quanto falta para ela acabar.
    pub timer: ChampSelectTimer,
    /// O time do jogador, na ordem das células (o jogador logado incluído).
    pub my_team: Vec<ChampSelectPlayer>,
    /// O time adversário. Nome, cargo e feitiços vêm vazios, como no client
    /// oficial; o campeão só aparece depois do pick.
    pub their_team: Vec<ChampSelectPlayer>,
    /// Campeões banidos até agora.
    pub bans: ChampSelectBans,
    /// Todas as ações do draft na ordem em que acontecem, as já feitas
    /// incluídas. Ações do mesmo `turn` rodam ao mesmo tempo.
    pub actions: Vec<ChampSelectAction>,
    /// A ação que é a vez do jogador logado agora (ban ou pick em andamento
    /// e ainda não confirmado), ou `null` se não é a vez dele. O `id` dela
    /// é o que os comandos `hoverChampion` e `confirmAction` (#23) usam.
    pub pending_action: Option<ChampSelectAction>,
}

/// Etapa atual da seleção e o tempo dela.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectTimer {
    pub phase: ChampSelectPhase,
    /// Milissegundos que faltavam para a etapa acabar quando o agente
    /// recebeu a sessão.
    pub time_left_ms: u32,
    /// Duração total da etapa, em milissegundos.
    pub total_time_ms: u32,
    /// Etapa sem limite de tempo (algumas personalizadas). Aí `timeLeftMs` e
    /// `endsAtMs` não valem.
    pub is_infinite: bool,
    /// Quando a etapa acaba, em milissegundos desde 1970 pelo relógio do
    /// computador (o mesmo da UI). A contagem regressiva é
    /// `Math.max(0, endsAtMs - Date.now())`; assim ela fica certa mesmo para
    /// uma UI que conectou depois, entre um evento e outro da LCU.
    // Cabe num `number` do JavaScript (bem menor que 2^53).
    #[ts(type = "number")]
    pub ends_at_ms: u64,
}

/// Etapa da seleção, com os nomes da LCU.
// `Deserialize` à mão para aceitar valores novos como `UNKNOWN`, como em
// `GameflowPhase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChampSelectPhase {
    /// Antes dos bans: cada um mostra o campeão que pretende jogar.
    Planning,
    /// Bans e picks.
    BanPick,
    /// Todos escolheram: tempo para trocar runas, feitiços e skin.
    Finalization,
    /// A seleção acabou e a partida está abrindo.
    GameStarting,
    /// Etapa que o agente não conhece.
    Unknown,
}

impl ChampSelectPhase {
    pub const ALL: [ChampSelectPhase; 5] = [
        ChampSelectPhase::Planning,
        ChampSelectPhase::BanPick,
        ChampSelectPhase::Finalization,
        ChampSelectPhase::GameStarting,
        ChampSelectPhase::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ChampSelectPhase::Planning => "PLANNING",
            ChampSelectPhase::BanPick => "BAN_PICK",
            ChampSelectPhase::Finalization => "FINALIZATION",
            ChampSelectPhase::GameStarting => "GAME_STARTING",
            ChampSelectPhase::Unknown => "UNKNOWN",
        }
    }

    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|p| p.as_str() == name)
            .unwrap_or(ChampSelectPhase::Unknown)
    }
}

impl<'de> Deserialize<'de> for ChampSelectPhase {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::from_name(&name))
    }
}

impl fmt::Display for ChampSelectPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Um jogador na seleção.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectPlayer {
    /// Posição do jogador na seleção (0 a 4 num time, 5 a 9 no outro). As
    /// ações apontam para ela em `actorCellId`.
    pub cell_id: i32,
    /// Se é o jogador logado.
    pub is_local_player: bool,
    /// Riot ID. `null` quando a LCU esconde o nome (ranqueada, time
    /// adversário).
    pub game_name: Option<String>,
    pub tag_line: Option<String>,
    /// Cargo recebido na fila, ou `unselected` nas filas sem cargo e no time
    /// adversário.
    pub assigned_position: Position,
    /// Campeão escolhido (ou em hover na vez do jogador), ou `null`.
    pub champion_id: Option<u32>,
    /// Campeão que o jogador pretende jogar, mostrado antes da vez dele
    /// (etapa `PLANNING` e hover fora da vez), ou `null`.
    pub champion_pick_intent: Option<u32>,
    /// Skin escolhida, ou `null`.
    pub selected_skin_id: Option<u32>,
    /// Feitiços de invocador, ou `null` quando escondidos.
    pub spell1_id: Option<u32>,
    pub spell2_id: Option<u32>,
    /// `puuid` do jogador, vazio quando a LCU esconde o nome, e o
    /// `obfuscatedPuuid` que ela manda no lugar. Servem para ligar as
    /// mensagens do chat da seleção (#27) a quem mandou. Não vão para a UI.
    #[serde(skip)]
    #[ts(skip)]
    pub puuid: String,
    #[serde(skip)]
    #[ts(skip)]
    pub obfuscated_puuid: String,
}

/// Campeões banidos de cada time.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectBans {
    pub my_team: Vec<u32>,
    pub their_team: Vec<u32>,
}

/// Uma ação do draft: um ban, um pick ou a revelação dos bans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectAction {
    /// Id da ação na LCU.
    pub id: u32,
    #[serde(rename = "type")]
    pub action_type: ChampSelectActionType,
    /// `cellId` de quem faz a ação; `-1` nas ações sem jogador
    /// (`ten_bans_reveal`).
    pub actor_cell_id: i32,
    /// Campeão da ação (em hover, banido ou escolhido), ou `null`.
    pub champion_id: Option<u32>,
    /// Ação confirmada.
    pub completed: bool,
    /// É a vez desta ação.
    pub is_in_progress: bool,
    /// Ação do time do jogador.
    pub is_ally_action: bool,
    /// Ordem da ação no draft, a partir de 0. Ações do mesmo turno rodam ao
    /// mesmo tempo (os bans simultâneos, os picks em dupla).
    pub turn: u32,
}

/// Tipo de ação, com os nomes da LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ChampSelectActionType {
    Ban,
    Pick,
    /// Revela os bans do time adversário (bans simultâneos).
    TenBansReveal,
    /// Tipo que o agente não conhece.
    Unknown,
}

impl ChampSelectActionType {
    pub const ALL: [ChampSelectActionType; 4] = [
        ChampSelectActionType::Ban,
        ChampSelectActionType::Pick,
        ChampSelectActionType::TenBansReveal,
        ChampSelectActionType::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ChampSelectActionType::Ban => "ban",
            ChampSelectActionType::Pick => "pick",
            ChampSelectActionType::TenBansReveal => "ten_bans_reveal",
            ChampSelectActionType::Unknown => "unknown",
        }
    }

    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|t| t.as_str() == name)
            .unwrap_or(ChampSelectActionType::Unknown)
    }
}

impl<'de> Deserialize<'de> for ChampSelectActionType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::from_name(&name))
    }
}

impl fmt::Display for ChampSelectActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ChampSelect {
    /// Lê a sessão do JSON da LCU (resposta do `GET` ou `data` de um evento),
    /// com o timer contado a partir de agora. `None` se não for uma sessão.
    pub fn from_json(value: &Value) -> Option<ChampSelect> {
        Self::from_json_at(value, now_ms())
    }

    /// Como [`ChampSelect::from_json`], com a hora em que a sessão chegou
    /// (milissegundos desde 1970).
    pub fn from_json_at(value: &Value, received_at_ms: u64) -> Option<ChampSelect> {
        let raw = RawSession::deserialize(value).ok()?;
        let local = raw.local_player_cell_id;

        let actions: Vec<ChampSelectAction> = raw
            .actions
            .into_iter()
            .enumerate()
            .flat_map(|(turn, group)| {
                let turn = u32::try_from(turn).unwrap_or(u32::MAX);
                group.into_iter().map(move |a| ChampSelectAction {
                    id: a.id,
                    action_type: a.action_type,
                    actor_cell_id: a.actor_cell_id,
                    champion_id: champion(a.champion_id),
                    completed: a.completed,
                    is_in_progress: a.is_in_progress,
                    is_ally_action: a.is_ally_action,
                    turn,
                })
            })
            .collect();

        let pending_action = actions
            .iter()
            .find(|a| {
                a.actor_cell_id == local
                    && a.is_in_progress
                    && !a.completed
                    && matches!(
                        a.action_type,
                        ChampSelectActionType::Ban | ChampSelectActionType::Pick
                    )
            })
            .cloned();

        let mut bans = ChampSelectBans::default();
        let banned = actions
            .iter()
            .filter(|a| a.action_type == ChampSelectActionType::Ban && a.completed);
        for action in banned {
            if let Some(id) = action.champion_id {
                if action.is_ally_action {
                    bans.my_team.push(id);
                } else {
                    bans.their_team.push(id);
                }
            }
        }
        // Em alguns modos os bans vêm só neste campo, sem ações.
        for (list, extra) in [
            (&mut bans.my_team, raw.bans.my_team_bans),
            (&mut bans.their_team, raw.bans.their_team_bans),
        ] {
            for id in extra.into_iter().filter_map(champion) {
                if !list.contains(&id) {
                    list.push(id);
                }
            }
        }

        let player = |p: RawPlayer| ChampSelectPlayer {
            cell_id: p.cell_id,
            is_local_player: p.cell_id == local,
            game_name: non_empty(p.game_name),
            tag_line: non_empty(p.tag_line),
            assigned_position: assigned_position(&p.assigned_position),
            champion_id: champion(p.champion_id),
            champion_pick_intent: champion(p.champion_pick_intent),
            selected_skin_id: champion(p.selected_skin_id),
            spell1_id: champion(p.spell1_id),
            spell2_id: champion(p.spell2_id),
            puuid: p.puuid,
            obfuscated_puuid: p.obfuscated_puuid,
        };

        let time_left_ms = clamp_ms(raw.timer.adjusted_time_left_in_phase);
        Some(ChampSelect {
            id: raw.id,
            queue_id: raw.queue_id,
            queue: Queue::from_id(raw.queue_id),
            is_custom_game: raw.is_custom_game,
            local_player_cell_id: local,
            has_simultaneous_bans: raw.has_simultaneous_bans,
            has_simultaneous_picks: raw.has_simultaneous_picks,
            timer: ChampSelectTimer {
                phase: raw.timer.phase,
                time_left_ms,
                total_time_ms: clamp_ms(raw.timer.total_time_in_phase),
                is_infinite: raw.timer.is_infinite,
                ends_at_ms: received_at_ms.saturating_add(u64::from(time_left_ms)),
            },
            my_team: raw.my_team.into_iter().map(player).collect(),
            their_team: raw.their_team.into_iter().map(player).collect(),
            bans,
            actions,
            pending_action,
        })
    }

    /// O jogador logado em `myTeam`, se estiver lá.
    pub fn local_player(&self) -> Option<&ChampSelectPlayer> {
        self.my_team.iter().find(|p| p.is_local_player)
    }
}

/// A sessão como a LCU manda, só com os campos usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSession {
    #[serde(default)]
    id: String,
    #[serde(default)]
    queue_id: u32,
    #[serde(default)]
    is_custom_game: bool,
    local_player_cell_id: i32,
    #[serde(default)]
    has_simultaneous_bans: bool,
    #[serde(default)]
    has_simultaneous_picks: bool,
    timer: RawTimer,
    my_team: Vec<RawPlayer>,
    #[serde(default)]
    their_team: Vec<RawPlayer>,
    #[serde(default)]
    actions: Vec<Vec<RawAction>>,
    #[serde(default)]
    bans: RawBans,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTimer {
    phase: ChampSelectPhase,
    #[serde(default)]
    adjusted_time_left_in_phase: f64,
    #[serde(default)]
    total_time_in_phase: f64,
    #[serde(default)]
    is_infinite: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlayer {
    cell_id: i32,
    #[serde(default)]
    game_name: String,
    #[serde(default)]
    tag_line: String,
    #[serde(default)]
    assigned_position: String,
    #[serde(default)]
    champion_id: i64,
    #[serde(default)]
    champion_pick_intent: i64,
    #[serde(default)]
    selected_skin_id: i64,
    #[serde(default)]
    spell1_id: i64,
    #[serde(default)]
    spell2_id: i64,
    #[serde(default)]
    puuid: String,
    #[serde(default)]
    obfuscated_puuid: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAction {
    id: u32,
    #[serde(rename = "type")]
    action_type: ChampSelectActionType,
    #[serde(default = "no_actor")]
    actor_cell_id: i32,
    #[serde(default)]
    champion_id: i64,
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    is_in_progress: bool,
    #[serde(default)]
    is_ally_action: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawBans {
    #[serde(default)]
    my_team_bans: Vec<i64>,
    #[serde(default)]
    their_team_bans: Vec<i64>,
}

fn no_actor() -> i32 {
    -1
}

/// Ids de campeão, skin e feitiço: a LCU usa `0` (ou negativo) para
/// "nenhum".
fn champion(id: i64) -> Option<u32> {
    u32::try_from(id).ok().filter(|&id| id > 0)
}

fn non_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

/// Na seleção a LCU manda o cargo em minúsculas (`"middle"`), diferente do
/// lobby (`"MIDDLE"`).
fn assigned_position(name: &str) -> Position {
    Position::ALL
        .into_iter()
        .find(|p| p.lcu_name().eq_ignore_ascii_case(name))
        .unwrap_or_default()
}

fn clamp_ms(ms: f64) -> u32 {
    if ms.is_finite() && ms > 0.0 {
        ms.min(f64::from(u32::MAX)) as u32
    } else {
        0
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Busca a sessão de seleção atual. Fora da seleção a LCU responde `404`,
/// que volta como `Ok(None)`.
pub async fn current_session(lcu: &LcuClient) -> Result<Option<ChampSelect>, LcuError> {
    match lcu.get::<Value>(CHAMP_SELECT_SESSION_PATH).await {
        Ok(body) => Ok(ChampSelect::from_json(&body)),
        Err(error) if error.status() == Some(404) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Caminho de uma ação da sessão na LCU.
pub fn action_path(action_id: u32) -> String {
    format!("{CHAMP_SELECT_SESSION_PATH}/actions/{action_id}")
}

/// Caminho que confirma uma ação da sessão.
pub fn complete_action_path(action_id: u32) -> String {
    format!("{}/complete", action_path(action_id))
}

/// O que o jogador quer fazer com uma ação: só marcar o campeão ou
/// confirmar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionIntent {
    /// Marcar o campeão sem confirmar ([`hover`]).
    Hover,
    /// Confirmar o ban ou o pick ([`confirm`]).
    Confirm,
}

/// Por que um pedido de hover, ban ou pick não vale. Nada foi mandado para a
/// LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidAction {
    /// Não há seleção de campeões aberta.
    NoChampSelect,
    /// `championId` vazio (`0`).
    NoChampion,
    /// Não há ação com esse `id` na sessão.
    UnknownAction,
    /// A ação é de outro jogador.
    NotYourAction,
    /// A ação não é um ban nem um pick.
    NotBanOrPick,
    /// A ação já foi confirmada.
    AlreadyCompleted,
    /// Não é a vez dessa ação. Fora da vez só vale marcar o pick (intenção).
    NotYourTurn,
}

impl fmt::Display for InvalidAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            InvalidAction::NoChampSelect => "não há seleção de campeões aberta",
            InvalidAction::NoChampion => "escolha um campeão",
            InvalidAction::UnknownAction => "essa ação não existe na seleção atual",
            InvalidAction::NotYourAction => "essa ação é de outro jogador",
            InvalidAction::NotBanOrPick => "essa ação não é um ban nem um pick",
            InvalidAction::AlreadyCompleted => "essa ação já foi confirmada",
            InvalidAction::NotYourTurn => "ainda não é a vez dessa ação",
        })
    }
}

impl std::error::Error for InvalidAction {}

/// Confere um pedido de hover ou de confirmação contra a sessão atual, com
/// as regras do client oficial:
///
/// - a ação existe, é do jogador logado, é um ban ou um pick e não foi
///   confirmada;
/// - confirmar só na vez da ação (`isInProgress`);
/// - marcar um ban também só na vez dele; marcar um pick vale antes da vez,
///   como intenção.
pub fn check_action(
    champ_select: Option<&ChampSelect>,
    action_id: u32,
    champion_id: u32,
    intent: ActionIntent,
) -> Result<(), InvalidAction> {
    let cs = champ_select.ok_or(InvalidAction::NoChampSelect)?;
    if champion_id == 0 {
        return Err(InvalidAction::NoChampion);
    }
    let action = cs
        .actions
        .iter()
        .find(|a| a.id == action_id)
        .ok_or(InvalidAction::UnknownAction)?;
    if action.actor_cell_id != cs.local_player_cell_id {
        return Err(InvalidAction::NotYourAction);
    }
    if !matches!(
        action.action_type,
        ChampSelectActionType::Ban | ChampSelectActionType::Pick
    ) {
        return Err(InvalidAction::NotBanOrPick);
    }
    if action.completed {
        return Err(InvalidAction::AlreadyCompleted);
    }
    let early_pick_intent =
        intent == ActionIntent::Hover && action.action_type == ChampSelectActionType::Pick;
    if !action.is_in_progress && !early_pick_intent {
        return Err(InvalidAction::NotYourTurn);
    }
    Ok(())
}

/// Marca o campeão `champion_id` na ação `action_id`, sem confirmar. A LCU
/// responde sem corpo; a sessão nova chega pelo WAMP (ou por
/// [`current_session`]).
pub async fn hover(lcu: &LcuClient, action_id: u32, champion_id: u32) -> Result<(), LcuError> {
    lcu.patch::<_, Option<Value>>(
        &action_path(action_id),
        &json!({ "championId": champion_id }),
    )
    .await
    .map(|_| ())
}

/// Confirma o ban ou o pick de `champion_id` na ação `action_id`: marca o
/// campeão e confirma, como o botão do client oficial.
pub async fn confirm(lcu: &LcuClient, action_id: u32, champion_id: u32) -> Result<(), LcuError> {
    hover(lcu, action_id, champion_id).await?;
    lcu.post::<_, Option<Value>>(&complete_action_path(action_id), &json!({}))
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(cell: i32, champion: i64, intent: i64, position: &str, name: &str) -> Value {
        json!({
            "assignedPosition": position,
            "cellId": cell,
            "championId": champion,
            "championPickIntent": intent,
            "gameName": name,
            "tagLine": if name.is_empty() { "" } else { "BR1" },
            "nameVisibilityType": if name.is_empty() { "HIDDEN" } else { "UNHIDDEN" },
            "selectedSkinId": champion * 1000,
            "spell1Id": if position.is_empty() { 0 } else { 4 },
            "spell2Id": if position.is_empty() { 0 } else { 14 },
            "team": if cell < 5 { 1 } else { 2 },
        })
    }

    fn action(id: u32, kind: &str, actor: i32, champion: i64, done: bool, now: bool) -> Value {
        json!({
            "actorCellId": actor,
            "championId": champion,
            "completed": done,
            "duration": 0,
            "id": id,
            "isAllyAction": (0..5).contains(&actor),
            "isInProgress": now,
            "pickTurn": 0,
            "type": kind,
        })
    }

    /// Sessão de ranqueada no meio dos picks, no formato da LCU (16.22).
    fn session() -> Value {
        json!({
            "id": "sessao-1",
            "gameId": 1026,
            "queueId": 420,
            "isCustomGame": false,
            "localPlayerCellId": 1,
            "hasSimultaneousBans": true,
            "hasSimultaneousPicks": false,
            "timer": {
                "adjustedTimeLeftInPhase": 21637,
                "internalNowInEpochMs": 1793841334474u64,
                "isInfinite": false,
                "phase": "BAN_PICK",
                "totalTimeInPhase": 25000,
            },
            "bans": {"myTeamBans": [], "numBans": 0, "theirTeamBans": []},
            "myTeam": [
                player(0, 145, 0, "bottom", ""),
                player(1, 0, 121, "jungle", "Jogador"),
            ],
            "theirTeam": [player(5, 238, 0, "", ""), player(6, 0, 0, "", "")],
            "actions": [
                [
                    action(0, "ban", 0, 117, true, false),
                    action(1, "ban", 1, 43, true, false),
                    action(5, "ban", 5, 245, true, false),
                    action(6, "ban", 6, 0, true, false),
                ],
                [action(100, "ten_bans_reveal", -1, 0, true, false)],
                [action(10, "pick", 0, 145, true, false)],
                [action(11, "pick", 5, 238, true, false)],
                [action(13, "pick", 1, 121, false, true)],
                [action(15, "pick", 6, 0, false, false)],
            ],
            "trades": [],
            "pickOrderSwaps": [],
        })
    }

    #[test]
    fn le_a_sessao_da_lcu() {
        let cs = ChampSelect::from_json_at(&session(), 1_000).unwrap();
        assert_eq!(cs.id, "sessao-1");
        assert_eq!(cs.queue_id, 420);
        assert_eq!(cs.queue, Some(Queue::RankedSolo));
        assert_eq!(cs.local_player_cell_id, 1);
        assert!(cs.has_simultaneous_bans);
        assert!(!cs.has_simultaneous_picks);

        assert_eq!(cs.timer.phase, ChampSelectPhase::BanPick);
        assert_eq!(cs.timer.time_left_ms, 21_637);
        assert_eq!(cs.timer.total_time_ms, 25_000);
        assert_eq!(cs.timer.ends_at_ms, 22_637);
        assert!(!cs.timer.is_infinite);

        assert_eq!(cs.my_team.len(), 2);
        let me = cs.local_player().unwrap();
        assert_eq!(me.cell_id, 1);
        assert_eq!(me.game_name.as_deref(), Some("Jogador"));
        assert_eq!(me.tag_line.as_deref(), Some("BR1"));
        assert_eq!(me.assigned_position, Position::Jungle);
        assert_eq!(me.champion_id, None);
        assert_eq!(me.champion_pick_intent, Some(121));
        assert_eq!(me.spell1_id, Some(4));

        let ally = &cs.my_team[0];
        assert!(!ally.is_local_player);
        assert_eq!(ally.game_name, None);
        assert_eq!(ally.assigned_position, Position::Bottom);
        assert_eq!(ally.champion_id, Some(145));
        assert_eq!(ally.selected_skin_id, Some(145_000));

        let enemy = &cs.their_team[0];
        assert_eq!(enemy.assigned_position, Position::Unselected);
        assert_eq!(enemy.champion_id, Some(238));
        assert_eq!(enemy.spell1_id, None);
        assert_eq!(cs.their_team[1].champion_id, None);
    }

    #[test]
    fn acoes_na_ordem_do_draft_e_a_vez_do_jogador() {
        let cs = ChampSelect::from_json_at(&session(), 0).unwrap();
        let order: Vec<(u32, u32)> = cs.actions.iter().map(|a| (a.id, a.turn)).collect();
        assert_eq!(
            order,
            [
                (0, 0),
                (1, 0),
                (5, 0),
                (6, 0),
                (100, 1),
                (10, 2),
                (11, 3),
                (13, 4),
                (15, 5)
            ]
        );
        let reveal = &cs.actions[4];
        assert_eq!(reveal.action_type, ChampSelectActionType::TenBansReveal);
        assert_eq!(reveal.actor_cell_id, -1);

        let pending = cs.pending_action.as_ref().unwrap();
        assert_eq!(pending.id, 13);
        assert_eq!(pending.action_type, ChampSelectActionType::Pick);
        assert_eq!(pending.champion_id, Some(121));
        assert!(pending.is_ally_action);

        // Ban vazio (tempo acabou sem escolher) não entra.
        assert_eq!(cs.bans.my_team, [117, 43]);
        assert_eq!(cs.bans.their_team, [245]);
    }

    #[test]
    fn sem_vez_quando_a_acao_esta_confirmada_ou_e_de_outro() {
        let mut raw = session();
        raw["actions"][4][0]["completed"] = json!(true);
        let cs = ChampSelect::from_json_at(&raw, 0).unwrap();
        assert_eq!(cs.pending_action, None);

        let mut raw = session();
        raw["localPlayerCellId"] = json!(0);
        assert_eq!(
            ChampSelect::from_json_at(&raw, 0).unwrap().pending_action,
            None
        );
    }

    #[test]
    fn confere_hover_e_confirmacao() {
        use ActionIntent::{Confirm, Hover};
        let cs = ChampSelect::from_json_at(&session(), 0).unwrap();
        let check = |action, champion, intent| check_action(Some(&cs), action, champion, intent);

        // A vez do jogador: pick 13.
        assert_eq!(check(13, 121, Hover), Ok(()));
        assert_eq!(check(13, 121, Confirm), Ok(()));
        assert_eq!(
            check_action(None, 13, 121, Hover),
            Err(InvalidAction::NoChampSelect)
        );
        assert_eq!(check(13, 0, Confirm), Err(InvalidAction::NoChampion));
        assert_eq!(check(999, 121, Hover), Err(InvalidAction::UnknownAction));
        // Pick de outro jogador, e o ban já feito pelo jogador.
        assert_eq!(check(15, 121, Hover), Err(InvalidAction::NotYourAction));
        assert_eq!(check(1, 43, Confirm), Err(InvalidAction::AlreadyCompleted));
        assert_eq!(check(100, 1, Hover), Err(InvalidAction::NotYourAction));

        // Antes da vez: o pick aceita hover (intenção), mas não confirmação;
        // o ban não aceita nenhum dos dois.
        let mut raw = session();
        raw["actions"][4][0]["isInProgress"] = json!(false);
        raw["actions"][0][1]["completed"] = json!(false);
        let cs = ChampSelect::from_json_at(&raw, 0).unwrap();
        let check = |action, intent| check_action(Some(&cs), action, 121, intent);
        assert_eq!(check(13, Hover), Ok(()));
        assert_eq!(check(13, Confirm), Err(InvalidAction::NotYourTurn));
        assert_eq!(check(1, Hover), Err(InvalidAction::NotYourTurn));
        assert_eq!(check(1, Confirm), Err(InvalidAction::NotYourTurn));

        // Ação que não é ban nem pick, do próprio jogador.
        let mut raw = session();
        raw["actions"][4][0]["type"] = json!("vote");
        let cs = ChampSelect::from_json_at(&raw, 0).unwrap();
        assert_eq!(
            check_action(Some(&cs), 13, 121, Hover),
            Err(InvalidAction::NotBanOrPick)
        );
    }

    #[test]
    fn caminhos_das_acoes() {
        assert_eq!(action_path(13), "/lol-champ-select/v1/session/actions/13");
        assert_eq!(
            complete_action_path(13),
            "/lol-champ-select/v1/session/actions/13/complete"
        );
    }

    #[test]
    fn bans_do_campo_bans_tambem_contam() {
        let mut raw = session();
        raw["bans"] = json!({"myTeamBans": [117, 99], "theirTeamBans": [1], "numBans": 6});
        let cs = ChampSelect::from_json_at(&raw, 0).unwrap();
        assert_eq!(cs.bans.my_team, [117, 43, 99]);
        assert_eq!(cs.bans.their_team, [245, 1]);
    }

    #[test]
    fn valores_novos_viram_unknown_e_lixo_vira_none() {
        let mut raw = session();
        raw["timer"]["phase"] = json!("ETAPA_NOVA");
        raw["actions"][0][0]["type"] = json!("vote");
        raw["myTeam"][0]["assignedPosition"] = json!("cargo-novo");
        let cs = ChampSelect::from_json_at(&raw, 0).unwrap();
        assert_eq!(cs.timer.phase, ChampSelectPhase::Unknown);
        assert_eq!(cs.actions[0].action_type, ChampSelectActionType::Unknown);
        assert_eq!(cs.my_team[0].assigned_position, Position::Unselected);

        assert_eq!(ChampSelect::from_json(&json!(null)), None);
        assert_eq!(ChampSelect::from_json(&json!({})), None);
        assert_eq!(
            ChampSelect::from_json(&json!({"errorCode": "RPC_ERROR", "httpStatus": 404})),
            None
        );
    }

    #[test]
    fn timer_infinito_e_negativo() {
        let mut raw = session();
        raw["timer"] = json!({"phase": "PLANNING", "adjustedTimeLeftInPhase": -40,
                              "totalTimeInPhase": 0, "isInfinite": true});
        let cs = ChampSelect::from_json_at(&raw, 500).unwrap();
        assert_eq!(cs.timer.phase, ChampSelectPhase::Planning);
        assert!(cs.timer.is_infinite);
        assert_eq!(cs.timer.time_left_ms, 0);
        assert_eq!(cs.timer.ends_at_ms, 500);
    }

    #[test]
    fn formato_para_a_ui() {
        let cs = ChampSelect::from_json_at(&session(), 1_000).unwrap();
        let value = serde_json::to_value(&cs).unwrap();
        assert_eq!(
            value["timer"],
            json!({
                "phase": "BAN_PICK",
                "timeLeftMs": 21637,
                "totalTimeMs": 25000,
                "isInfinite": false,
                "endsAtMs": 22637,
            })
        );
        assert_eq!(
            value["pendingAction"],
            json!({
                "id": 13,
                "type": "pick",
                "actorCellId": 1,
                "championId": 121,
                "completed": false,
                "isInProgress": true,
                "isAllyAction": true,
                "turn": 4,
            })
        );
        assert_eq!(
            value["myTeam"][1],
            json!({
                "cellId": 1,
                "isLocalPlayer": true,
                "gameName": "Jogador",
                "tagLine": "BR1",
                "assignedPosition": "jungle",
                "championId": null,
                "championPickIntent": 121,
                "selectedSkinId": null,
                "spell1Id": 4,
                "spell2Id": 14,
            })
        );
        assert_eq!(
            value["bans"],
            json!({"myTeam": [117, 43], "theirTeam": [245]})
        );
        assert_eq!(value["queue"], json!("rankedSolo"));
        for phase in ChampSelectPhase::ALL {
            assert_eq!(serde_json::to_value(phase).unwrap(), json!(phase.as_str()));
        }
        for kind in ChampSelectActionType::ALL {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(kind.as_str()));
        }
        // Ida e volta: a UI de teste do mock pode reler o que o agente mandou.
        let back: ChampSelect = serde_json::from_value(value).unwrap();
        assert_eq!(back, cs);
    }
}
