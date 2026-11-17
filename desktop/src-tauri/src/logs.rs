//! Log de erros do app em arquivo (#534).
//!
//! Os arquivos ficam na pasta de logs do app
//! (`%LOCALAPPDATA%\io.github.rmuratah.easylobby\logs` no Windows), um por
//! dia; quem grava é [`easylobby_agent::logging`]. Daqui saem os comandos da
//! UI: gravar um erro da interface e abrir a pasta no Explorer.
//!
//! Em build de release, os erros também vão para a base do Supabase
//! ([`easylobby_agent::error_report`], #550), a não ser que o jogador desligue
//! "Enviar erros para os devs" nas Configurações.

use std::path::PathBuf;

use easylobby_agent::error_report::{self, ReportConfig, ReportSwitch};
use tauri::{AppHandle, Manager};

/// Arquivo que marca que o jogador desligou o envio de erros.
const REPORTS_OFF_FILE: &str = "enviar-erros-desligado";
/// Arquivo com o id aleatório desta instalação.
const INSTALL_ID_FILE: &str = "id-instalacao";

/// Estado do envio de erros, para os comandos da UI.
pub struct ErrorReports {
    /// `None` quando o envio não existe nesta build (desenvolvimento).
    switch: Option<ReportSwitch>,
}

/// Tamanho máximo de cada campo que a UI manda.
const MAX_UI_FIELD_CHARS: usize = 4_000;

/// Pasta dos arquivos de log.
pub fn dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_log_dir().ok()
}

fn config_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok()
}

/// Id da instalação: lido do arquivo ou criado na primeira vez.
fn install_id(dir: &std::path::Path) -> String {
    let path = dir.join(INSTALL_ID_FILE);
    if let Ok(id) = std::fs::read_to_string(&path) {
        let id = id.trim();
        if id.len() == 36 {
            return id.to_owned();
        }
    }
    let id = error_report::new_install_id();
    let _ = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, &id));
    id
}

/// Monta o envio de erros. Só em release: o `tauri dev` não manda nada.
fn error_reporter(app: &AppHandle) -> Option<(ReportConfig, ReportSwitch)> {
    if cfg!(debug_assertions) {
        return None;
    }
    let dir = config_dir(app)?;
    let config = ReportConfig::load(install_id(&dir))?;
    let enabled = !dir.join(REPORTS_OFF_FILE).exists();
    Some((config, ReportSwitch::new(enabled)))
}

/// Liga o log em arquivo e o envio de erros. Sem a pasta, o app segue sem
/// log.
pub fn init(app: &AppHandle) {
    let reporter = error_reporter(app);
    app.manage(ErrorReports {
        switch: reporter.as_ref().map(|(_, switch)| switch.clone()),
    });
    let Some(dir) = dir(app) else {
        eprintln!("easylobby: pasta de logs indisponível");
        return;
    };
    let layer = reporter.map(|(config, switch)| error_report::layer(config, switch));
    if let Err(error) = easylobby_agent::logging::init(&dir, layer) {
        eprintln!("easylobby: não deu para abrir a pasta de logs: {error}");
        return;
    }
    tracing::info!("easylobby {} iniciado", easylobby_agent::VERSION);
}

fn clip(text: &str) -> String {
    if text.chars().count() <= MAX_UI_FIELD_CHARS {
        return text.to_owned();
    }
    let cut: String = text.chars().take(MAX_UI_FIELD_CHARS).collect();
    format!("{cut}…")
}

/// Grava um erro da interface React: exceção, promessa rejeitada sem
/// tratamento ou tela que quebrou.
#[tauri::command]
pub fn log_ui_error(kind: String, message: String, stack: Option<String>) {
    let (kind, message) = (clip(&kind), clip(&message));
    match stack.as_deref().map(clip) {
        Some(stack) if !stack.is_empty() => {
            tracing::error!(target: "easylobby_ui", "[{kind}] {message}\n{stack}");
        }
        _ => tracing::error!(target: "easylobby_ui", "[{kind}] {message}"),
    }
}

/// Abre a pasta de logs no gerenciador de arquivos.
#[tauri::command]
pub fn open_logs_folder(app: AppHandle) -> Result<(), String> {
    let dir = dir(&app).ok_or("pasta de logs indisponível")?;
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    // O Explorer sai com código 1 mesmo quando abre a pasta; só a falha ao
    // iniciar conta.
    std::process::Command::new(program)
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Se "Enviar erros para os devs" está ligada. `None` quando esta build não
/// envia erros (desenvolvimento), e a opção some da tela.
#[tauri::command]
pub fn error_reports_enabled(state: tauri::State<'_, ErrorReports>) -> Option<bool> {
    state.switch.as_ref().map(ReportSwitch::is_enabled)
}

/// Liga ou desliga o envio de erros e lembra a escolha. Devolve o valor que
/// ficou.
#[tauri::command]
pub fn set_error_reports_enabled(
    app: AppHandle,
    state: tauri::State<'_, ErrorReports>,
    enabled: bool,
) -> Result<bool, String> {
    let switch = state.switch.as_ref().ok_or("envio de erros indisponível")?;
    let dir = config_dir(&app).ok_or("pasta de configuração indisponível")?;
    let marker = dir.join(REPORTS_OFF_FILE);
    let saved = if enabled {
        match std::fs::remove_file(&marker) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    } else {
        std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&marker, b""))
    };
    saved.map_err(|error| error.to_string())?;
    switch.set(enabled);
    Ok(switch.is_enabled())
}
