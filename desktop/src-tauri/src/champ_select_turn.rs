//! Traz a janela para a frente na seleção de campeões (#431): quando ela
//! começa e a cada nova vez do jogador de banir ou escolher. O jogador pode
//! estar em outra janela, ou com o easylobby minimizado ou na bandeja.

use easylobby_agent::champ_select::ChampSelect;
use easylobby_agent::ws::protocol::ServerMessage;
use easylobby_agent::ws::Hub;
use tauri::AppHandle;
use tokio::sync::broadcast::error::RecvError;

use crate::match_found::raise_main_window;

/// Lembra a seleção e a vez do jogador que já trouxeram a janela. A seleção
/// chega a cada mudança (hover, timer, picks dos outros); só uma seleção
/// nova ou uma vez nova contam.
#[derive(Debug, Default)]
struct Detector {
    session: Option<String>,
    action: Option<u32>,
}

impl Detector {
    /// `true` quando a janela deve vir para a frente agora.
    fn update(&mut self, champ_select: Option<&ChampSelect>) -> bool {
        let Some(champ_select) = champ_select else {
            *self = Self::default();
            return false;
        };
        self.observe(
            &champ_select.id,
            champ_select.pending_action.as_ref().map(|a| a.id),
        )
    }

    fn observe(&mut self, session: &str, action: Option<u32>) -> bool {
        let entered = self.session.as_deref() != Some(session);
        let new_turn = action.is_some() && (entered || action != self.action);
        if entered {
            self.session = Some(session.to_owned());
        }
        self.action = action;
        entered || new_turn
    }
}

/// Acompanha a seleção de campeões do `hub` até ele ser trocado ou parar.
pub async fn watch(app: AppHandle, hub: &Hub) {
    let (state, mut events) = hub.subscribe();
    let mut detector = Detector::default();
    // Seleção que já estava aberta quando o canal subiu também avisa.
    let mut raise = detector.update(state.champ_select.as_ref());
    loop {
        if raise {
            if let Err(error) = raise_main_window(&app) {
                eprintln!("erro ao trazer a janela para a frente: {error}");
            }
        }
        raise = match events.recv().await {
            Ok(ServerMessage::ChampSelect { champ_select }) => {
                detector.update(champ_select.as_ref())
            }
            Ok(_) => false,
            // Perdeu eventos: confere o estado atual.
            Err(RecvError::Lagged(_)) => detector.update(hub.state().champ_select.as_ref()),
            Err(RecvError::Closed) => return,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traz_a_janela_ao_entrar_e_a_cada_vez_do_jogador() {
        let mut detector = Detector::default();
        // Entrou na seleção (planejamento, sem vez ainda).
        assert!(detector.observe("a", None));
        assert!(!detector.observe("a", None));
        // Vez de banir; o hover não repete.
        assert!(detector.observe("a", Some(1)));
        assert!(!detector.observe("a", Some(1)));
        // Confirmou o ban; os outros jogam.
        assert!(!detector.observe("a", None));
        // Vez de escolher.
        assert!(detector.observe("a", Some(7)));
        assert!(!detector.observe("a", Some(7)));
    }

    #[test]
    fn uma_selecao_nova_avisa_de_novo() {
        let mut detector = Detector::default();
        assert!(detector.observe("a", Some(1)));
        // Dodge e nova partida: outra seleção, mesmo id de ação.
        assert!(detector.observe("b", Some(1)));
    }

    #[test]
    fn sair_da_selecao_esquece_tudo() {
        let mut detector = Detector::default();
        assert!(detector.observe("a", None));
        assert!(!detector.update(None));
        assert!(detector.observe("a", None));
    }
}
