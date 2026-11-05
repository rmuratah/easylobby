//! Shell Tauri 2 do easylobby.
//!
//! Por enquanto só abre a janela com a UI. Bandeja (#14) e a integração com o
//! agente (#15) entram nas próximas issues.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o easylobby");
}
