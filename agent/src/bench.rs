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

/// Confere um pedido de troca com o banco: seleção aberta com banco e o
/// campeão nele.
pub fn check_bench_swap(
    champ_select: Option<&ChampSelect>,
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
                })
            })
            .collect();
    }
    ids.iter()
        .filter_map(|v| {
            Some(BenchChampion {
                champion_id: id(v)?,
                is_priority: false,
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
                    is_priority: false
                },
                BenchChampion {
                    champion_id: 266,
                    is_priority: true
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
                is_priority: false
            }]
        );
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
        assert_eq!(check_bench_swap(Some(&cs), 266), Ok(()));
        assert_eq!(
            check_bench_swap(Some(&cs), 22),
            Err(InvalidBenchAction::NotOnBench)
        );
        assert_eq!(
            check_bench_swap(None, 1),
            Err(InvalidBenchAction::NoChampSelect)
        );
        assert_eq!(
            check_bench_swap(Some(&aram(json!({"benchEnabled": false}))), 1),
            Err(InvalidBenchAction::NoBench)
        );
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
        assert_eq!(check_bench_swap(Some(chosen), 77), Ok(()));
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
            json!([{"championId": 1, "isPriority": false},
                   {"championId": 266, "isPriority": true}])
        );
    }
}
