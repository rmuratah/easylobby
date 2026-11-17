//! Abrir o easylobby junto com o LoL (#511).
//!
//! O app inicia com o Windows, escondido na bandeja, com o argumento
//! [`HIDDEN_ARG`]. O canal do agente fica tentando conectar na LCU; quando o
//! LoL abre e a conexão sobe, [`watch`] mostra a janela, e o `kill-ux` de
//! sempre tira a UI oficial do caminho.
//!
//! A opção "Abrir junto com o LoL" das Configurações liga e desliga o início
//! com o Windows (a chave `Run` do registro, pelo `tauri-plugin-autostart`).
//! O registro é a fonte da verdade: não há outra cópia da escolha. Na
//! primeira vez que o app abre, a opção é ligada; um arquivo na pasta de
//! configuração lembra que isso já foi feito, para não ligar de novo o que o
//! jogador desligou.

use easylobby_agent::ws::protocol::{LcuStatus, ServerMessage};
use easylobby_agent::ws::Hub;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tokio::sync::broadcast::error::RecvError;

/// Argumento do início com o Windows: abre só na bandeja, sem a janela.
pub const HIDDEN_ARG: &str = "--bandeja";

/// Arquivo que marca que a opção já ganhou o valor padrão (ligada).
const DEFAULT_APPLIED_FILE: &str = "abrir-com-lol-padrao";

/// Se este processo foi aberto pelo início com o Windows.
pub fn started_hidden() -> bool {
    std::env::args().any(|arg| arg == HIDDEN_ARG)
}

/// Liga o início com o Windows na primeira vez que o app abre. Em build de
/// desenvolvimento não liga: registraria o executável do `tauri dev`.
pub fn apply_default(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let Ok(dir) = app.path().app_config_dir() else {
        return;
    };
    let marker = dir.join(DEFAULT_APPLIED_FILE);
    if marker.exists() {
        return;
    }
    if let Err(error) = app.autolaunch().enable() {
        tracing::warn!("erro ao ligar o início com o Windows: {error}");
        return;
    }
    if let Err(error) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&marker, b"")) {
        tracing::warn!("erro ao salvar a opção de abrir com o LoL: {error}");
    }
}

/// Se a opção "Abrir junto com o LoL" está ligada.
#[tauri::command]
pub fn open_with_league_enabled(app: AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

/// Liga ou desliga a opção "Abrir junto com o LoL". Devolve o valor que ficou.
#[tauri::command]
pub fn set_open_with_league(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|error| error.to_string())?;
    autolaunch.is_enabled().map_err(|error| error.to_string())
}

/// Lembra a última conexão com o LoL que já mostrou a janela.
#[derive(Debug, Default)]
struct Detector {
    shown_session: Option<u32>,
}

impl Detector {
    /// `true` quando o LoL acabou de conectar e a janela deve aparecer.
    fn update(&mut self, status: &LcuStatus) -> bool {
        match status {
            LcuStatus::Connected { session } if self.shown_session != Some(*session) => {
                self.shown_session = Some(*session);
                true
            }
            _ => false,
        }
    }
}

fn show_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // Só a janela escondida na bandeja: com ela aberta, não rouba o foco.
    if window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false) {
        return;
    }
    if let Err(error) = crate::match_found::raise_main_window(app) {
        tracing::warn!("erro ao mostrar a janela com o LoL aberto: {error}");
    }
}

/// Mostra a janela cada vez que o LoL abre, até o `hub` ser trocado ou parar.
pub async fn watch(app: AppHandle, hub: &Hub) {
    let (state, mut events) = hub.subscribe();
    let mut detector = Detector::default();
    let mut show = detector.update(&state.lcu);
    loop {
        if show {
            show_window(&app);
        }
        show = match events.recv().await {
            Ok(ServerMessage::Lcu { status }) => detector.update(&status),
            Ok(_) => false,
            Err(RecvError::Lagged(_)) => detector.update(&hub.state().lcu),
            Err(RecvError::Closed) => return,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retrying() -> LcuStatus {
        LcuStatus::Retrying {
            attempt: 1,
            retry_in_ms: 1000,
            client_not_running: true,
            error: String::new(),
        }
    }

    #[test]
    fn mostra_a_janela_uma_vez_por_conexao() {
        let mut detector = Detector::default();
        assert!(!detector.update(&LcuStatus::Starting));
        assert!(!detector.update(&retrying()));
        assert!(detector.update(&LcuStatus::Connected { session: 1 }));
        // O mesmo status de novo não mostra outra vez.
        assert!(!detector.update(&LcuStatus::Connected { session: 1 }));
        assert!(!detector.update(&LcuStatus::Disconnected {
            reason: "LoL fechou".to_owned()
        }));
        // O LoL abriu de novo.
        assert!(detector.update(&LcuStatus::Connected { session: 2 }));
    }
}
