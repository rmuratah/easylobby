//! Avança as telas do fim de partida com a UI oficial fechada (#180).
//!
//! Veja [`crate::pre_end_of_game`]. Enquanto a guarda da UI estiver ligada
//! (o agente fechou a UI e o jogador não a reabriu), cada tela que aparece em
//! `currentSequenceEvent` é dada como concluída, como a UI oficial faz. Com a
//! UI oficial aberta (botão de emergência ou `kill-ux` desligado), é ela quem
//! conduz e o agente não mexe.
//!
//! A votação de honra não se perde: com a UI aberta, a tela `honor-vote`
//! também acaba em segundos e a cédula continua valendo em `EndOfGame`
//! (#182).

use super::Hub;
use crate::pre_end_of_game::{self, SequenceEvent};

/// Se o agente deve concluir a tela `event`: só com a UI oficial fechada por
/// ele e com uma tela esperando.
pub(super) fn should_advance(guard_active: bool, event: &SequenceEvent) -> Option<&str> {
    guard_active.then(|| event.pending()).flatten()
}

impl Hub {
    /// Conclui a tela `event`, se a UI oficial estiver fechada pelo agente.
    pub(super) fn advance_pre_end_of_game(&self, event: &SequenceEvent) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (client, name) = {
            let shared = self.lock();
            let Some(name) = should_advance(shared.ux_guard.active, event) else {
                return;
            };
            let Some(client) = shared.client.clone() else {
                return;
            };
            (client, name.to_owned())
        };
        runtime.spawn(async move {
            // Uma falha não trava nada: a próxima tela, a releitura da fase
            // ou o "jogar de novo" seguem valendo.
            let _ = pre_end_of_game::complete(&client, &name).await;
        });
    }

    /// Relê a tela atual e a conclui. Para quando a fase `PreEndOfGame`
    /// começa ou o agente conecta no meio dela.
    pub(super) fn spawn_advance_pre_end_of_game(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            let Ok(event) = pre_end_of_game::current(&client).await else {
                return;
            };
            if hub.lock().session == session {
                hub.advance_pre_end_of_game(&event);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tela(name: &str) -> SequenceEvent {
        SequenceEvent {
            name: name.into(),
            priority: 1,
        }
    }

    #[test]
    fn so_avanca_com_a_ui_fechada_e_uma_tela_esperando() {
        assert_eq!(
            should_advance(true, &tela("honor-vote")),
            Some("honor-vote")
        );
        assert_eq!(should_advance(false, &tela("honor-vote")), None);
        assert_eq!(should_advance(true, &tela("")), None);
    }
}
