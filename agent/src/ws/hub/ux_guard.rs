//! Mantém a UI oficial fechada quando o próprio LoL a reabre (#188).
//!
//! O agente fecha a UI oficial ao conectar (`kill-ux`, #12), mas o LoL a
//! abre de novo sozinho em alguns momentos: na partida encontrada, quando o
//! jogador aceita e na entrada da seleção de campeões, o client pede para
//! mostrar a janela principal e, sem a UI, sobe o `LeagueClientUx` outra vez.
//!
//! Enquanto a UI oficial estiver fechada pelo agente, cada um desses momentos
//! arma uma sequência curta de `kill-ux` (que não faz mal com a UI já
//! fechada), porque a UI leva alguns segundos para subir. A guarda desliga
//! quando o jogador reabre a UI pelo botão de emergência (`launchUx`) e
//! quando a conexão cai, e só age nas fases que o app cobre: no pós-jogo, por
//! exemplo, a UI oficial fica como estiver (#180).

use std::time::Duration;

use tokio::time::Instant;

use super::Hub;
use crate::gameflow::GameflowPhase;
use crate::ux;

/// Quando fechar a UI de novo, contando do momento em que o LoL pode ter
/// começado a reabri-la.
pub(super) const RECLOSE_AFTER: [Duration; 6] = [
    Duration::from_millis(300),
    Duration::from_millis(1_500),
    Duration::from_secs(3),
    Duration::from_secs(6),
    Duration::from_secs(10),
    Duration::from_secs(15),
];

/// Estado da guarda dentro do [`Hub`].
#[derive(Debug, Default)]
pub(super) struct UxGuard {
    /// O agente fechou a UI oficial nesta conexão e o jogador não a reabriu.
    pub(super) active: bool,
    /// Muda a cada sequência armada; uma sequência mais nova encerra as
    /// anteriores.
    pub(super) generation: u64,
}

/// Fases em que o LoL costuma reabrir a UI oficial ao entrar.
pub(super) fn reopens_ux(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        Some(GameflowPhase::ReadyCheck | GameflowPhase::ChampSelect)
    )
}

/// Se um pedido `/riotclient/ux-state/request` é para mostrar a UI (por
/// exemplo `{"state": "ShowMain"}`). Os pedidos para esconder ou minimizar
/// não reabrem nada e são ignorados, para o próprio `kill-ux` não armar a
/// guarda de novo.
pub(super) fn asks_to_show(data: &serde_json::Value) -> bool {
    data["state"]
        .as_str()
        .is_some_and(|state| state.to_ascii_lowercase().starts_with("show"))
}

/// Fases que o app substitui. Fora delas a guarda não fecha a UI.
fn covered(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        None | Some(
            GameflowPhase::Idle
                | GameflowPhase::Lobby
                | GameflowPhase::Matchmaking
                | GameflowPhase::ReadyCheck
                | GameflowPhase::ChampSelect
        )
    )
}

impl Hub {
    /// Liga ou desliga a guarda. Ligada quando o agente fechou a UI oficial
    /// ao conectar; desligada quando o jogador a reabre ou a conexão cai.
    pub(super) fn set_ux_guard(&self, active: bool) {
        let mut shared = self.lock();
        shared.ux_guard.active = active;
        shared.ux_guard.generation += 1;
    }

    /// O LoL pode estar reabrindo a UI oficial: fecha de novo algumas vezes
    /// nos próximos segundos, se a guarda estiver ligada.
    pub(super) fn guard_ux(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (client, session, generation) = {
            let mut shared = self.lock();
            if !shared.ux_guard.active {
                return;
            }
            let Some(client) = shared.client.clone() else {
                return;
            };
            shared.ux_guard.generation += 1;
            (client, shared.session, shared.ux_guard.generation)
        };
        let hub = self.clone();
        runtime.spawn(async move {
            let start = Instant::now();
            for delay in RECLOSE_AFTER {
                tokio::time::sleep_until(start + delay).await;
                if !hub.still_guarding(session, generation) {
                    return;
                }
                // Uma falha não importa: a próxima tentativa cobre.
                let _ = ux::kill_ux(&client).await;
            }
        });
    }

    /// Se a sequência `generation` da conexão `session` ainda deve fechar a UI.
    fn still_guarding(&self, session: u64, generation: u64) -> bool {
        let shared = self.lock();
        shared.ux_guard.active
            && shared.ux_guard.generation == generation
            && shared.session == session
            && covered(shared.state.gameflow_phase)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::connection::{ConnectionEvent, DisconnectReason};
    use crate::wamp::EventType;
    use serde_json::json;

    #[test]
    fn fases_que_reabrem_e_fases_cobertas() {
        assert!(reopens_ux(Some(GameflowPhase::ReadyCheck)));
        assert!(reopens_ux(Some(GameflowPhase::ChampSelect)));
        assert!(!reopens_ux(Some(GameflowPhase::Matchmaking)));
        assert!(!reopens_ux(None));
        for phase in [
            GameflowPhase::Lobby,
            GameflowPhase::Matchmaking,
            GameflowPhase::ReadyCheck,
            GameflowPhase::ChampSelect,
        ] {
            assert!(covered(Some(phase)), "{phase:?}");
        }
        // No pós-jogo e na partida a UI oficial fica como estiver.
        for phase in [
            GameflowPhase::InProgress,
            GameflowPhase::WaitingForStats,
            GameflowPhase::PreEndOfGame,
            GameflowPhase::EndOfGame,
        ] {
            assert!(!covered(Some(phase)), "{phase:?}");
        }
    }

    #[test]
    fn so_pedidos_para_mostrar_armam() {
        assert!(asks_to_show(&json!({"state": "ShowMain", "requestId": 3})));
        assert!(asks_to_show(&json!({"state": "Show"})));
        assert!(!asks_to_show(&json!({"state": "HideAll"})));
        assert!(!asks_to_show(&json!({"state": "MinimizeAll"})));
        assert!(!asks_to_show(&json!(null)));
    }

    #[tokio::test]
    async fn emergencia_e_queda_desligam_a_guarda() {
        let hub = Hub::new();
        assert!(!hub.lock().ux_guard.active);
        hub.set_ux_guard(true);
        assert!(hub.lock().ux_guard.active);
        // Sem LoL o launchUx falha, mas o jogador pediu a UI: a guarda sai.
        let _ = hub.launch_ux().await;
        assert!(!hub.lock().ux_guard.active);

        hub.set_ux_guard(true);
        hub.apply(ConnectionEvent::Disconnected(DisconnectReason::Closed))
            .await;
        assert!(!hub.lock().ux_guard.active);
    }

    #[tokio::test]
    async fn sem_conexao_nada_e_armado() {
        let hub = Hub::new();
        hub.set_ux_guard(true);
        let before = hub.lock().ux_guard.generation;
        hub.apply(event(
            crate::ux::UX_STATE_REQUEST_PATH,
            EventType::Create,
            json!({"state": "ShowMain"}),
        ))
        .await;
        hub.apply(event(
            super::super::GAMEFLOW_PHASE_PATH,
            EventType::Update,
            json!("ReadyCheck"),
        ))
        .await;
        assert_eq!(hub.lock().ux_guard.generation, before);
    }
}
