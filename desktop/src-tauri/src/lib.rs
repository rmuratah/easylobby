//! Shell Tauri 2 com janela, bandeja e canal local do agente.

use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::ws::protocol::CommandResult;
use easylobby_agent::ws::{self, Hub, WsConfig};

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Quanto o Sair espera a LCU reabrir a UI oficial antes de fechar mesmo
/// assim (#89).
const LAUNCH_UX_TIMEOUT: Duration = Duration::from_secs(3);

/// Quantas vezes seguidas o canal do agente é subido de novo antes de
/// desistir e avisar o jogador (#91).
const MAX_RESTARTS: u32 = 3;

/// Espera antes de cada nova tentativa, multiplicada pelo número da tentativa.
const RESTART_DELAY: Duration = Duration::from_secs(2);

/// Um canal que ficou de pé por esse tempo zera a conta de falhas: quedas
/// raras, espalhadas por horas, não esgotam as tentativas.
const HEALTHY_RUN: Duration = Duration::from_secs(60);

/// O [`Hub`] do canal que está rodando, para o Sair pedir o `launch-ux`.
/// Cada vez que o canal sobe de novo ele ganha um `Hub` novo, para a UI não
/// receber o estado da conexão anterior.
#[derive(Default)]
struct Channel {
    hub: Mutex<Option<Hub>>,
}

impl Channel {
    fn hub(&self) -> Option<Hub> {
        self.hub.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set_hub(&self, hub: Option<Hub>) {
        *self.hub.lock().unwrap_or_else(|e| e.into_inner()) = hub;
    }
}

fn show_main_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}

fn open_from_tray(app: &AppHandle) {
    if let Err(error) = show_main_window(app) {
        eprintln!("erro ao abrir a janela: {error}");
    }
}

/// Caixa de erro que não trava a thread que chamou. Em release não há
/// console, então é o único jeito de o jogador ver o motivo.
fn show_error(app: &AppHandle, message: String, on_close: impl FnOnce(bool) + Send + 'static) {
    eprintln!("{message}");
    app.dialog()
        .message(message)
        .title("easylobby")
        .kind(MessageDialogKind::Error)
        .show(on_close);
}

fn credential_source() -> CredentialSource {
    if std::env::var("EASYLOBBY_MOCK_LCU").as_deref() == Ok("1") {
        CredentialSource::Lockfile(
            std::env::temp_dir()
                .join("easylobby-mock-lcu")
                .join("lockfile"),
        )
    } else {
        CredentialSource::Discover
    }
}

fn bind_channel() -> std::io::Result<TcpListener> {
    let listener = TcpListener::bind(ws::default_addr())?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Roda o canal do agente uma vez, numa tarefa própria para um pânico não
/// passar despercebido, e devolve o motivo de ele ter parado.
async fn run_channel(app: &AppHandle, listener: TcpListener, source: CredentialSource) -> String {
    let hub = Hub::new();
    app.state::<Channel>().set_hub(Some(hub.clone()));
    let task = tauri::async_runtime::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener)?;
        let lcu = LcuConnection::new(ReconnectConfig::with_source(source));
        ws::run_with_hub(lcu, listener, WsConfig::default(), hub).await
    });
    let reason = match task.await {
        Ok(Ok(())) => "o canal parou sem erro".to_owned(),
        Ok(Err(error)) => error.to_string(),
        Err(error) => format!("a tarefa do canal falhou: {error}"),
    };
    app.state::<Channel>().set_hub(None);
    reason
}

/// Mantém o canal do agente de pé: se ele cair, sobe de novo; se não
/// conseguir depois de [`MAX_RESTARTS`] tentativas, avisa o jogador em vez de
/// deixar a UI em "Reconectando" para sempre (#91).
async fn supervise_channel(app: AppHandle, listener: TcpListener, source: CredentialSource) {
    let mut listener = Some(listener);
    let mut failures = 0;
    loop {
        if let Some(listener) = listener.take() {
            let started = Instant::now();
            let reason = run_channel(&app, listener, source.clone()).await;
            if started.elapsed() >= HEALTHY_RUN {
                failures = 0;
            }
            eprintln!("o canal do agente parou: {reason}");
            failures += 1;
            if failures > MAX_RESTARTS {
                show_error(
                    &app,
                    format!(
                        "O canal do agente parou e não voltou: {reason}\n\n\
                         Feche o easylobby pela bandeja e abra de novo."
                    ),
                    |_| {},
                );
                return;
            }
        }
        tokio::time::sleep(RESTART_DELAY * failures).await;
        match bind_channel() {
            Ok(fresh) => listener = Some(fresh),
            Err(error) => {
                eprintln!("erro ao reabrir o canal do agente: {error}");
                failures += 1;
                if failures > MAX_RESTARTS {
                    show_error(
                        &app,
                        format!(
                            "O canal do agente parou e não foi possível reabrir a porta {}: \
                             {error}\n\nFeche o easylobby pela bandeja e abra de novo.",
                            ws::DEFAULT_PORT
                        ),
                        |_| {},
                    );
                    return;
                }
            }
        }
    }
}

/// Sair pela bandeja: reabre a UI oficial antes de fechar, senão o LoL fica
/// rodando sem nenhuma janela (#89). Se a LCU não responder em
/// [`LAUNCH_UX_TIMEOUT`], fecha mesmo assim.
fn quit(app: &AppHandle) {
    static QUITTING: AtomicBool = AtomicBool::new(false);
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let hub = app.state::<Channel>().hub();
        if let Some(hub) = hub.filter(|hub| hub.client().is_some()) {
            match tokio::time::timeout(LAUNCH_UX_TIMEOUT, hub.launch_ux()).await {
                Ok(CommandResult::Ok) => {}
                Ok(CommandResult::Error { message, .. }) => {
                    eprintln!("erro ao reabrir a UI oficial: {message}");
                }
                Err(_) => eprintln!("a LCU não respondeu ao launch-ux a tempo"),
            }
        }
        app.exit(0);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Precisa ser o primeiro plugin: a segunda instância mostra a janela
        // da primeira e sai antes de tentar a porta do canal (#90).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            open_from_tray(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(Channel::default())
        .setup(|app| {
            let handle = app.handle().clone();
            // Reserve a porta antes de abrir a UI: não conecte silenciosamente
            // a outro processo que já esteja usando o endereço. Outra
            // instância do easylobby já foi tratada pelo single-instance;
            // aqui sobra outro programa na porta, e o jogador precisa saber.
            let listener = match bind_channel() {
                Ok(listener) => listener,
                Err(error) => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                    let exit = handle.clone();
                    show_error(
                        &handle,
                        format!(
                            "O easylobby não conseguiu abrir a porta {} do canal do agente: \
                             {error}\n\nOutro programa pode estar usando essa porta.",
                            ws::DEFAULT_PORT
                        ),
                        move |_| exit.exit(1),
                    );
                    return Ok(());
                }
            };
            tauri::async_runtime::spawn(supervise_channel(handle, listener, credential_source()));

            let open = MenuItem::with_id(app, "open", "Abrir easylobby", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit_item])?;
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
                    "quit" => quit(app),
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
