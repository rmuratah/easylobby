//! Atualização do LoL em andamento (#575).
//!
//! O hub guarda as duas metades que a LCU manda separadas (o patcher e a
//! trava da fila no gameflow) e publica o resumo de [`lol_update::summarize`]
//! em `lolUpdate`. Ao conectar, as duas são relidas pelo REST; depois, os
//! eventos de cada uma mudam só a sua metade.
//!
//! Enquanto o LoL mexe nos arquivos do jogo, a guarda da UI oficial não a
//! fecha (veja `ux_guard`): o `kill-ux` adiado da conexão espera o patch
//! acabar. Quando ele acaba, o agente relê as filas (que reabrem) e as notas
//! da atualização (que mudaram de versão) e, se a UI oficial subiu nesse
//! meio-tempo, fecha de novo.

use super::Hub;
use crate::lol_update::{self, LolUpdate, PatcherState};
use crate::rest::LcuClient;
use crate::ws::protocol::ServerMessage;

/// As duas metades do estado do patch, como a LCU mandou por último.
#[derive(Debug, Default)]
pub(super) struct LolUpdateSync {
    patcher: PatcherState,
    blocks_queue: bool,
}

impl Hub {
    /// Publica `update`. Quando um patch em andamento acaba, relê o que ele
    /// pode ter mudado.
    pub fn set_lol_update(&self, update: Option<LolUpdate>) {
        let mut finished = false;
        self.update(|shared| {
            if shared.state.lol_update == update {
                return None;
            }
            let was_running = shared
                .state
                .lol_update
                .as_ref()
                .is_some_and(LolUpdate::in_progress);
            let running = update.as_ref().is_some_and(LolUpdate::in_progress);
            finished = was_running && !running && shared.client.is_some();
            shared.state.lol_update = update.clone();
            Some(ServerMessage::LolUpdate { lol_update: update })
        });
        if finished {
            tracing::info!("a atualização do LoL terminou");
            self.spawn_refresh_queues();
            self.spawn_refresh_patch_notes();
            // A UI oficial pode ter subido durante o patch.
            self.guard_ux();
        }
    }

    /// Muda uma das metades e publica o resumo.
    fn edit_lol_update(&self, edit: impl FnOnce(&mut LolUpdateSync)) {
        let update = {
            let mut shared = self.lock();
            edit(&mut shared.lol_update);
            lol_update::summarize(shared.lol_update.patcher, shared.lol_update.blocks_queue)
        };
        self.set_lol_update(update);
    }

    /// Evento do patcher.
    pub(super) fn apply_patcher_event(&self, data: &serde_json::Value) {
        if let Some(patcher) = PatcherState::from_json(data) {
            self.edit_lol_update(|sync| sync.patcher = patcher);
        }
    }

    /// Evento da disponibilidade do gameflow.
    pub(super) fn apply_availability_event(&self, data: &serde_json::Value) {
        let blocks_queue = lol_update::blocked_by_patch(data);
        self.edit_lol_update(|sync| sync.blocks_queue = blocks_queue);
    }

    /// Relê as duas metades ao conectar. Um endpoint que não responder fica
    /// como parado: o próximo evento corrige.
    pub(super) async fn refresh_lol_update(&self, client: &LcuClient, session: u64) {
        let (patcher, blocked) = tokio::join!(
            lol_update::patcher_state(client),
            lol_update::queue_blocked(client),
        );
        if self.lock().session != session {
            return;
        }
        let patcher = patcher.ok().flatten().unwrap_or_default();
        let blocked = blocked.unwrap_or(false);
        self.edit_lol_update(|sync| {
            sync.patcher = patcher;
            sync.blocks_queue = blocked;
        });
    }

    /// Limpa o estado quando o LoL fecha.
    pub(super) fn clear_lol_update(&self) {
        self.lock().lol_update = LolUpdateSync::default();
        self.set_lol_update(None);
    }

    /// Se o LoL está mexendo nos arquivos do jogo agora.
    pub(super) fn patching(shared: &super::Shared) -> bool {
        shared
            .state
            .lol_update
            .as_ref()
            .is_some_and(LolUpdate::in_progress)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::tests::event;
    use super::*;
    use crate::lol_update::{LolUpdateStage, AVAILABILITY_PATH, PATCH_STATE_PATH};
    use crate::wamp::EventType;

    #[tokio::test]
    async fn eventos_do_patcher_e_da_fila_viram_o_aviso() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();

        hub.apply(event(
            PATCH_STATE_PATH,
            EventType::Update,
            json!({"action": "Patching", "isUpdateAvailable": true, "isUpToDate": false, "percentPatched": 10.0}),
        ))
        .await;
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::LolUpdate {
                lol_update: Some(LolUpdate {
                    stage: LolUpdateStage::Downloading,
                    percent: Some(10),
                    blocks_queue: false,
                })
            }
        );

        hub.apply(event(
            AVAILABILITY_PATH,
            EventType::Update,
            json!({"isAvailable": false, "state": "Patching"}),
        ))
        .await;
        assert!(hub.state().lol_update.unwrap().blocks_queue);
        rx.try_recv().unwrap();

        // O mesmo estado de novo não vira mensagem.
        hub.apply(event(
            AVAILABILITY_PATH,
            EventType::Update,
            json!({"isAvailable": false, "state": "Patching"}),
        ))
        .await;
        assert!(rx.try_recv().is_err());

        // Patch terminou: o patcher para e a fila destrava.
        hub.apply(event(
            PATCH_STATE_PATH,
            EventType::Update,
            json!({"action": "Idle", "isUpdateAvailable": false, "isUpToDate": true, "percentPatched": 100.0}),
        ))
        .await;
        hub.apply(event(
            AVAILABILITY_PATH,
            EventType::Update,
            json!({"isAvailable": true, "state": "Available"}),
        ))
        .await;
        assert_eq!(hub.state().lol_update, None);
    }

    #[tokio::test]
    async fn conexao_caindo_limpa_o_aviso() {
        use crate::connection::{ConnectionEvent, DisconnectReason};

        let hub = Hub::new();
        hub.apply(event(
            AVAILABILITY_PATH,
            EventType::Update,
            json!({"isAvailable": false, "state": "Patching"}),
        ))
        .await;
        assert!(hub.state().lol_update.is_some());
        hub.apply(ConnectionEvent::Disconnected(DisconnectReason::Closed))
            .await;
        assert_eq!(hub.state().lol_update, None);
        // A metade guardada também foi limpa: um evento do patcher parado
        // não traz o aviso de volta.
        hub.apply(event(
            PATCH_STATE_PATH,
            EventType::Update,
            json!({"action": "Idle", "isUpToDate": true}),
        ))
        .await;
        assert_eq!(hub.state().lol_update, None);
    }
}
