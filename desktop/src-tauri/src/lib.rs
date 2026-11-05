//! Shell Tauri 2 com janela, bandeja e canal local do agente.

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::ws::{self, WsConfig};

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

fn show_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}

fn open_from_tray(app: &tauri::AppHandle) {
    if let Err(error) = show_main_window(app) {
        eprintln!("erro ao abrir a janela: {error}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Reserve a porta antes de abrir a UI: não conecte silenciosamente
            // a outra instância se já existir um processo usando o endereço.
            let listener = std::net::TcpListener::bind(ws::default_addr())?;
            listener.set_nonblocking(true)?;
            let source = if std::env::var("EASYLOBBY_MOCK_LCU").as_deref() == Ok("1") {
                CredentialSource::Lockfile(
                    std::env::temp_dir()
                        .join("easylobby-mock-lcu")
                        .join("lockfile"),
                )
            } else {
                CredentialSource::Discover
            };
            tauri::async_runtime::spawn(async move {
                let result = async {
                    let listener = tokio::net::TcpListener::from_std(listener)?;
                    let lcu = LcuConnection::new(ReconnectConfig::with_source(source));
                    ws::run(lcu, listener, WsConfig::default()).await
                }
                .await;
                if let Err(error) = result {
                    eprintln!("erro no canal do agente: {error}");
                }
            });

            let open = MenuItem::with_id(app, "open", "Abrir easylobby", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let icon = app
                .default_window_icon()
                .ok_or_else(|| std::io::Error::other("ícone do aplicativo ausente"))?
                .clone();

            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip(format!("easylobby {}", easylobby_agent::VERSION))
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => open_from_tray(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        open_from_tray(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    match window.hide() {
                        Ok(()) => api.prevent_close(),
                        Err(error) => eprintln!("erro ao ocultar a janela: {error}"),
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o easylobby");
}
