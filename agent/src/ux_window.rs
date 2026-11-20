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
//! Escondida, a UI ainda toca os sons dela: no fim da seleção ela fica de pé
//! uns 10 s (veja `ws::hub::ux_guard`, #342) e toca os sons da seleção. Por
//! isso, enquanto o esconderijo durar, as sessões de áudio dos mesmos
//! processos ficam no mudo pelo mixer de volume do Windows (Core Audio,
//! `ISimpleAudioVolume`, #566), como o jogador faria no "Mixer de volume".
//! Um aviso do Windows (`IAudioSessionNotification`) pega cada sessão nova
//! assim que ela nasce, e a varredura pega as que já existiam. O mudo dura
//! enquanto houver um esconderijo; quando o último para, as sessões que o
//! agente silenciou voltam a ter som. Uma sessão que já estava no mudo fica
//! como estava.
//!
//! O Windows guarda o mudo por executável: o Chromium da UI cria as sessões
//! sem `AUDCLNT_STREAMFLAGS_NOPERSIST`, e uma UI que fecha no mudo abre no
//! mudo da próxima vez, até sem o easylobby (#621). Por isso o som tem de
//! voltar com a UI ainda viva: [`release_mute`] devolve o som antes de cada
//! `kill-ux`, `launch-ux` e `quit` (veja [`crate::ux`]), e os processos
//! liberados ficam [`RELEASE_FOR`] sem voltar ao mudo, o tempo de fecharem.
//! Um arquivo na pasta de configuração ([`CLEAN_FILE`]) diz que o agente
//! não deixou nenhum mudo salvo. Sem ele (o app caiu com a UI no mudo, ou o
//! mudo ficou salvo pela 0.1.3), uma sessão da UI que já nasce no mudo conta
//! como silenciada pelo agente e ganha o som de volta na próxima liberação.
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

/// Quanto um processo da UI liberado por [`release_mute`] fica sem voltar ao
/// mudo: o tempo de o `kill-ux` ou o `quit` fecharem a UI (#621). Se ele
/// ainda existir depois disso, o esconderijo o silencia de novo.
pub const RELEASE_FOR: Duration = Duration::from_secs(10);

/// Arquivo, na [pasta de configuração](crate::riot_api::config_dir), que diz
/// que nenhum mudo da UI oficial ficou salvo no Windows pelo agente (#621).
pub const CLEAN_FILE: &str = "ux-mute-ok";

/// Quanto [`UxHider::stop`] espera a thread do esconderijo acabar (e devolver
/// o som), para o app não sair antes disso.
#[cfg(windows)]
const STOP_WAIT: Duration = Duration::from_secs(1);

/// Devolve o som das sessões de áudio da UI oficial que o agente pôs no mudo,
/// com a UI ainda viva, para o Windows não guardar o mudo (#621). Chame
/// antes de fechar a UI ou o LoL e antes de reabrir a UI. Os processos
/// liberados ficam [`RELEASE_FOR`] sem voltar ao mudo. Fora do Windows não
/// faz nada.
pub fn release_mute() {
    #[cfg(windows)]
    windows::release_mute();
}

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

/// Para o esconderijo: `(restore, wait)`. Com `wait`, espera a thread acabar.
type StopFn = Box<dyn FnOnce(bool, bool) + Send>;

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
            windows::start().map(|stop| Self {
                stop: Some(Box::new(stop)),
            })
        }
        #[cfg(not(windows))]
        {
            Err(HideError::Unsupported)
        }
    }

    /// Esconderijo que chama `stop` ao parar. Serve aos testes do Hub.
    #[cfg(test)]
    pub(crate) fn from_stop(stop: impl FnOnce(bool) + Send + 'static) -> Self {
        Self {
            stop: Some(Box::new(move |restore, _wait| stop(restore))),
        }
    }

    /// Para de esconder. Com `restore`, mostra de novo as janelas que este
    /// esconderijo escondeu e que ainda existem. Espera até [`STOP_WAIT`] a
    /// thread acabar, para o som da UI já ter voltado quando o app sair.
    pub fn stop(mut self, restore: bool) {
        if let Some(stop) = self.stop.take() {
            stop(restore, true);
        }
    }
}

impl Drop for UxHider {
    /// Solto, para sem mostrar nada e sem esperar: o [`Hub`](crate::ws::Hub)
    /// solta o esconderijo velho com a trava na mão.
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop(false, false);
        }
    }
}

/// O esconderijo no Windows: uma thread com o hook, um timer e a fila de
/// mensagens que os dois precisam.
///
/// Junto com `lockfile::windows`, é o único `unsafe` do agente: as chamadas
/// da API do Windows. Só se mexe em janelas (esconder e mostrar) e no mudo
/// das sessões de áudio da UI (#566); nenhum processo é aberto.
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

    use super::{is_ux_process, HideError, STOP_WAIT, SWEEP_EVERY};

    pub(super) use mute::release as release_mute;

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
        /// O mudo das sessões de áudio da UI (#566), se o Core Audio respondeu.
        muter: Option<mute::Muter>,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::default();
    }

    pub(super) fn start() -> Result<impl FnOnce(bool, bool) + Send + 'static, HideError> {
        let restore = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel();
        // Fecha quando a thread acaba (o `Sender` cai junto com ela).
        let (done_tx, done_rx) = mpsc::channel::<()>();
        let thread_restore = Arc::clone(&restore);
        std::thread::Builder::new()
            .name("easylobby-ux-hider".into())
            .spawn(move || {
                let _done = done_tx;
                run(&ready_tx, &thread_restore);
            })?;
        let thread_id = ready_rx.recv().map_err(|_| HideError::Hook)??;
        Ok(move |show_again: bool, wait: bool| {
            restore.store(show_again, Ordering::SeqCst);
            // SAFETY: só posta uma mensagem; se a thread já acabou, falha sem efeito.
            unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
            if wait {
                let _ = done_rx.recv_timeout(STOP_WAIT);
            }
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
            // O mudo começa antes de o esconderijo valer: um esconderijo
            // novo que substitui um velho já conta antes de o velho parar.
            let com = mute::init_com();
            let muter = if com { mute::Muter::start() } else { None };
            STATE.with(|s| s.borrow_mut().muter = muter);
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
            if let Some(muter) = STATE.with(|s| s.borrow_mut().muter.take()) {
                muter.finish();
            }
            if com {
                mute::uninit_com();
            }
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
        STATE.with(|s| {
            let s = s.borrow();
            if let Some(muter) = &s.muter {
                muter.mute_ux(|pid| s.ux_pids.contains(&pid));
            }
        });
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

    /// O mudo das sessões de áudio da UI oficial pelo Core Audio (#566).
    ///
    /// Só usa o mixer de volume do Windows: as sessões vêm do serviço de
    /// áudio, e o pid de cada uma, de `IAudioSessionControl2::GetProcessId`.
    /// Nenhum processo do LoL é aberto.
    mod mute {
        use std::path::PathBuf;
        use std::sync::{Mutex, MutexGuard, PoisonError};
        use std::time::Instant;

        use windows::core::{implement, Interface, Ref, Result};
        use windows::Win32::Media::Audio::{
            eRender, IAudioSessionControl, IAudioSessionControl2, IAudioSessionManager2,
            IAudioSessionNotification, IAudioSessionNotification_Impl, IMMDeviceEnumerator,
            ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
        };
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        };

        use super::super::{is_ux_process, CLEAN_FILE, RELEASE_FOR};

        /// Quantos esconderijos estão silenciando, os pids que o agente pôs
        /// no mudo (para devolver o som) e os que ele liberou há pouco.
        struct Shared {
            active: usize,
            muted: Vec<u32>,
            /// Pids liberados por [`release`], com quando: não voltam ao mudo
            /// por [`RELEASE_FOR`] (#621).
            released: Vec<(u32, Instant)>,
            /// Ficou mudo salvo de antes ([`CLEAN_FILE`] ausente): uma sessão
            /// da UI que nasce no mudo conta como silenciada pelo agente.
            adopt: bool,
        }

        static SHARED: Mutex<Shared> = Mutex::new(Shared {
            active: 0,
            muted: Vec::new(),
            released: Vec::new(),
            adopt: false,
        });

        fn shared() -> MutexGuard<'static, Shared> {
            SHARED.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// O arquivo que diz que nenhum mudo ficou salvo (#621).
        fn clean_file() -> Option<PathBuf> {
            crate::riot_api::config_dir().map(|dir| dir.join(CLEAN_FILE))
        }

        /// Se ficou mudo salvo de antes. Sem pasta de configuração, confia
        /// que não (o comportamento de antes).
        fn left_muted() -> bool {
            clean_file().is_some_and(|file| !file.exists())
        }

        /// Anota que o agente deixou (`false`) ou não deixou (`true`) mudo
        /// salvo. Um erro só faz a próxima abertura conferir de novo.
        fn mark_clean(clean: bool) {
            let Some(file) = clean_file() else {
                return;
            };
            let result = if clean {
                file.parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| std::fs::write(&file, b""))
            } else {
                match std::fs::remove_file(&file) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    other => other,
                }
            };
            if let Err(error) = result {
                tracing::warn!("erro ao anotar o mudo da UI oficial: {error}");
            }
        }

        /// Entra no COM (MTA) nesta thread. `false` se não deu.
        pub(super) fn init_com() -> bool {
            // SAFETY: sem ponteiros; desfeito por `uninit_com` na mesma thread.
            unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() }
        }

        /// Sai do COM nesta thread, depois de soltar todas as interfaces.
        pub(super) fn uninit_com() {
            // SAFETY: par do `init_com` que deu certo nesta thread.
            unsafe { CoUninitialize() };
        }

        /// As sessões de áudio de cada saída de som ativa, com o aviso de
        /// sessão nova inscrito em cada uma.
        pub(super) struct Muter {
            managers: Vec<IAudioSessionManager2>,
            notification: IAudioSessionNotification,
        }

        impl Muter {
            /// Começa a silenciar. `None` se o Core Audio não respondeu; aí
            /// o esconderijo segue só com as janelas.
            pub(super) fn start() -> Option<Self> {
                let managers = managers().ok()?;
                {
                    let mut shared = shared();
                    if shared.active == 0 {
                        shared.adopt = left_muted();
                        if shared.adopt {
                            tracing::info!(
                                "o mudo da UI oficial pode ter ficado salvo no Windows: \
                                 o som volta na próxima liberação (#621)"
                            );
                        }
                    }
                    shared.active += 1;
                }
                let notification: IAudioSessionNotification = OnSessionCreated.into();
                for manager in &managers {
                    // SAFETY: `notification` vive no `Muter` até o `finish`,
                    // que desinscreve antes de soltar.
                    let _ = unsafe { manager.RegisterSessionNotification(&notification) };
                }
                Some(Self {
                    managers,
                    notification,
                })
            }

            /// Põe no mudo as sessões que já existem dos processos `is_ux`.
            /// Pedir a lista também liga o aviso de sessão nova, que o
            /// Windows só manda depois do primeiro pedido.
            pub(super) fn mute_ux(&self, is_ux: impl Fn(u32) -> bool) {
                for_each_session(&self.managers, |session| {
                    if let Some(pid) = session_pid(session).filter(|&pid| is_ux(pid)) {
                        mute(session, pid);
                    }
                });
            }

            /// Para de silenciar. Se era o último esconderijo, devolve o som
            /// às sessões que o agente silenciou.
            pub(super) fn finish(self) {
                for manager in &self.managers {
                    // SAFETY: a mesma inscrição do `start`.
                    let _ = unsafe { manager.UnregisterSessionNotification(&self.notification) };
                }
                {
                    let mut shared = shared();
                    shared.active = shared.active.saturating_sub(1);
                    if shared.active > 0 {
                        return;
                    }
                }
                unmute_recorded(&self.managers);
            }
        }

        /// Devolve o som das sessões anotadas em `muted` e marca os pids como
        /// liberados. Se todas voltaram a ter som, anota que nenhum mudo
        /// ficou salvo; uma sessão que já não existe pode ter deixado o mudo
        /// salvo, e a próxima abertura confere.
        fn unmute_recorded(managers: &[IAudioSessionManager2]) {
            let pids = std::mem::take(&mut shared().muted);
            if pids.is_empty() {
                return;
            }
            let mut unmuted = Vec::new();
            for_each_session(managers, |session| {
                let Some(pid) = session_pid(session).filter(|pid| pids.contains(pid)) else {
                    return;
                };
                if let Ok(volume) = session.cast::<ISimpleAudioVolume>() {
                    // SAFETY: sem contexto de evento (ponteiro nulo aceito).
                    if unsafe { volume.SetMute(false, std::ptr::null()) }.is_ok() {
                        unmuted.push(pid);
                    }
                }
            });
            let clean = pids.iter().all(|pid| unmuted.contains(pid));
            if !clean {
                tracing::warn!("o som de parte da UI oficial não voltou: ela já tinha fechado");
            }
            let mut shared = shared();
            let now = Instant::now();
            shared.released.retain(|(pid, _)| !pids.contains(pid));
            shared.released.extend(pids.iter().map(|&pid| (pid, now)));
            // Com a trava, para não anotar "sem mudo" logo depois de outra
            // sessão entrar no mudo.
            if clean && shared.muted.is_empty() {
                shared.adopt = false;
                mark_clean(true);
            }
        }

        /// Devolve o som das sessões da UI que o agente pôs no mudo, numa
        /// thread própria (com o COM dela) e esperando acabar (#621).
        pub(in super::super) fn release() {
            if shared().muted.is_empty() {
                return;
            }
            let worker = std::thread::Builder::new()
                .name("easylobby-ux-unmute".into())
                .spawn(|| {
                    if !init_com() {
                        return;
                    }
                    if let Ok(managers) = managers() {
                        unmute_recorded(&managers);
                    }
                    uninit_com();
                });
            if let Ok(worker) = worker {
                let _ = worker.join();
            }
        }

        /// O gerenciador de sessões de cada saída de som ativa.
        fn managers() -> Result<Vec<IAudioSessionManager2>> {
            // SAFETY: chamadas COM com a thread já no MTA; as interfaces
            // devolvidas são contadas pelo `windows`.
            unsafe {
                let enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let devices = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
                let mut managers = Vec::new();
                for i in 0..devices.GetCount()? {
                    if let Ok(manager) = devices.Item(i).and_then(|device| {
                        device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)
                    }) {
                        managers.push(manager);
                    }
                }
                Ok(managers)
            }
        }

        fn for_each_session(
            managers: &[IAudioSessionManager2],
            mut f: impl FnMut(&IAudioSessionControl),
        ) {
            for manager in managers {
                // SAFETY: chamadas COM sem ponteiros crus.
                unsafe {
                    let Ok(sessions) = manager.GetSessionEnumerator() else {
                        continue;
                    };
                    for i in 0..sessions.GetCount().unwrap_or(0) {
                        if let Ok(session) = sessions.GetSession(i) {
                            f(&session);
                        }
                    }
                }
            }
        }

        /// Pid da sessão; `None` nas sessões de vários processos (a do
        /// sistema, por exemplo).
        fn session_pid(session: &IAudioSessionControl) -> Option<u32> {
            let session = session.cast::<IAudioSessionControl2>().ok()?;
            // SAFETY: chamada COM sem ponteiros crus.
            let pid = unsafe { session.GetProcessId() }.ok()?;
            (pid != 0).then_some(pid)
        }

        /// Põe a sessão no mudo e anota o pid. Uma sessão que já estava no
        /// mudo (pelo jogador) não é anotada, para não ganhar som depois,
        /// a menos que o agente tenha deixado mudo salvo de antes (#621). Um
        /// pid liberado há pouco fica com som: a UI dele está fechando.
        fn mute(session: &IAudioSessionControl, pid: u32) {
            let adopt = {
                let mut shared = shared();
                shared.released.retain(|(_, at)| at.elapsed() < RELEASE_FOR);
                if shared.released.iter().any(|&(p, _)| p == pid) {
                    return;
                }
                shared.adopt
            };
            let Ok(volume) = session.cast::<ISimpleAudioVolume>() else {
                return;
            };
            // SAFETY: sem contexto de evento (ponteiro nulo aceito).
            unsafe {
                if volume.GetMute().is_ok_and(|muted| muted.as_bool()) {
                    if !adopt {
                        return;
                    }
                } else if volume.SetMute(true, std::ptr::null()).is_err() {
                    return;
                }
            }
            let mut shared = shared();
            if shared.muted.is_empty() {
                // O mudo agora pode ficar salvo no Windows até o som voltar.
                mark_clean(false);
            }
            if !shared.muted.contains(&pid) {
                shared.muted.push(pid);
            }
        }

        /// Aviso do Windows de sessão nova, numa thread do próprio COM.
        #[implement(IAudioSessionNotification)]
        struct OnSessionCreated;

        impl IAudioSessionNotification_Impl for OnSessionCreated_Impl {
            fn OnSessionCreated(&self, session: Ref<'_, IAudioSessionControl>) -> Result<()> {
                let session = session.ok()?;
                let Some(pid) = session_pid(session) else {
                    return Ok(());
                };
                let is_ux = crate::lockfile::process_names()
                    .into_iter()
                    .any(|(p, name)| p == pid && is_ux_process(&name));
                if is_ux {
                    mute(session, pid);
                }
                Ok(())
            }
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
