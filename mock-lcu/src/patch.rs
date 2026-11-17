//! Atualização do LoL simulada (#575).
//!
//! Para ver o aviso de patch do app sem esperar o dia de patch. O mock
//! trava a fila (`availability` em `Patching`), faz o patcher baixar de 0 a
//! 100% e, no fim, destrava a fila e deixa o patcher parado e em dia.
//!
//! Controle: `POST /mock-lcu/patch` com `{"segundos": 30}` começa (ou
//! recomeça) um patch que leva esse tempo (padrão: [`DEFAULT_SECONDS`]);
//! `DELETE /mock-lcu/patch` termina na hora. Na linha de comando,
//! `--patch <segundos>` começa um logo que o mock sobe.
//!
//! O formato do patcher segue o `/help` da LCU; o de um patch de verdade
//! ainda não foi gravado.

use easylobby_agent::fixtures::EventFixture;
use easylobby_agent::lol_update::{AVAILABILITY_PATH, PATCH_STATE_PATH};
use serde_json::{json, Value};

/// Começa (`POST`) ou termina (`DELETE`) o patch simulado.
pub const START: &str = "/mock-lcu/patch";

/// Duração padrão do patch simulado.
pub const DEFAULT_SECONDS: f64 = 20.0;

/// Quantos eventos de progresso o patch manda do começo ao fim.
pub const STEPS: u32 = 20;

/// Estado do patcher em `percent` (`None`: parado e em dia).
pub fn patcher(percent: Option<u8>) -> Value {
    let (action, up_to_date, percent) = match percent {
        Some(p) => ("Patching", false, f64::from(p)),
        None => ("Idle", true, 100.0),
    };
    json!({
        "action": action,
        "components": [{
            "action": action,
            "id": "lol_game_client",
            "isCorrupted": false,
            "isUpToDate": up_to_date,
            "isUpdateAvailable": !up_to_date,
            "progress": null,
        }],
        "isCorrupted": false,
        "isStopped": false,
        "isUpToDate": up_to_date,
        "isUpdateAvailable": !up_to_date,
        "percentPatched": percent,
    })
}

fn event(uri: &str, data: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.into(),
        event_type: "Update".into(),
        data,
    }
}

/// Eventos do começo do patch: a fila trava e o download começa em 0%.
pub fn begin() -> Vec<EventFixture> {
    vec![
        event(
            AVAILABILITY_PATH,
            json!({"isAvailable": false, "state": "Patching"}),
        ),
        event(PATCH_STATE_PATH, patcher(Some(0))),
    ]
}

/// Evento do passo `step` de [`STEPS`].
pub fn progress(step: u32) -> EventFixture {
    let percent = (step.min(STEPS) * 100 / STEPS) as u8;
    event(PATCH_STATE_PATH, patcher(Some(percent)))
}

/// Eventos do fim: o patcher para em dia e a fila destrava.
pub fn finish() -> Vec<EventFixture> {
    vec![
        event(PATCH_STATE_PATH, patcher(None)),
        event(
            AVAILABILITY_PATH,
            json!({"isAvailable": true, "state": "Available"}),
        ),
    ]
}

/// Duração pedida no corpo do `POST` (`{"segundos": 30}`).
pub fn seconds(body: &Value) -> f64 {
    body["segundos"]
        .as_f64()
        .filter(|s| s.is_finite() && *s > 0.0)
        .unwrap_or(DEFAULT_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use easylobby_agent::lol_update::{self, LolUpdateStage, PatcherState};

    #[test]
    fn o_agente_le_o_patch_simulado() {
        let blocked = lol_update::blocked_by_patch(&begin()[0].data);
        let patcher = PatcherState::from_json(&progress(STEPS / 2).data).unwrap();
        let update = lol_update::summarize(patcher, blocked).unwrap();
        assert_eq!(update.stage, LolUpdateStage::Downloading);
        assert_eq!(update.percent, Some(50));
        assert!(update.blocks_queue);

        let end = finish();
        let patcher = PatcherState::from_json(&end[0].data).unwrap();
        assert_eq!(
            lol_update::summarize(patcher, lol_update::blocked_by_patch(&end[1].data)),
            None
        );
    }

    #[test]
    fn duracao_do_post() {
        assert_eq!(seconds(&json!({"segundos": 5})), 5.0);
        assert_eq!(seconds(&json!({})), DEFAULT_SECONDS);
        assert_eq!(seconds(&json!({"segundos": -1})), DEFAULT_SECONDS);
    }
}
