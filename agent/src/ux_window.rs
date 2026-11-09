//! Esconde a janela da UI oficial enquanto ela ainda não pode ser fechada
//! (#286).
//!
//! O `kill-ux` sai só depois de `ReconnectConfig::kill_ux_delay` (30 s),
//! porque o Vanguard fecha o LoL (VAN7003) se não achar a UI oficial enquanto
//! inicializa (#214). Nesse tempo a UI precisa existir, mas o jogador não
//! precisa vê-la. Um [`UxHider`] esconde as janelas de primeiro nível dos
//! processos `LeagueClientUx` e `LeagueClientUxRender` com
//! `ShowWindowAsync(SW_HIDE)`:
//!
//! - um `SetWinEventHook` em `EVENT_OBJECT_SHOW`, fora de contexto
//!   (`WINEVENT_OUTOFCONTEXT`), pega cada janela assim que ela aparece. O
//!   aviso chega numa thread do próprio agente: nada é injetado no LoL;
//! - a cada [`SWEEP_EVERY`], uma varredura com `EnumWindows` esconde as que
//!   já estavam abertas e relê os pids pela Toolhelp, como o
//!   [`lockfile`](crate::lockfile) faz.
//!
//! Nenhum processo é aberto e nenhuma memória é lida: o pid de cada janela
//! vem de `GetWindowThreadProcessId`. Fora do Windows (o mock-lcu) não há o
//! que esconder e [`UxHider::start`] devolve [`HideError::Unsupported`]; quem
//! chama usa o `ux-minimize` da LCU como plano B.
//!
//! [`UxHider::stop`] encerra o esconderijo. Com `restore`, mostra de novo as
//! janelas escondidas, para o botão de emergência (`launch-ux`) achar a UI
//! visível. Soltar o [`UxHider`] encerra sem mostrar nada.

use std::fmt;
use std::time::Duration;

use thiserror::Error;

/// Processos da UI oficial cujas janelas são escondidas.
pub const UX_PROCESSES: [&str; 2] = ["LeagueClientUx", "LeagueClientUxRender"];

/// Intervalo da varredura que esconde as janelas que o hook não pegou.
pub const SWEEP_EVERY: Duration = Duration::from_millis(250);

/// Por que o esconderijo não começou.
#[derive(Debug, Error)]
pub enum HideError {
    #[error("esconder a janela da UI oficial só funciona no Windows")]
    Unsupported,
    #[error("o Windows recusou o hook de janelas")]
    Hook,
    #[error("não consegui criar a thread do esconderijo: {0}")]
    Thread(#[from] std::io::Error),
}

/// Se o processo `name` (com ou sem `.exe`) é da UI oficial.
pub fn is_ux_process(name: &str) -> bool {
    let base = name
        .len()
        .checked_sub(4)
        .filter(|&i| name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(".exe"))
        .map_or(name, |i| &name[..i]);
    UX_PROCESSES.iter().any(|p| base.eq_ignore_ascii_case(p))
}

type StopFn = Box<dyn FnOnce(bool) + Send>;

/// Esconde as janelas da UI oficial até [`UxHider::stop`] ou até ser solto.
pub struct UxHider {
    stop: Option<StopFn>,
}

impl fmt::Debug for UxHider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UxHider")
            .field("running", &self.stop.is_some())
            .finish()
    }
}

impl UxHider {
    /// Começa a esconder as janelas da UI oficial, inclusive as que já estão
    /// abertas.
    pub fn start() -> Result<Self, HideError> {
        #[cfg(windows)]
        {
            windows::start().map(Self::from_stop)
        }
        #[cfg(not(windows))]
        {
            Err(HideError::Unsupported)
        }
    }

    /// Esconderijo que chama `stop` ao parar. Também serve aos testes do Hub.
    #[cfg(any(windows, test))]
    pub(crate) fn from_stop(stop: impl FnOnce(bool) + Send + 'static) -> Self {
        Self {
            stop: Some(Box::new(stop)),
        }
    }

    /// Para de esconder. Com `restore`, mostra de novo as janelas que este
    /// esconderijo escondeu e que ainda existem.
    pub fn stop(mut self, restore: bool) {
        if let Some(stop) = self.stop.take() {
            stop(restore);
        }
    }
}

impl Drop for UxHider {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop(false);
        }
    }
}

/// O esconderijo no Windows: uma thread com o hook, um timer e a fila de
/// mensagens que os dois precisam.
///
/// Junto com `lockfile::windows`, é o único `unsafe` do agente: as chamadas
/// da API do Windows. Só se mexe em janelas (esconder e mostrar); nenhum
/// processo é aberto.
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};

    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetAncestor, GetMessageW, GetWindowThreadProcessId, IsWindow, IsWindowVisible,
        KillTimer, PeekMessageW, PostThreadMessageW, SetTimer, ShowWindowAsync, CHILDID_SELF,
        EVENT_OBJECT_SHOW, GA_ROOT, MSG, OBJID_WINDOW, PM_NOREMOVE, SW_HIDE, SW_SHOW,
        WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_QUIT, WM_TIMER,
    };

    use super::{is_ux_process, HideError, SWEEP_EVERY};

    /// Estado da thread do esconderijo. O hook fora de contexto e o timer
    /// chegam na própria thread, então não precisa de trava.
    #[derive(Default)]
    struct State {
        /// Pids dos processos da UI oficial na última leitura.
        ux_pids: HashSet<u32>,
        /// Pids já vistos que não são da UI, até a próxima leitura.
        others: HashSet<u32>,
        /// Janelas escondidas, para mostrar de novo no `restore`.
        hidden: Vec<isize>,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::default();
    }

    pub(super) fn start() -> Result<impl FnOnce(bool) + Send + 'static, HideError> {
        let restore = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_restore = Arc::clone(&restore);
        std::thread::Builder::new()
            .name("easylobby-ux-hider".into())
            .spawn(move || run(&ready_tx, &thread_restore))?;
        let thread_id = ready_rx.recv().map_err(|_| HideError::Hook)??;
        Ok(move |show_again: bool| {
            restore.store(show_again, Ordering::SeqCst);
            // SAFETY: só posta uma mensagem; se a thread já acabou, falha sem efeito.
            unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
        })
    }

    fn run(ready: &mpsc::Sender<Result<u32, HideError>>, restore: &AtomicBool) {
        let mut msg = MSG::default();
        // SAFETY: as chamadas recebem ponteiros válidos (`msg` vive até o fim)
        // ou nulos onde a API aceita. O hook e o timer são desfeitos antes de
        // a thread acabar.
        unsafe {
            // Cria a fila de mensagens antes de devolver o id da thread, para
            // o WM_QUIT não se perder.
            PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
            let hook = SetWinEventHook(
                EVENT_OBJECT_SHOW,
                EVENT_OBJECT_SHOW,
                std::ptr::null_mut(),
                Some(on_show),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if hook.is_null() {
                let _ = ready.send(Err(HideError::Hook));
                return;
            }
            let timer = SetTimer(
                std::ptr::null_mut(),
                0,
                u32::try_from(SWEEP_EVERY.as_millis()).unwrap_or(250),
                None,
            );
            let _ = ready.send(Ok(GetCurrentThreadId()));
            sweep();

            loop {
                let got = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                if got == 0 || got == -1 || msg.message == WM_QUIT {
                    break;
                }
                if msg.message == WM_TIMER {
                    sweep();
                }
            }

            if timer != 0 {
                KillTimer(std::ptr::null_mut(), timer);
            }
            UnhookWinEvent(hook);
            if restore.load(Ordering::SeqCst) {
                let hidden = STATE.with(|s| std::mem::take(&mut s.borrow_mut().hidden));
                for hwnd in hidden {
                    let hwnd = hwnd as HWND;
                    if IsWindow(hwnd) != 0 {
                        ShowWindowAsync(hwnd, SW_SHOW);
                    }
                }
            }
        }
    }

    /// Relê os pids da UI e esconde as janelas dela que estão visíveis.
    fn sweep() {
        refresh_pids();
        // SAFETY: `enum_window` não guarda o `HWND` além da chamada.
        unsafe { EnumWindows(Some(enum_window), 0) };
    }

    fn refresh_pids() {
        let ux_pids = crate::lockfile::process_names()
            .into_iter()
            .filter(|(_, name)| is_ux_process(name))
            .map(|(pid, _)| pid)
            .collect();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.ux_pids = ux_pids;
            s.others.clear();
        });
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, _: LPARAM) -> BOOL {
        // SAFETY: o `hwnd` vem do próprio EnumWindows.
        unsafe { hide_if_ux(hwnd, false) };
        1
    }

    unsafe extern "system" fn on_show(
        _hook: HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        id_object: i32,
        id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        if hwnd.is_null() || id_object != OBJID_WINDOW || id_child != CHILDID_SELF as i32 {
            return;
        }
        // SAFETY: o `hwnd` vem do aviso do Windows; uma janela que já
        // fechou só faz as chamadas falharem.
        unsafe {
            if GetAncestor(hwnd, GA_ROOT) == hwnd {
                hide_if_ux(hwnd, true);
            }
        }
    }

    /// Esconde `hwnd` se ela é visível e de um processo da UI. Com
    /// `may_refresh`, um pid desconhecido faz reler a lista (a UI pode ter
    /// acabado de abrir).
    unsafe fn hide_if_ux(hwnd: HWND, may_refresh: bool) {
        // SAFETY: `hwnd` é uma janela que o Windows acabou de dar.
        unsafe {
            if IsWindowVisible(hwnd) == 0 {
                return;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 || !is_ux_pid(pid, may_refresh) {
                return;
            }
            if ShowWindowAsync(hwnd, SW_HIDE) != 0 {
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    if !s.hidden.contains(&(hwnd as isize)) {
                        s.hidden.push(hwnd as isize);
                    }
                });
            }
        }
    }

    fn is_ux_pid(pid: u32, may_refresh: bool) -> bool {
        let known = STATE.with(|s| {
            let s = s.borrow();
            if s.ux_pids.contains(&pid) {
                Some(true)
            } else if s.others.contains(&pid) {
                Some(false)
            } else {
                None
            }
        });
        match known {
            Some(answer) => answer,
            None if may_refresh => {
                refresh_pids();
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    let ux = s.ux_pids.contains(&pid);
                    if !ux {
                        s.others.insert(pid);
                    }
                    ux
                })
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn reconhece_os_processos_da_ui() {
        assert!(is_ux_process("LeagueClientUx.exe"));
        assert!(is_ux_process("LeagueClientUxRender.exe"));
        assert!(is_ux_process("leagueclientux"));
        assert!(!is_ux_process("LeagueClient.exe"));
        assert!(!is_ux_process("RiotClientServices.exe"));
        assert!(!is_ux_process("League of Legends.exe"));
    }

    #[cfg(not(windows))]
    #[test]
    fn fora_do_windows_nao_esconde() {
        assert!(matches!(UxHider::start(), Err(HideError::Unsupported)));
    }

    #[test]
    fn soltar_para_sem_mostrar() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        drop(UxHider::from_stop(move |r| log.lock().unwrap().push(r)));
        let log = Arc::clone(&seen);
        UxHider::from_stop(move |r| log.lock().unwrap().push(r)).stop(true);
        assert_eq!(*seen.lock().unwrap(), vec![false, true]);
    }
}
