//! Atualização do LoL (patch) em andamento (#575).
//!
//! Quando sai um patch, o LoL baixa os arquivos novos do jogo com o client
//! aberto, e enquanto isso a fila não começa. O client oficial mostra o
//! progresso no botão "Jogar"; com a UI oficial fechada pelo `kill-ux`, o
//! jogador do easylobby não via nada e achava que o app tinha travado.
//!
//! O agente acompanha dois recursos da LCU:
//!
//! - [`PATCH_STATE_PATH`]: o patcher do jogo. `action` diz o que ele está
//!   fazendo (`Idle`, `CheckingForUpdates`, `Patching`, `Repairing`...),
//!   `percentPatched` vai de 0 a 100 e `isUpdateAvailable` diz se há
//!   atualização esperando. Os componentes trazem os bytes baixados em
//!   `progress.total`, usados quando `percentPatched` não vem.
//! - [`AVAILABILITY_PATH`]: se o gameflow deixa jogar. `{"isAvailable":
//!   false, "state": "Patching"}` é a fila travada pelo patch; é o que o
//!   client oficial usa para desligar o botão "Jogar".
//!
//! [`summarize`] junta os dois num [`LolUpdate`] para a UI, ou `None` sem
//! atualização. A checagem rápida que o LoL faz ao abrir (meio segundo,
//! "Install is up to date" no log) só aparece se travar a fila, para o
//! aviso não piscar a cada conexão.
//!
//! Os formatos vêm do `/help` da LCU e do evento de `availability` gravado na
//! fixture 16.22; o de um patch de verdade ainda não foi gravado. Por isso a
//! leitura é tolerante: campo que falta ou valor novo não derrubam nada.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Estado do patcher do jogo.
pub const PATCH_STATE_PATH: &str = "/lol-patch/v1/products/league_of_legends/state";

/// Se o gameflow deixa jogar agora (`{"isAvailable": true, "state": "Available"}`).
pub const AVAILABILITY_PATH: &str = "/lol-gameflow/v1/availability";

/// O que o patcher está fazendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LolUpdateStage {
    /// Conferindo os arquivos ou preparando o download.
    Checking,
    /// Baixando e aplicando o patch.
    Downloading,
    /// Reparando arquivos do jogo.
    Repairing,
    /// Há atualização, mas o patcher está parado. O LoL trava o patcher
    /// durante o lobby, a fila e a partida, e o download começa quando o
    /// jogador sai deles.
    Pending,
}

/// Atualização do LoL em andamento, para o aviso da UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LolUpdate {
    pub stage: LolUpdateStage,
    /// Progresso de 0 a 100, ou `null` quando a LCU não diz.
    pub percent: Option<u8>,
    /// A LCU travou a fila até o patch terminar (`availability` em
    /// `Patching`).
    pub blocks_queue: bool,
}

impl LolUpdate {
    /// O LoL está mexendo nos arquivos do jogo. Nesse meio-tempo o agente não
    /// fecha a UI oficial.
    pub fn in_progress(&self) -> bool {
        matches!(
            self.stage,
            LolUpdateStage::Downloading | LolUpdateStage::Repairing
        ) || self.blocks_queue
    }
}

/// O patcher, lido de [`PATCH_STATE_PATH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PatcherState {
    /// O que o patcher faz agora; `None` parado (`Idle`).
    pub activity: Option<LolUpdateStage>,
    pub update_available: bool,
    pub percent: Option<u8>,
}

impl PatcherState {
    /// Lê a resposta (ou o `data` do evento) de [`PATCH_STATE_PATH`]. `None`
    /// se não for um objeto.
    pub fn from_json(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let action = object.get("action").and_then(Value::as_str).unwrap_or("");
        let activity = activity(action);
        let up_to_date = object.get("isUpToDate").and_then(Value::as_bool);
        let update_available = object
            .get("isUpdateAvailable")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && up_to_date != Some(true);
        let percent = object
            .get("percentPatched")
            .and_then(Value::as_f64)
            .and_then(to_percent)
            .or_else(|| components_percent(object.get("components")));
        Some(Self {
            activity,
            update_available,
            percent,
        })
    }
}

/// Etapa de um `action` do patcher. Ações novas que não são `Idle` contam
/// como download, que é o que travar a fila costuma significar.
fn activity(action: &str) -> Option<LolUpdateStage> {
    let action = action.to_ascii_lowercase();
    if action.is_empty() || action == "idle" {
        None
    } else if action.contains("check") || action.contains("verif") {
        Some(LolUpdateStage::Checking)
    } else if action.contains("repair") {
        Some(LolUpdateStage::Repairing)
    } else {
        Some(LolUpdateStage::Downloading)
    }
}

fn to_percent(value: f64) -> Option<u8> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Some(value.min(100.0).floor() as u8)
}

/// Porcentagem pelos bytes dos componentes (`progress.total.bytesComplete`
/// sobre `bytesRequired`), somando todos.
fn components_percent(components: Option<&Value>) -> Option<u8> {
    let (mut done, mut required) = (0u64, 0u64);
    for component in components?.as_array()? {
        let total = &component["progress"]["total"];
        let (Some(complete), Some(needed)) = (
            total["bytesComplete"].as_u64(),
            total["bytesRequired"].as_u64(),
        ) else {
            continue;
        };
        done = done.saturating_add(complete);
        required = required.saturating_add(needed);
    }
    if required == 0 {
        return None;
    }
    to_percent(done as f64 * 100.0 / required as f64)
}

/// Se a resposta (ou o `data` do evento) de [`AVAILABILITY_PATH`] diz que a
/// fila espera o patch.
pub fn blocked_by_patch(value: &Value) -> bool {
    value["state"]
        .as_str()
        .is_some_and(|state| state.eq_ignore_ascii_case("patching"))
}

/// O aviso para a UI, ou `None` sem atualização.
pub fn summarize(patcher: PatcherState, blocks_queue: bool) -> Option<LolUpdate> {
    let stage = match patcher.activity {
        Some(LolUpdateStage::Checking) if !blocks_queue && !patcher.update_available => {
            return None;
        }
        Some(stage) => stage,
        None if patcher.update_available => LolUpdateStage::Pending,
        // A LCU diz que a fila espera o patch, mas o patcher ainda não
        // começou (ou não respondeu).
        None if blocks_queue => LolUpdateStage::Checking,
        None => return None,
    };
    let percent = match stage {
        LolUpdateStage::Downloading | LolUpdateStage::Repairing => patcher.percent,
        _ => None,
    };
    Some(LolUpdate {
        stage,
        percent,
        blocks_queue,
    })
}

/// Lê o patcher. `Ok(None)` se a resposta não for um objeto.
pub async fn patcher_state(client: &LcuClient) -> Result<Option<PatcherState>, LcuError> {
    let value: Value = client.get(PATCH_STATE_PATH).await?;
    Ok(PatcherState::from_json(&value))
}

/// Lê se a fila espera o patch.
pub async fn queue_blocked(client: &LcuClient) -> Result<bool, LcuError> {
    let value: Value = client.get(AVAILABILITY_PATH).await?;
    Ok(blocked_by_patch(&value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Resposta do patcher no formato do `/help` da LCU.
    fn state(action: &str, percent: f64, available: bool, up_to_date: bool) -> Value {
        json!({
            "action": action,
            "components": [{
                "action": action,
                "id": "lol_game_client",
                "isCorrupted": false,
                "isUpToDate": up_to_date,
                "isUpdateAvailable": available,
                "progress": null,
            }],
            "isCorrupted": false,
            "isStopped": false,
            "isUpToDate": up_to_date,
            "isUpdateAvailable": available,
            "percentPatched": percent,
        })
    }

    #[test]
    fn parado_e_em_dia_nao_tem_aviso() {
        let patcher = PatcherState::from_json(&state("Idle", 100.0, false, true)).unwrap();
        assert_eq!(patcher.activity, None);
        assert!(!patcher.update_available);
        assert_eq!(summarize(patcher, false), None);
    }

    #[test]
    fn baixando_mostra_a_porcentagem() {
        let patcher = PatcherState::from_json(&state("Patching", 42.7, true, false)).unwrap();
        assert_eq!(
            summarize(patcher, true),
            Some(LolUpdate {
                stage: LolUpdateStage::Downloading,
                percent: Some(42),
                blocks_queue: true,
            })
        );
    }

    #[test]
    fn sem_percent_patched_usa_os_bytes_dos_componentes() {
        let value = json!({
            "action": "Patching",
            "components": [
                {"progress": {"total": {"bytesComplete": 30, "bytesRequired": 100}}},
                {"progress": {"total": {"bytesComplete": 20, "bytesRequired": 100}}},
                {"progress": null},
            ],
        });
        let patcher = PatcherState::from_json(&value).unwrap();
        assert_eq!(patcher.percent, Some(25));
        // Valor fora da faixa não passa de 100.
        let patcher = PatcherState::from_json(&state("Patching", 180.0, true, false)).unwrap();
        assert_eq!(patcher.percent, Some(100));
    }

    #[test]
    fn checagem_rapida_ao_abrir_nao_pisca_o_aviso() {
        let patcher =
            PatcherState::from_json(&state("CheckingForUpdates", 0.0, false, true)).unwrap();
        assert_eq!(patcher.activity, Some(LolUpdateStage::Checking));
        assert_eq!(summarize(patcher, false), None);
        // Mas, se a fila travou, o aviso aparece.
        assert_eq!(
            summarize(patcher, true).map(|u| u.stage),
            Some(LolUpdateStage::Checking)
        );
    }

    #[test]
    fn atualizacao_esperando_o_patcher_destravar() {
        let patcher = PatcherState::from_json(&state("Idle", 0.0, true, false)).unwrap();
        let update = summarize(patcher, false).unwrap();
        assert_eq!(update.stage, LolUpdateStage::Pending);
        assert_eq!(update.percent, None);
        assert!(!update.in_progress());
    }

    #[test]
    fn reparo_e_acao_nova() {
        let repair = PatcherState::from_json(&state("Repairing", 10.0, false, false)).unwrap();
        assert_eq!(repair.activity, Some(LolUpdateStage::Repairing));
        assert!(summarize(repair, false).unwrap().in_progress());
        let new = PatcherState::from_json(&state("Migrating", 5.0, false, false)).unwrap();
        assert_eq!(new.activity, Some(LolUpdateStage::Downloading));
    }

    #[test]
    fn fila_travada_sem_patcher() {
        let update = summarize(PatcherState::default(), true).unwrap();
        assert_eq!(update.stage, LolUpdateStage::Checking);
        assert!(update.in_progress());
    }

    #[test]
    fn availability() {
        assert!(blocked_by_patch(
            &json!({"isAvailable": false, "state": "Patching"})
        ));
        // Gravado na fixture 16.22.
        assert!(!blocked_by_patch(
            &json!({"isAvailable": true, "state": "InGameFlow"})
        ));
        assert!(!blocked_by_patch(&json!(null)));
        assert!(PatcherState::from_json(&json!("Idle")).is_none());
    }

    #[test]
    fn formato_para_a_ui() {
        let update = LolUpdate {
            stage: LolUpdateStage::Downloading,
            percent: Some(7),
            blocks_queue: true,
        };
        assert_eq!(
            serde_json::to_value(&update).unwrap(),
            json!({"stage": "downloading", "percent": 7, "blocksQueue": true})
        );
    }
}
