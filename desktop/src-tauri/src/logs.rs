//! Log de erros do app em arquivo (#534).
//!
//! Os arquivos ficam na pasta de logs do app
//! (`%LOCALAPPDATA%\io.github.rmuratah.easylobby\logs` no Windows), um por
//! dia; quem grava é [`easylobby_agent::logging`]. Daqui saem os comandos da
//! UI: gravar um erro da interface e abrir a pasta no Explorer.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Tamanho máximo de cada campo que a UI manda.
const MAX_UI_FIELD_CHARS: usize = 4_000;

/// Pasta dos arquivos de log.
pub fn dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_log_dir().ok()
}

/// Liga o log em arquivo. Sem a pasta, o app segue sem log.
pub fn init(app: &AppHandle) {
    let Some(dir) = dir(app) else {
        eprintln!("easylobby: pasta de logs indisponível");
        return;
    };
    if let Err(error) = easylobby_agent::logging::init(&dir) {
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
