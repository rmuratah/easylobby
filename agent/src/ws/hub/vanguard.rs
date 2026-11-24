//! Aviso de Vanguard desconectado (#579). Veja [`crate::vanguard`].
//!
//! O agente lê o estado ao conectar e no login, e aplica os eventos do WAMP
//! no mesmo URI. Com o LoL fechado, o estado volta a `null`.
//!
//! O sinal do vigia do Vanguard (#661) também chega aqui, e é ele que libera
//! o primeiro `kill-ux` (veja `ux_guard`).

use std::time::Duration;

use super::Hub;
use crate::gameflow::GameflowPhase;
use crate::launcher::{self, LaunchOptions};
use crate::vanguard::{self, Vanguard};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

/// Quanto [`Hub::restart_league`] espera o LoL fechar antes de abrir de novo.
const QUIT_WAIT: Duration = Duration::from_secs(30);
/// Intervalo entre as olhadas na lista de processos enquanto o LoL fecha.
const QUIT_POLL: Duration = Duration::from_millis(500);

/// Fases em que fechar o LoL faz mal: na seleção é esquiva e na partida o
/// jogador perde a reconexão.
fn restart_hurts(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        Some(
            GameflowPhase::ReadyCheck
                | GameflowPhase::ChampSelect
                | GameflowPhase::GameStart
                | GameflowPhase::InProgress
                | GameflowPhase::Reconnect
        )
    )
}

impl Hub {
    /// O Vanguard respondeu que não está conectado (#579). Sem resposta da
    /// LCU não bloqueia: uma versão sem o endpoint travaria a fila.
    pub(super) fn vanguard_blocks_queue(&self) -> bool {
        self.lock()
            .state
            .vanguard
            .as_ref()
            .is_some_and(|v| !v.connected)
    }

    pub fn set_vanguard(&self, vanguard: Option<Vanguard>) {
        self.update(|shared| {
            if shared.state.vanguard == vanguard {
                return None;
            }
            if let Some(v) = vanguard.as_ref().filter(|v| !v.connected) {
                tracing::warn!(
                    "Vanguard desconectado (state {}, vanguardStatus {:?}): a partida não vai abrir",
                    v.state,
                    v.status
                );
            }
            // A folga do primeiro kill-ux conta de quando ele conectou.
            let connected = vanguard.as_ref().is_some_and(|v| v.connected);
            let was_connected = shared.state.vanguard.as_ref().is_some_and(|v| v.connected);
            if !connected {
                shared.ux_guard.vanguard_ready_at = None;
                // O vigia vai ter de se ligar de novo (#661).
                shared.ux_guard.forget_watcher();
            } else if !was_connected {
                shared.ux_guard.vanguard_ready_at = Some(tokio::time::Instant::now());
            }
            shared.state.vanguard = vanguard.clone();
            Some(ServerMessage::Vanguard { vanguard })
        });
    }

    /// O vigia do Vanguard regravou o ajuste dele (#661); veja
    /// [`vanguard::WATCHER_SIGNAL_PATH`]. No sinal que o confirma, o primeiro
    /// `kill-ux` fica liberado.
    pub(super) fn note_vanguard_watcher(&self, now: tokio::time::Instant) {
        let mut shared = self.lock();
        if !shared.ux_guard.note_watcher(now) {
            return;
        }
        if let Some(ready_at) = shared.ux_guard.vanguard_ready_at {
            tracing::info!(
                "vigia do Vanguard ligado, {} s depois de o Vanguard conectar",
                now.saturating_duration_since(ready_at).as_secs()
            );
        }
    }

    /// Fecha o LoL e abre de novo (#579). Com o Vanguard caído no meio da
    /// sessão (código 7003), só reabrir o LoL resolveu.
    pub async fn restart_league(&self, launch: &LaunchOptions) -> CommandResult {
        let (client, phase) = {
            let shared = self.lock();
            (shared.client.clone(), shared.state.gameflow_phase)
        };
        let Some(client) = client else {
            return CommandResult::error(ErrorCode::NotConnected, "o LoL não está aberto");
        };
        if restart_hurts(phase) {
            return CommandResult::error(
                ErrorCode::InvalidCommand,
                "não dá para reabrir o LoL com partida encontrada, na seleção de campeões nem durante a partida",
            );
        }
        tracing::info!("reabrindo o LoL a pedido do jogador");
        if let Err(error) = crate::ux::quit_client(&client).await {
            return CommandResult::error(ErrorCode::Lcu, error.to_string());
        }
        let deadline = tokio::time::Instant::now() + QUIT_WAIT;
        while still_running().await {
            if tokio::time::Instant::now() >= deadline {
                return CommandResult::error(
                    ErrorCode::LaunchFailed,
                    "o LoL não fechou a tempo; abra de novo pelo Riot Client",
                );
            }
            tokio::time::sleep(QUIT_POLL).await;
        }
        match launcher::launch_league(launch).await {
            Ok(_) => CommandResult::Ok,
            Err(e) => CommandResult::error(ErrorCode::LaunchFailed, e.to_string()),
        }
    }

    /// Relê o estado do Vanguard numa tarefa separada. Uma falha (LoL
    /// subindo) deixa o que já estava; o evento ou o login corrigem.
    pub(super) fn spawn_refresh_vanguard(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            let Ok(read) = vanguard::current(&client).await else {
                return;
            };
            if hub.lock().session != session {
                return;
            }
            hub.set_vanguard(read);
        });
    }
}

/// Se ainda há um `LeagueClient` ou `LeagueClientUx` rodando.
async fn still_running() -> bool {
    tokio::task::spawn_blocking(|| {
        crate::lockfile::list_processes()
            .iter()
            .any(|p| p.is_client())
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::wamp::EventType;
    use serde_json::json;

    #[test]
    fn nao_reabre_na_selecao_nem_na_partida() {
        // Sem responder a partida encontrada, o LoL conta como recusa.
        assert!(restart_hurts(Some(GameflowPhase::ReadyCheck)));
        assert!(restart_hurts(Some(GameflowPhase::ChampSelect)));
        assert!(restart_hurts(Some(GameflowPhase::InProgress)));
        assert!(!restart_hurts(Some(GameflowPhase::Lobby)));
        assert!(!restart_hurts(None));
    }

    #[tokio::test]
    async fn nao_entra_na_fila_sem_o_vanguard() {
        let hub = Hub::new();
        hub.apply(event(
            vanguard::SESSION_PATH,
            EventType::Update,
            json!({"state": "NOT_CONNECTED", "vanguardStatus": 3}),
        ))
        .await;
        match hub.start_queue().await {
            CommandResult::Error { code, message } => {
                assert_eq!(code, ErrorCode::InvalidCommand);
                assert!(message.contains("Vanguard"));
            }
            other => panic!("entrou na fila sem o Vanguard: {other:?}"),
        }
    }

    #[tokio::test]
    async fn reabrir_sem_lol_responde_not_connected() {
        let hub = Hub::new();
        assert!(matches!(
            hub.restart_league(&LaunchOptions::default()).await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn evento_do_vanguard_atualiza_o_estado() {
        let hub = Hub::new();
        hub.apply(event(
            vanguard::SESSION_PATH,
            EventType::Update,
            json!({"state": "NOT_CONNECTED", "vanguardStatus": 3}),
        ))
        .await;
        let v = hub.lock().state.vanguard.clone().unwrap();
        assert!(!v.connected);
        assert_eq!(v.state, "NOT_CONNECTED");

        hub.apply(event(
            vanguard::SESSION_PATH,
            EventType::Update,
            json!({"state": "CONNECTED", "vanguardStatus": 0}),
        ))
        .await;
        assert!(hub.lock().state.vanguard.as_ref().unwrap().connected);

        hub.apply(event(
            vanguard::SESSION_PATH,
            EventType::Delete,
            json!(null),
        ))
        .await;
        assert_eq!(hub.lock().state.vanguard, None);
    }
}
