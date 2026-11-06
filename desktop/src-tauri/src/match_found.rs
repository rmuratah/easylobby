//! Traz a janela para a frente quando a partida é encontrada: o jogador pode
//! estar em outra janela, ou com o easylobby minimizado ou na bandeja.

use easylobby_agent::ready_check::{ReadyCheck, ReadyCheckResponse, ReadyCheckState};
use easylobby_agent::ws::protocol::ServerMessage;
use easylobby_agent::ws::Hub;
use tauri::{AppHandle, Manager, UserAttentionType};
use tokio::sync::broadcast::error::RecvError;

/// Ready check esperando a resposta do jogador.
fn waiting_for_answer(ready_check: Option<&ReadyCheck>) -> bool {
    ready_check.is_some_and(|check| {
        check.state == ReadyCheckState::InProgress
            && check.player_response == ReadyCheckResponse::Pending
    })
}

/// Lembra se a partida atual já trouxe a janela para a frente. O ready check
/// chega de segundo em segundo; só o primeiro de cada partida conta.
#[derive(Debug, Default)]
struct Detector {
    raised: bool,
}

impl Detector {
    /// `true` quando a janela deve vir para a frente agora.
    fn update(&mut self, ready_check: Option<&ReadyCheck>) -> bool {
        let waiting = waiting_for_answer(ready_check);
        let raise = waiting && !self.raised;
        self.raised = waiting;
        raise
    }
}

/// Mostra, restaura e foca a janela. O Windows só deixa um programa tomar o
/// foco em alguns casos; por isso a janela fica por cima de tudo por um
/// instante e, se mesmo assim não ganhar o foco, o ícone pisca na barra de
/// tarefas.
fn raise_main_window(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window.show()?;
    window.unminimize()?;
    window.set_always_on_top(true)?;
    let focused = window.set_focus();
    window.set_always_on_top(false)?;
    focused?;
    if !window.is_focused()? {
        window.request_user_attention(Some(UserAttentionType::Critical))?;
    }
    Ok(())
}

/// Acompanha os ready checks do `hub` até ele ser trocado ou parar.
pub async fn watch(app: AppHandle, hub: &Hub) {
    let (state, mut events) = hub.subscribe();
    let mut detector = Detector::default();
    // Partida que já estava aberta quando o canal subiu também avisa.
    let mut raise = detector.update(state.ready_check.as_ref());
    loop {
        if raise {
            if let Err(error) = raise_main_window(&app) {
                eprintln!("erro ao trazer a janela para a frente: {error}");
            }
        }
        raise = match events.recv().await {
            Ok(ServerMessage::ReadyCheck { ready_check }) => detector.update(ready_check.as_ref()),
            Ok(_) => false,
            // Perdeu eventos: confere o estado atual.
            Err(RecvError::Lagged(_)) => detector.update(hub.state().ready_check.as_ref()),
            Err(RecvError::Closed) => return,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(state: ReadyCheckState, player_response: ReadyCheckResponse) -> ReadyCheck {
        ReadyCheck {
            state,
            player_response,
            timer: 1.0,
            duration: 13.0,
        }
    }

    #[test]
    fn traz_a_janela_uma_vez_por_partida() {
        let mut detector = Detector::default();
        let waiting = check(ReadyCheckState::InProgress, ReadyCheckResponse::Pending);
        assert!(!detector.update(None));
        assert!(detector.update(Some(&waiting)));
        // O timer chega de segundo em segundo: não repete.
        assert!(!detector.update(Some(&waiting)));
        let accepted = check(ReadyCheckState::InProgress, ReadyCheckResponse::Accepted);
        assert!(!detector.update(Some(&accepted)));
        assert!(!detector.update(None));
        // Alguém recusou e a fila achou outra partida.
        assert!(detector.update(Some(&waiting)));
    }

    #[test]
    fn nao_traz_a_janela_com_resultado_final() {
        let mut detector = Detector::default();
        for state in [
            ReadyCheckState::EveryoneReady,
            ReadyCheckState::StrangerNotReady,
            ReadyCheckState::Error,
        ] {
            assert!(!detector.update(Some(&check(state, ReadyCheckResponse::Pending))));
        }
    }
}
