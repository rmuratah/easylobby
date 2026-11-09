//! O que fazer com o LoL quando o jogador sai do easylobby.
//!
//! Com a UI oficial fechada pelo agente, o LoL fica rodando sem janela, e
//! sair do easylobby o deixaria lá gastando memória sem o jogador ver. Ao
//! sair, então, o agente encerra o LoL inteiro, como o botão de fechar do
//! próprio client.
//!
//! Há momentos em que encerrar faz mal: na seleção de campeões vira esquiva
//! (com penalidade), e durante a partida o jogador perde o jeito de
//! reconectar. Nesses momentos o agente reabre a UI oficial em vez de
//! encerrar, como antes (#89). E se a UI oficial está à vista (o jogador a
//! reabriu pelo botão de emergência ou desligou o kill-ux), o jogador está
//! usando o LoL por ela, e o LoL fica como está.

use super::Hub;
use crate::gameflow::GameflowPhase;
use crate::ux;

/// O que o agente faz com o LoL ao sair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitAction {
    /// O LoL não está aberto: nada a fazer.
    NotConnected,
    /// A UI oficial está à vista: o LoL fica como está.
    KeepOpen,
    /// Encerra o LoL (`/process-control/v1/process/quit`).
    QuitClient,
    /// Num momento em que encerrar faz mal: reabre a UI oficial
    /// (`launch-ux`) para o LoL não ficar sem janela.
    LaunchUx,
}

/// Se dá para encerrar o LoL nessa fase sem prejuízo para o jogador. Fases
/// desconhecidas ficam do lado seguro (não encerra).
pub(super) fn can_quit(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        None | Some(
            GameflowPhase::Idle
                | GameflowPhase::Lobby
                | GameflowPhase::Matchmaking
                | GameflowPhase::WaitingForStats
                | GameflowPhase::PreEndOfGame
                | GameflowPhase::EndOfGame
                | GameflowPhase::TerminatedInError
        )
    )
}

/// Decide o [`ExitAction`] a partir da conexão, da guarda da UI oficial e
/// da fase.
pub(super) fn exit_action(
    connected: bool,
    ux_hidden: bool,
    phase: Option<GameflowPhase>,
) -> ExitAction {
    if !connected {
        ExitAction::NotConnected
    } else if !ux_hidden {
        ExitAction::KeepOpen
    } else if can_quit(phase) {
        ExitAction::QuitClient
    } else {
        ExitAction::LaunchUx
    }
}

impl Hub {
    /// Cuida do LoL quando o jogador sai do easylobby: encerra o LoL, ou
    /// reabre a UI oficial se encerrar fizer mal agora (veja o módulo).
    /// Devolve o que fez, ou o erro da LCU.
    pub async fn close_league_on_exit(&self) -> Result<ExitAction, crate::rest::LcuError> {
        let (client, action) = {
            let shared = self.lock();
            let action = exit_action(
                shared.client.is_some(),
                shared.ux_guard.active,
                shared.state.gameflow_phase,
            );
            (shared.client.clone(), action)
        };
        let Some(client) = client else {
            return Ok(action);
        };
        match action {
            ExitAction::NotConnected | ExitAction::KeepOpen => {}
            ExitAction::QuitClient => {
                // A guarda não precisa mais fechar nada.
                self.set_ux_guard(false);
                self.stop_hiding(None, false);
                ux::quit_client(&client).await?;
            }
            ExitAction::LaunchUx => {
                self.set_ux_guard(false);
                self.stop_hiding(None, true);
                ux::launch_ux(&client).await?;
            }
        }
        Ok(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encerra_fora_da_partida_e_da_selecao() {
        for phase in [
            None,
            Some(GameflowPhase::Idle),
            Some(GameflowPhase::Lobby),
            Some(GameflowPhase::Matchmaking),
            Some(GameflowPhase::WaitingForStats),
            Some(GameflowPhase::PreEndOfGame),
            Some(GameflowPhase::EndOfGame),
            Some(GameflowPhase::TerminatedInError),
        ] {
            assert_eq!(
                exit_action(true, true, phase),
                ExitAction::QuitClient,
                "{phase:?}"
            );
        }
    }

    #[test]
    fn reabre_a_ui_quando_encerrar_faz_mal() {
        for phase in [
            GameflowPhase::CheckedIntoTournament,
            GameflowPhase::ReadyCheck,
            GameflowPhase::ChampSelect,
            GameflowPhase::GameStart,
            GameflowPhase::FailedToLaunch,
            GameflowPhase::InProgress,
            GameflowPhase::Reconnect,
            GameflowPhase::Unknown,
        ] {
            assert_eq!(
                exit_action(true, true, Some(phase)),
                ExitAction::LaunchUx,
                "{phase:?}"
            );
        }
    }

    #[test]
    fn com_a_ui_oficial_a_vista_ou_sem_lol_nao_mexe() {
        assert_eq!(
            exit_action(true, false, Some(GameflowPhase::Lobby)),
            ExitAction::KeepOpen
        );
        assert_eq!(
            exit_action(false, true, Some(GameflowPhase::Lobby)),
            ExitAction::NotConnected
        );
    }

    #[tokio::test]
    async fn sem_lol_nao_faz_nada() {
        let hub = Hub::new();
        assert_eq!(
            hub.close_league_on_exit().await.unwrap(),
            ExitAction::NotConnected
        );
    }
}
