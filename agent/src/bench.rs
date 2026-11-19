//! Reroll e banco de campeões do ARAM (#201).
//!
//! No ARAM (e na ARAM: Desordem) a seleção não tem ban nem pick: a LCU
//! sorteia um campeão para cada jogador. A sessão
//! (`GET /lol-champ-select/v1/session`) diz:
//!
//! - `benchEnabled`: a seleção tem banco;
//! - `benchChampions`: os campeões no banco do time, com `championId` e
//!   `isPriority` (versões antigas mandavam só `benchChampionIds`);
//! - `allowRerolling` e `rerollsRemaining`: se dá para rolar de novo e
//!   quantos rerolls o jogador ainda tem.
//!
//! O jogador pode:
//!
//! - [`reroll`]: `POST /lol-champ-select/v1/session/my-selection/reroll`
//!   sorteia outro campeão; o antigo vai para o banco;
//! - [`swap_with_bench`]: `POST /lol-champ-select/v1/session/bench/swap/{id}`
//!   troca o campeão dele pelo `id` do banco; o dele vai para o banco.
//!
//! [`check_reroll`] e [`check_bench_swap`] conferem o pedido contra a sessão
//! atual antes de falar com a LCU.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::champ_select::{ChampSelect, ChampSelectPhase, CHAMP_SELECT_SESSION_PATH};
use crate::rest::{LcuClient, LcuError};

/// Um campeão no banco do time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BenchChampion {
    pub champion_id: u32,
    /// Campeão com prioridade no banco (a LCU marca alguns assim no ARAM).
    pub is_priority: bool,
    /// Quando o campeão libera para troca, em milissegundos desde 1970 pelo
    /// relógio do computador (#588). O client oficial trava o banco por
    /// [`BENCH_COOLDOWN_MS`] e a LCU não manda esse tempo, então o agente
    /// calcula (veja [`apply_bench_cooldown`]). `null` quando não há espera
    /// conhecida. A espera que falta é
    /// `Math.max(0, availableAtMs - Date.now())`.
    #[ts(type = "number | null")]
    pub available_at_ms: Option<u64>,
}

/// Quanto o banco fica travado depois de cada mudança (#588).
///
/// A LCU não manda essa espera: nem a sessão nem a mensagem do servidor
/// (`teambuilder/v1/tbdGameDtoV1`) têm campo para ela. Medida numa gravação
/// de tela do client oficial no LoL real (16.22): cerca de 2,6 s.
pub const BENCH_COOLDOWN_MS: u64 = 2_600;

/// O que o agente já viu do banco numa seleção, para calcular a espera de
/// cada campeão (#588).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BenchCooldown {
    /// Id da seleção.
    session_id: String,
    /// Quando cada campeão chegou ao banco. `0` para os que já estavam lá
    /// quando o agente viu a seleção pela primeira vez (hora desconhecida).
    arrived_at: HashMap<u32, u64>,
    /// Quando o jogador logado trocou com o banco pela última vez.
    last_swap_at: Option<u64>,
}

/// Preenche `available_at_ms` de cada campeão do banco de `cs` (#588).
///
/// Como no client oficial (gravação de tela no LoL real, 16.22):
/// - na fase das cartas (`BAN_PICK`) o banco fica travado até a etapa
///   acabar e mais [`BENCH_COOLDOWN_MS`];
/// - no começo da finalização o banco inteiro trava por
///   [`BENCH_COOLDOWN_MS`];
/// - depois de cada troca do jogador logado com o banco, o banco inteiro
///   trava de novo;
/// - um campeão que acabou de chegar ao banco (troca de um aliado) trava
///   sozinho.
///
/// `previous` é a seleção publicada antes desta; `now_ms` é a hora em que
/// esta chegou.
pub(crate) fn apply_bench_cooldown(
    tracker: &mut Option<BenchCooldown>,
    previous: Option<&ChampSelect>,
    cs: &mut ChampSelect,
    now_ms: u64,
) {
    if !cs.bench_enabled {
        return;
    }
    let fresh = tracker.as_ref().is_none_or(|t| t.session_id != cs.id);
    if fresh {
        *tracker = Some(BenchCooldown {
            session_id: cs.id.clone(),
            ..BenchCooldown::default()
        });
    }
    let Some(track) = tracker.as_mut() else {
        return;
    };
    let previous = previous.filter(|p| p.id == cs.id);

    // O jogador logado pegou um campeão que estava no banco.
    let local_champion = |c: &ChampSelect| c.local_player().and_then(|p| p.champion_id);
    if let Some(prev) = previous {
        let now_mine = local_champion(cs);
        if now_mine.is_some()
            && now_mine != local_champion(prev)
            && prev
                .bench_champions
                .iter()
                .any(|b| Some(b.champion_id) == now_mine)
        {
            track.last_swap_at = Some(now_ms);
        }
    }

    let on_bench: Vec<u32> = cs.bench_champions.iter().map(|b| b.champion_id).collect();
    track.arrived_at.retain(|id, _| on_bench.contains(id));
    for id in &on_bench {
        track
            .arrived_at
            .entry(*id)
            .or_insert(if fresh { 0 } else { now_ms });
    }

    let timer = &cs.timer;
    let phase_lock = match timer.phase {
        _ if timer.is_infinite => 0,
        ChampSelectPhase::Planning | ChampSelectPhase::BanPick => timer.ends_at_ms,
        ChampSelectPhase::Finalization => timer
            .ends_at_ms
            .saturating_sub(u64::from(timer.total_time_ms)),
        _ => 0,
    };
    let after = |at: u64| {
        if at == 0 {
            0
        } else {
            at.saturating_add(BENCH_COOLDOWN_MS)
        }
    };
    let common = after(phase_lock).max(track.last_swap_at.map_or(0, after));
    for champion in &mut cs.bench_champions {
        let arrived = track
            .arrived_at
            .get(&champion.champion_id)
            .copied()
            .unwrap_or(0);
        let at = common.max(after(arrived));
        champion.available_at_ms = (at > 0).then_some(at);
    }
}

/// Caminho do reroll.
pub fn reroll_path() -> String {
    format!("{CHAMP_SELECT_SESSION_PATH}/my-selection/reroll")
}

/// Caminho da troca com o campeão `champion_id` do banco.
pub fn bench_swap_path(champion_id: u32) -> String {
    format!("{CHAMP_SELECT_SESSION_PATH}/bench/swap/{champion_id}")
}

/// Por que um reroll ou uma troca com o banco não vale. Nada foi mandado para
/// a LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidBenchAction {
    /// Não há seleção de campeões aberta.
    NoChampSelect,
    /// A partida já está abrindo.
    GameStarting,
    /// A seleção não tem banco (não é ARAM).
    NoBench,
    /// O campeão não está no banco.
    NotOnBench,
    /// O jogador não tem o campeão (nem está na rotação).
    NotOwned,
    /// A fila não deixa rolar de novo.
    RerollNotAllowed,
    /// O jogador não tem mais rerolls.
    NoRerolls,
}

impl fmt::Display for InvalidBenchAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            InvalidBenchAction::NoChampSelect => "não há seleção de campeões aberta",
            InvalidBenchAction::GameStarting => "a partida já está abrindo",
            InvalidBenchAction::NoBench => "essa seleção não tem banco de campeões",
            InvalidBenchAction::NotOnBench => "esse campeão não está no banco",
            InvalidBenchAction::NotOwned => "você não tem esse campeão",
            InvalidBenchAction::RerollNotAllowed => "essa fila não deixa rolar de novo",
            InvalidBenchAction::NoRerolls => "você não tem mais rerolls",
        })
    }
}

impl std::error::Error for InvalidBenchAction {}

fn open(champ_select: Option<&ChampSelect>) -> Result<&ChampSelect, InvalidBenchAction> {
    let cs = champ_select.ok_or(InvalidBenchAction::NoChampSelect)?;
    if cs.timer.phase == ChampSelectPhase::GameStarting {
        return Err(InvalidBenchAction::GameStarting);
    }
    Ok(cs)
}

/// Confere um pedido de reroll: seleção aberta, fila que deixa rolar de novo
/// e reroll sobrando.
pub fn check_reroll(champ_select: Option<&ChampSelect>) -> Result<(), InvalidBenchAction> {
    let cs = open(champ_select)?;
    if !cs.allow_rerolling {
        return Err(InvalidBenchAction::RerollNotAllowed);
    }
    if cs.rerolls_remaining == 0 {
        return Err(InvalidBenchAction::NoRerolls);
    }
    Ok(())
}

/// Confere um pedido de troca com o banco: seleção aberta com banco, o
/// campeão nele e, quando o agente já sabe quais campeões o jogador pode
/// escolher (`pickable`, os dele e os da rotação), o campeão nessa lista.
/// Com `pickable` em `None` a troca passa e a LCU decide.
pub fn check_bench_swap(
    champ_select: Option<&ChampSelect>,
    pickable: Option<&[u32]>,
    champion_id: u32,
) -> Result<(), InvalidBenchAction> {
    let cs = open(champ_select)?;
    if !cs.bench_enabled {
        return Err(InvalidBenchAction::NoBench);
    }
    if !cs
        .bench_champions
        .iter()
        .any(|c| c.champion_id == champion_id)
    {
        return Err(InvalidBenchAction::NotOnBench);
    }
    if pickable.is_some_and(|ids| !ids.contains(&champion_id)) {
        return Err(InvalidBenchAction::NotOwned);
    }
    Ok(())
}

/// Sorteia outro campeão para o jogador. A sessão nova chega pelo WAMP (ou
/// por [`crate::champ_select::current_session`]).
pub async fn reroll(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.post::<_, Option<Value>>(&reroll_path(), &json!({}))
        .await
        .map(|_| ())
}

/// Troca o campeão do jogador pelo `champion_id` do banco.
pub async fn swap_with_bench(lcu: &LcuClient, champion_id: u32) -> Result<(), LcuError> {
    lcu.post::<_, Option<Value>>(&bench_swap_path(champion_id), &json!({}))
        .await
        .map(|_| ())
}

/// Lê o banco da sessão: `benchChampions` ou, se não vier, o antigo
/// `benchChampionIds`. Entradas sem campeão são puladas.
pub(crate) fn bench_from_raw(champions: &[Value], ids: &[Value]) -> Vec<BenchChampion> {
    let id = |v: &Value| {
        v.as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|&n| n > 0)
    };
    if !champions.is_empty() {
        return champions
            .iter()
            .filter_map(|c| {
                Some(BenchChampion {
                    champion_id: id(&c["championId"])?,
                    is_priority: c["isPriority"].as_bool().unwrap_or(false),
                    available_at_ms: None,
                })
            })
            .collect();
    }
    ids.iter()
        .filter_map(|v| {
            Some(BenchChampion {
                champion_id: id(v)?,
                is_priority: false,
                available_at_ms: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aram(extra: Value) -> ChampSelect {
        let mut raw = json!({
            "id": "aram-1",
            "queueId": 450,
            "localPlayerCellId": 0,
            "timer": {"phase": "BAN_PICK", "adjustedTimeLeftInPhase": 50000,
                      "totalTimeInPhase": 60000},
            "myTeam": [{"cellId": 0, "championId": 22}, {"cellId": 1, "championId": 99}],
            "actions": [],
            "benchEnabled": true,
            "benchChampions": [
                {"championId": 1, "isPriority": false},
                {"championId": 266, "isPriority": true},
            ],
            "allowRerolling": true,
            "rerollsRemaining": 1,
        });
        for (k, v) in extra.as_object().unwrap() {
            raw[k] = v.clone();
        }
        ChampSelect::from_json_at(&raw, 0).unwrap()
    }

    #[test]
    fn le_o_banco_e_os_rerolls() {
        let cs = aram(json!({}));
        assert!(cs.bench_enabled);
        assert!(cs.allow_rerolling);
        assert_eq!(cs.rerolls_remaining, 1);
        assert_eq!(
            cs.bench_champions,
            [
                BenchChampion {
                    champion_id: 1,
                    is_priority: false,
                    available_at_ms: None,
                },
                BenchChampion {
                    champion_id: 266,
                    is_priority: true,
                    available_at_ms: None,
                },
            ]
        );
        // Formato antigo, e lixo pulado.
        let cs = aram(json!({"benchChampions": [], "benchChampionIds": [7, 0, -3, 8]}));
        let ids: Vec<u32> = cs.bench_champions.iter().map(|c| c.champion_id).collect();
        assert_eq!(ids, [7, 8]);
        let cs = aram(json!({"benchChampions": [{"isPriority": true}, {"championId": 5}]}));
        assert_eq!(
            cs.bench_champions,
            [BenchChampion {
                champion_id: 5,
                is_priority: false,
                available_at_ms: None,
            }]
        );
    }

    /// Uma sessão de ARAM recebida em `now`, com o banco `bench`, o jogador
    /// logado com `mine` e a etapa `phase` com `left` de `total` ms.
    fn cooldown_session(
        phase: &str,
        left: i64,
        total: i64,
        mine: u32,
        bench: &[u32],
        now: u64,
    ) -> ChampSelect {
        let bench: Vec<Value> = bench.iter().map(|id| json!({"championId": id})).collect();
        let mut raw = json!({
            "id": "aram-1",
            "queueId": 2400,
            "localPlayerCellId": 0,
            "timer": {"phase": phase, "adjustedTimeLeftInPhase": left, "totalTimeInPhase": total},
            "myTeam": [{"cellId": 0, "championId": mine}, {"cellId": 1, "championId": 99}],
            "actions": [],
            "benchEnabled": true,
            "benchChampions": bench,
        });
        raw["allowRerolling"] = json!(false);
        ChampSelect::from_json_at(&raw, now).unwrap()
    }

    fn available(cs: &ChampSelect) -> Vec<(u32, Option<u64>)> {
        cs.bench_champions
            .iter()
            .map(|b| (b.champion_id, b.available_at_ms))
            .collect()
    }

    /// Aplica a espera como o hub faz e devolve a seleção publicada.
    fn publish(
        tracker: &mut Option<BenchCooldown>,
        previous: Option<&ChampSelect>,
        mut cs: ChampSelect,
        now: u64,
    ) -> ChampSelect {
        apply_bench_cooldown(tracker, previous, &mut cs, now);
        cs
    }

    #[test]
    fn espera_do_banco_como_no_client_oficial() {
        let cd = BENCH_COOLDOWN_MS;
        let mut tracker = None;
        // Fase das cartas: travado até a etapa acabar (10 s) e mais a espera.
        let cards = publish(
            &mut tracker,
            None,
            cooldown_session("BAN_PICK", 10_000, 15_000, 0, &[1], 5_000),
            5_000,
        );
        assert_eq!(available(&cards), [(1, Some(15_000 + cd))]);

        // A finalização começou antes (aos 12 s): conta a partir dela.
        let fin = publish(
            &mut tracker,
            Some(&cards),
            cooldown_session("FINALIZATION", 30_000, 30_000, 7, &[1, 2], 12_000),
            12_000,
        );
        assert_eq!(
            available(&fin),
            [(1, Some(12_000 + cd)), (2, Some(12_000 + cd))]
        );

        // Um aliado trocou: só o campeão que chegou ao banco (3) trava.
        let ally = publish(
            &mut tracker,
            Some(&fin),
            cooldown_session("FINALIZATION", 25_000, 30_000, 7, &[1, 2, 3], 17_000),
            17_000,
        );
        assert_eq!(
            available(&ally),
            [
                (1, Some(12_000 + cd)),
                (2, Some(12_000 + cd)),
                (3, Some(17_000 + cd))
            ]
        );

        // O jogador pegou o 1 do banco: o banco inteiro trava de novo, e o
        // 7 dele chega ao banco.
        let mine = publish(
            &mut tracker,
            Some(&ally),
            cooldown_session("FINALIZATION", 20_000, 30_000, 1, &[7, 2, 3], 22_000),
            22_000,
        );
        assert_eq!(
            available(&mine),
            [
                (7, Some(22_000 + cd)),
                (2, Some(22_000 + cd)),
                (3, Some(22_000 + cd))
            ]
        );

        // Atualização sem mudança no banco: nada muda.
        let again = publish(
            &mut tracker,
            Some(&mine),
            cooldown_session("FINALIZATION", 19_000, 30_000, 1, &[7, 2, 3], 23_000),
            23_000,
        );
        assert_eq!(available(&again), available(&mine));
    }

    #[test]
    fn espera_do_banco_sem_ter_visto_o_comeco() {
        // O agente conectou no meio da finalização: o que já estava no banco
        // só espera o começo da etapa, que já passou há muito.
        let mut tracker = None;
        let cs = publish(
            &mut tracker,
            None,
            cooldown_session("FINALIZATION", 10_000, 30_000, 7, &[1], 40_000),
            40_000,
        );
        assert_eq!(available(&cs), [(1, Some(20_000 + BENCH_COOLDOWN_MS))]);
        // Outra seleção começa do zero.
        let other = {
            let mut cs = cooldown_session("FINALIZATION", 30_000, 30_000, 7, &[1], 90_000);
            cs.id = "aram-2".into();
            cs
        };
        let other = publish(&mut tracker, Some(&cs), other, 90_000);
        assert_eq!(available(&other), [(1, Some(90_000 + BENCH_COOLDOWN_MS))]);
        // Sem banco, nada é preenchido.
        let mut normal = cooldown_session("BAN_PICK", 10_000, 15_000, 7, &[1], 0);
        normal.bench_enabled = false;
        let normal = publish(&mut tracker, None, normal, 0);
        assert_eq!(available(&normal), [(1, None)]);
    }

    #[test]
    fn confere_reroll() {
        assert_eq!(check_reroll(Some(&aram(json!({})))), Ok(()));
        assert_eq!(check_reroll(None), Err(InvalidBenchAction::NoChampSelect));
        assert_eq!(
            check_reroll(Some(&aram(json!({"rerollsRemaining": 0})))),
            Err(InvalidBenchAction::NoRerolls)
        );
        assert_eq!(
            check_reroll(Some(&aram(json!({"allowRerolling": false})))),
            Err(InvalidBenchAction::RerollNotAllowed)
        );
        assert_eq!(
            check_reroll(Some(&aram(json!({"timer": {"phase": "GAME_STARTING"}})))),
            Err(InvalidBenchAction::GameStarting)
        );
    }

    #[test]
    fn confere_troca_com_o_banco() {
        let cs = aram(json!({}));
        assert_eq!(check_bench_swap(Some(&cs), None, 266), Ok(()));
        assert_eq!(
            check_bench_swap(Some(&cs), None, 22),
            Err(InvalidBenchAction::NotOnBench)
        );
        assert_eq!(
            check_bench_swap(None, None, 1),
            Err(InvalidBenchAction::NoChampSelect)
        );
        assert_eq!(
            check_bench_swap(Some(&aram(json!({"benchEnabled": false}))), None, 1),
            Err(InvalidBenchAction::NoBench)
        );
        // Campeão que o jogador não tem: recusado sem falar com a LCU.
        assert_eq!(
            check_bench_swap(Some(&cs), Some(&[1, 22]), 266),
            Err(InvalidBenchAction::NotOwned)
        );
        assert_eq!(check_bench_swap(Some(&cs), Some(&[1, 22]), 1), Ok(()));
    }

    /// As sessões da gravação real de ARAM com cartas (16.22-aram).
    fn recorded_sessions() -> Vec<ChampSelect> {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/16.22-aram");
        let set = crate::fixtures::FixtureSet::load(&dir).unwrap();
        set.events
            .iter()
            .filter(|e| e.uri == CHAMP_SELECT_SESSION_PATH)
            .map(|e| ChampSelect::from_json_at(&e.data, 0).unwrap())
            .collect()
    }

    #[test]
    fn le_a_gravacao_com_cartas() {
        let sessions = recorded_sessions();
        // Abre com o pick em andamento (a escolha da carta) e o banco vazio.
        let open = &sessions[0];
        assert!(open.bench_enabled);
        assert!(open.bench_champions.is_empty());
        assert_eq!(open.timer.phase, ChampSelectPhase::BanPick);
        let pick = open.pending_action.as_ref().unwrap();
        assert_eq!(
            pick.action_type,
            crate::champ_select::ChampSelectActionType::Pick
        );
        // Com 0 rerolls, mesmo com `allowRerolling`, o reroll não vale.
        assert_eq!(check_reroll(Some(open)), Err(InvalidBenchAction::NoRerolls));
        // Escolheu o Jayce (126): as outras cartas foram para o banco.
        let chosen = &sessions[1];
        assert_eq!(chosen.local_player().unwrap().champion_id, Some(126));
        let bench: Vec<u32> = chosen
            .bench_champions
            .iter()
            .map(|c| c.champion_id)
            .collect();
        assert_eq!(bench, [432, 77]);
        assert!(chosen.pending_action.is_none());
        assert_eq!(check_bench_swap(Some(chosen), None, 77), Ok(()));
        // Trocou pelo Nunu (77) do banco.
        let swapped = &sessions[2];
        assert_eq!(swapped.local_player().unwrap().champion_id, Some(77));
        let bench: Vec<u32> = swapped
            .bench_champions
            .iter()
            .map(|c| c.champion_id)
            .collect();
        assert_eq!(bench, [432, 126]);
    }

    #[test]
    fn caminhos() {
        assert_eq!(
            reroll_path(),
            "/lol-champ-select/v1/session/my-selection/reroll"
        );
        assert_eq!(
            bench_swap_path(266),
            "/lol-champ-select/v1/session/bench/swap/266"
        );
    }

    #[test]
    fn formato_para_a_ui() {
        let value = serde_json::to_value(aram(json!({}))).unwrap();
        assert_eq!(value["benchEnabled"], json!(true));
        assert_eq!(value["allowRerolling"], json!(true));
        assert_eq!(value["rerollsRemaining"], json!(1));
        assert_eq!(
            value["benchChampions"],
            json!([{"championId": 1, "isPriority": false, "availableAtMs": null},
                   {"championId": 266, "isPriority": true, "availableAtMs": null}])
        );
    }
}
