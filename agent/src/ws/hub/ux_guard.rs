//! Mantém a UI oficial fechada quando o próprio LoL a reabre (#188).
//!
//! O agente fecha a UI oficial ao conectar (`kill-ux`, #12), mas o LoL a
//! abre de novo sozinho em alguns momentos: na partida encontrada, quando o
//! jogador aceita e na entrada da seleção de campeões, o client pede para
//! mostrar a janela principal e, sem a UI, sobe o `LeagueClientUx` outra vez.
//!
//! Enquanto a UI oficial estiver fechada pelo agente, cada um desses momentos
//! arma uma sequência curta de `kill-ux` (que não faz mal com a UI já
//! fechada), porque a UI leva alguns segundos para subir. A guarda desliga
//! quando o jogador reabre a UI pelo botão de emergência (`launchUx`) e
//! quando a conexão cai, e só age nas fases que o app cobre.
//!
//! No pós-jogo (`WaitingForStats`, `PreEndOfGame`, `EndOfGame`) o app tem
//! tela própria (#180), e o LoL sobe a UI de novo assim que o jogo fecha: a
//! guarda fecha a UI ao entrar em `WaitingForStats` (#448).
//!
//! Quando o jogo fecha, o LoL passa uns segundos em `Reconnect` antes do
//! pós-jogo, já com a UI de pé. Ali a UI não é fechada, porque é por ela que
//! o jogador reconecta se o jogo caiu, mas a janela fica escondida enquanto a
//! fase durar (#448). O botão de emergência mostra a janela de novo.
//!
//! Durante a partida (`InProgress`) a guarda também fecha a UI (#349): ela
//! sobe no fim da seleção (veja abaixo) e o LoL a mostra na partida. É o que
//! a opção da Riot "fechar o client durante a partida" faz. Na reconexão
//! (`Reconnect`) a UI fica como estiver, para o jogador poder voltar.
//!
//! Ao conectar, o primeiro `kill-ux` também sai daqui, depois da espera de
//! `ReconnectConfig::kill_ux_delay` ([`Hub::schedule_kill_ux`]): fechar a UI
//! enquanto o Vanguard inicializa fecha o LoL com o erro VAN7003. Durante a
//! espera a guarda fica desligada, senão o próprio LoL abrindo a janela
//! (`ShowMain`) armaria um `kill-ux` na hora.
//!
//! Esse primeiro `kill-ux` também espera o Vanguard conectar (#579) e o
//! vigia dele se ligar à LCU (#661): com a UI fechada antes disso, o Vanguard
//! cai com o código 7003 cerca de 146 s depois de conectar, e a partida
//! seguinte vira esquiva. Veja [`vanguard_allows_kill`].
//!
//! Enquanto a UI não pode ser fechada (a espera do primeiro `kill-ux` e cada
//! sequência de fechar de novo), a janela dela fica escondida pelo
//! [`UxHider`] (#286), para o jogador não ver o client oficial. Se o
//! esconderijo não começar (fora do Windows, ou o hook recusado), a espera
//! do primeiro `kill-ux` minimiza a UI pela LCU (`ux-minimize`). O botão de
//! emergência mostra de novo as janelas escondidas antes do `launch-ux`.
//!
//! No fim da seleção a guarda não fecha a UI (#342): o LoL pede a janela
//! (`ShowMain`) uns 10 s antes da etapa `FINALIZATION` acabar, e um `kill-ux`
//! que caia na passagem para `GAME_STARTING` vira esquiva do jogador, sem
//! mensagem nenhuma. Nos últimos [`SPARE_BEFORE_GAME`] da finalização e no
//! `GAME_STARTING` a janela só fica escondida; a UI é fechada de novo quando
//! a partida começa (`InProgress`) e quando o jogador volta ao lobby. Como o
//! LoL sobe a UI nesse fim sem aviso garantido, a sequência armada na
//! seleção segue escondendo a janela até a seleção acabar (#448).
//!
//! Enquanto o LoL baixa ou repara os arquivos do jogo (#575), a guarda não
//! fecha a UI: o primeiro `kill-ux` espera o patch acabar e as sequências
//! param. O fim do patch arma uma sequência nova.

use std::time::Duration;

use tokio::time::Instant;

use super::Hub;
use crate::champ_select::{ChampSelect, ChampSelectPhase};
use crate::gameflow::GameflowPhase;
use crate::ux;
use crate::ux_window::UxHider;

/// Quando fechar a UI de novo, contando do momento em que o LoL pode ter
/// começado a reabri-la.
pub(super) const RECLOSE_AFTER: [Duration; 6] = [
    Duration::from_millis(300),
    Duration::from_millis(1_500),
    Duration::from_secs(3),
    Duration::from_secs(6),
    Duration::from_secs(10),
    Duration::from_secs(15),
];

/// Se a fase não deixa fechar a UI quando a espera acaba (por exemplo, o
/// agente abriu com a partida em andamento), olha de novo a cada intervalo.
pub(super) const KILL_UX_RETRY: Duration = Duration::from_secs(5);

/// Nos últimos instantes da etapa `FINALIZATION` a guarda não fecha a UI: o
/// `kill-ux` na passagem para `GAME_STARTING` faz o jogador esquivar (#342).
/// Folga sobre os 10 s em que o LoL pede a janela antes do fim da etapa.
pub(super) const SPARE_BEFORE_GAME: Duration = Duration::from_secs(15);

/// Folga entre o Vanguard conectar e o primeiro `kill-ux` (#579), em
/// múltiplos de `kill_ux_delay` (60 s com o padrão de 30 s). Num PC em que o
/// Vanguard levou 27 s para conectar, fechar a UI 23 s depois fez o Vanguard
/// cair com o código 7003 dois minutos mais tarde, e a partida seguinte virou
/// esquiva. Num PC em que a folga foi de 29 s, nunca caiu.
///
/// A folga sozinha não basta (#661): ela vale com o vigia do Vanguard já
/// ligado. Sem sinal dele, a espera é [`WATCHER_MISSING_FACTOR`].
const VANGUARD_SPARE_FACTOR: u32 = 2;

/// Sem sinal do vigia do Vanguard, quanto o primeiro `kill-ux` espera desde
/// que o Vanguard conectou (#661), em múltiplos de `kill_ux_delay` (180 s
/// com o padrão). Sem o vigia, o Vanguard cai de 146 a 150 s depois de
/// conectar: se passou disso conectado, o vigia se ligou e foi o sinal que
/// não chegou.
const WATCHER_MISSING_FACTOR: u32 = 6;

/// Intervalo mínimo entre o primeiro sinal do vigia do Vanguard e o sinal
/// que o confirma (#661). O vigia dá sinal a cada 30 s; uma gravação solta
/// do ajuste, ou duas seguidas, não contam como ele.
const WATCHER_MIN_GAP: Duration = Duration::from_secs(15);

/// Quanto o primeiro `kill-ux` espera, desde a conexão, por uma resposta do
/// Vanguard, em múltiplos de `kill_ux_delay` (120 s com o padrão). Sem o
/// endpoint (versão antiga do LoL, mock), fecha depois disso.
const VANGUARD_UNKNOWN_FACTOR: u32 = 4;

/// Se o Vanguard já deixa o primeiro `kill-ux` sair (#579, #661): conectado
/// há pelo menos `2 × delay` e com o vigia dele ligado (`watcher`); sem sinal
/// do vigia, conectado há pelo menos `6 × delay`; sem resposta da LCU, depois
/// de `4 × delay` desde a conexão. Desconectado, espera: fechar a UI não
/// ajuda, e o aviso da UI pede para reabrir o LoL.
pub(super) fn vanguard_allows_kill(
    vanguard: Option<&crate::vanguard::Vanguard>,
    ready_at: Option<Instant>,
    watcher: bool,
    connected_at: Instant,
    now: Instant,
    delay: Duration,
) -> bool {
    match vanguard {
        Some(v) if v.connected => {
            let factor = if watcher {
                VANGUARD_SPARE_FACTOR
            } else {
                WATCHER_MISSING_FACTOR
            };
            ready_at.is_none_or(|at| now >= at + delay * factor)
        }
        Some(_) => false,
        None => now >= connected_at + delay * VANGUARD_UNKNOWN_FACTOR,
    }
}

/// Estado da guarda dentro do [`Hub`].
#[derive(Debug, Default)]
pub(super) struct UxGuard {
    /// O agente fechou a UI oficial nesta conexão e o jogador não a reabriu.
    pub(super) active: bool,
    /// O `kill-ux` adiado desta conexão ainda vai sair: a UI oficial está
    /// aberta, mas escondida ou minimizada pelo agente, e o jogador não a
    /// pediu (#571).
    pub(super) kill_pending: bool,
    /// Muda a cada sequência armada; uma sequência mais nova encerra as
    /// anteriores.
    pub(super) generation: u64,
    /// Esconderijo da janela da UI oficial em andamento, com o número que o
    /// identifica (#286).
    hider: Option<(u64, UxHider)>,
    /// Número do último esconderijo.
    hider_id: u64,
    /// Quando o Vanguard passou a `CONNECTED` nesta conexão (#579), ou
    /// `None` com ele desconectado ou sem resposta.
    pub(super) vanguard_ready_at: Option<Instant>,
    /// Primeiro sinal do vigia do Vanguard desde que o Vanguard conectou
    /// (#661). Veja [`UxGuard::note_watcher`].
    watcher_first_at: Option<Instant>,
    /// O vigia do Vanguard deu sinal de novo: está ligado à LCU, e fechar a
    /// UI oficial não derruba mais o Vanguard (#661).
    pub(super) watcher_confirmed: bool,
}

impl UxGuard {
    /// Anota um sinal do vigia do Vanguard
    /// ([`crate::vanguard::WATCHER_SIGNAL_PATH`], #661). Devolve `true` no
    /// sinal que confirma o vigia: o primeiro que chega pelo menos
    /// [`WATCHER_MIN_GAP`] depois do primeiro de todos. Sem o Vanguard
    /// conectado o sinal não conta, porque o LoL também grava o ajuste ao
    /// subir.
    pub(super) fn note_watcher(&mut self, now: Instant) -> bool {
        if self.vanguard_ready_at.is_none() || self.watcher_confirmed {
            return false;
        }
        match self.watcher_first_at {
            None => self.watcher_first_at = Some(now),
            Some(first) if now >= first + WATCHER_MIN_GAP => self.watcher_confirmed = true,
            Some(_) => {}
        }
        self.watcher_confirmed
    }

    /// O Vanguard deixou de estar conectado: o vigia vai ter de se ligar de
    /// novo, e os sinais dele contam do zero.
    pub(super) fn forget_watcher(&mut self) {
        self.watcher_first_at = None;
        self.watcher_confirmed = false;
    }
}

/// Fases em que o LoL costuma reabrir a UI oficial ao entrar.
pub(super) fn reopens_ux(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        Some(
            GameflowPhase::ReadyCheck
                | GameflowPhase::ChampSelect
                | GameflowPhase::InProgress
                | GameflowPhase::Reconnect
                | GameflowPhase::WaitingForStats
        )
    )
}

/// Se um pedido `/riotclient/ux-state/request` é para mostrar a UI (por
/// exemplo `{"state": "ShowMain"}`). Os pedidos para esconder ou minimizar
/// não reabrem nada e são ignorados, para o próprio `kill-ux` não armar a
/// guarda de novo.
pub(super) fn asks_to_show(data: &serde_json::Value) -> bool {
    data["state"]
        .as_str()
        .is_some_and(|state| state.to_ascii_lowercase().starts_with("show"))
}

/// Fases que o app substitui. Fora delas a guarda não fecha a UI.
fn covered(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        None | Some(
            GameflowPhase::Idle
                | GameflowPhase::Lobby
                | GameflowPhase::Matchmaking
                | GameflowPhase::ReadyCheck
                | GameflowPhase::ChampSelect
                | GameflowPhase::InProgress
                | GameflowPhase::WaitingForStats
                | GameflowPhase::PreEndOfGame
                | GameflowPhase::EndOfGame
        )
    )
}

/// Fases em que a sequência da guarda segue escondendo a janela depois do
/// último passo: na seleção o LoL ainda sobe a UI antes da partida, e ali
/// ela só pode ser escondida (#342); na reconexão a UI fica para o jogador
/// reconectar, mas escondida (#448).
fn hides_until_game(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        Some(GameflowPhase::ChampSelect | GameflowPhase::GameStart | GameflowPhase::Reconnect)
    )
}

/// Se a seleção está perto de abrir a partida, quando fechar a UI oficial
/// faz o jogador esquivar (#342): na etapa `GAME_STARTING` ou nos últimos
/// [`SPARE_BEFORE_GAME`] da `FINALIZATION`. `now_ms` no relógio de
/// `ends_at_ms`.
pub(super) fn game_starting(champ_select: Option<&ChampSelect>, now_ms: u64) -> bool {
    let Some(cs) = champ_select else {
        return false;
    };
    match cs.timer.phase {
        ChampSelectPhase::GameStarting => true,
        ChampSelectPhase::Finalization => {
            let spare = u64::try_from(SPARE_BEFORE_GAME.as_millis()).unwrap_or(u64::MAX);
            !cs.timer.is_infinite && cs.timer.ends_at_ms.saturating_sub(now_ms) <= spare
        }
        _ => false,
    }
}

/// Se a guarda pode fechar a UI agora: numa fase que o app cobre, fora do
/// fim da seleção e sem o LoL atualizando os arquivos do jogo (#575).
fn may_kill(shared: &super::Shared) -> bool {
    covered(shared.state.gameflow_phase) && !game_starting_now(shared) && !Hub::patching(shared)
}

/// [`game_starting`] com a seleção atual, agora.
fn game_starting_now(shared: &super::Shared) -> bool {
    game_starting(
        shared.state.champ_select.as_ref(),
        crate::champ_select::now_ms(),
    )
}

/// Descreve, para o log, a fase do LoL e a etapa da seleção, com quanto
/// falta para ela acabar. É o que mostra, depois de uma esquiva, se o
/// `kill-ux` caiu no fim da seleção (#342).
fn ux_context(shared: &super::Shared) -> String {
    let phase = shared.state.gameflow_phase;
    match shared.state.champ_select.as_ref() {
        None => format!("fase {phase:?}, sem seleção"),
        Some(cs) if cs.timer.is_infinite => {
            format!("fase {phase:?}, seleção em {:?} sem limite", cs.timer.phase)
        }
        Some(cs) => {
            let left = cs
                .timer
                .ends_at_ms
                .saturating_sub(crate::champ_select::now_ms());
            format!(
                "fase {phase:?}, seleção em {:?} faltando {left} ms",
                cs.timer.phase
            )
        }
    }
}

/// Descreve, para o log do primeiro `kill-ux`, o que o Vanguard dizia quando
/// ele saiu: mostra se a UI fechou com o vigia ligado ou pela espera máxima
/// (#661).
fn vanguard_context(shared: &super::Shared) -> &'static str {
    match shared.state.vanguard.as_ref() {
        Some(v) if !v.connected => "Vanguard desconectado",
        Some(_) if shared.ux_guard.watcher_confirmed => "vigia do Vanguard ligado",
        Some(_) => "sem sinal do vigia do Vanguard",
        None => "sem resposta do Vanguard",
    }
}

/// Intervalo em que a sequência confere se ainda precisa esconder a janela
/// depois do último `kill-ux`.
pub(super) const HIDE_CHECK_EVERY: Duration = Duration::from_millis(500);

/// O que um passo da sequência da guarda faz.
#[derive(Debug, PartialEq, Eq)]
enum GuardStep {
    /// A sequência acabou: a guarda desligou, uma sequência mais nova
    /// assumiu, a conexão mudou ou a fase não é do app.
    Stop,
    /// Fim da seleção (#342) ou reconexão (#448): a UI não pode ser
    /// fechada, só escondida.
    Hide,
    /// Fechar a UI agora.
    Kill,
}

impl Hub {
    /// Liga ou desliga a guarda. Ligada quando o agente fechou a UI oficial
    /// ao conectar; desligada quando o jogador a reabre ou a conexão cai.
    pub(super) fn set_ux_guard(&self, active: bool) {
        let mut shared = self.lock();
        shared.ux_guard.active = active;
        shared.ux_guard.kill_pending = false;
        shared.ux_guard.generation += 1;
    }

    /// O LoL pode estar reabrindo a UI oficial: fecha de novo algumas vezes
    /// nos próximos segundos, se a guarda estiver ligada.
    pub(super) fn guard_ux(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (client, session, generation) = {
            let mut shared = self.lock();
            if !shared.ux_guard.active {
                return;
            }
            let Some(client) = shared.client.clone() else {
                return;
            };
            shared.ux_guard.generation += 1;
            tracing::info!("guarda da UI oficial armada ({})", ux_context(&shared));
            (client, shared.session, shared.ux_guard.generation)
        };
        let hub = self.clone();
        runtime.spawn(async move {
            let start = Instant::now();
            // A UI leva alguns segundos para subir e fechar: escondida, o
            // jogador não a vê nesse meio-tempo.
            let hider = hub.start_hiding();
            let mut stopped = false;
            let mut hid = false;
            for delay in RECLOSE_AFTER {
                tokio::time::sleep_until(start + delay).await;
                match hub.guard_step(session, generation) {
                    GuardStep::Stop => {
                        stopped = true;
                        break;
                    }
                    // Fim da seleção ou reconexão: só esconde.
                    GuardStep::Hide => {
                        if !hid {
                            hid = true;
                            tracing::info!(
                                "UI oficial só escondida, sem kill-ux ({})",
                                hub.ux_context_now()
                            );
                        }
                    }
                    // Uma falha não importa: a próxima tentativa cobre.
                    GuardStep::Kill => {
                        tracing::info!("kill-ux da guarda ({})", hub.ux_context_now());
                        if let Err(error) = ux::kill_ux(&client).await {
                            tracing::warn!("kill-ux da guarda falhou: {error}");
                        }
                    }
                }
            }
            // Na seleção o LoL sobe a UI uns 10 s antes da partida, quando
            // ela só pode ser escondida, e na reconexão ela fica: segue
            // escondendo até a fase acabar. A fase seguinte (`InProgress`,
            // `WaitingForStats`) arma uma sequência nova (#448).
            if !stopped && hider.is_some() {
                while hub.keeps_hiding(session, generation) {
                    tokio::time::sleep(HIDE_CHECK_EVERY).await;
                }
            }
            hub.stop_hiding(hider, false);
        });
    }

    /// Fecha a UI oficial depois de `delay`, na conexão atual, e liga a
    /// guarda. Desiste se a conexão cair ou se o jogador reabrir a UI pelo
    /// botão de emergência antes disso. Fora das fases que o app cobre
    /// (partida, pós-jogo), espera até voltar a uma delas.
    pub(super) fn schedule_kill_ux(&self, delay: Duration) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (client, session, generation) = {
            let shared = self.lock();
            let Some(client) = shared.client.clone() else {
                return;
            };
            (client, shared.session, shared.ux_guard.generation)
        };
        let hub = self.clone();
        let connected_at = Instant::now();
        runtime.spawn(async move {
            // A UI precisa existir para o Vanguard, mas não precisa aparecer.
            let hider = hub.start_hiding();
            if hider.is_none() {
                let _ = ux::minimize_ux(&client).await;
            }
            tokio::time::sleep(delay).await;
            loop {
                let now = Instant::now();
                match hub.take_scheduled_kill(session, generation, connected_at, delay, now) {
                    ScheduledKill::Cancelled => break,
                    ScheduledKill::Wait => tokio::time::sleep(KILL_UX_RETRY).await,
                    ScheduledKill::Kill => {
                        // Se falhar, a guarda (agora ligada) fecha na próxima
                        // vez que o LoL pedir a janela.
                        tracing::info!("primeiro kill-ux ({})", hub.first_kill_context_now());
                        if let Err(error) = ux::kill_ux(&client).await {
                            tracing::warn!("primeiro kill-ux falhou: {error}");
                        }
                        break;
                    }
                }
            }
            hub.stop_hiding(hider, false);
        });
    }

    /// Começa a esconder a janela da UI oficial. Devolve o número do
    /// esconderijo, ou `None` se ele não pôde começar.
    fn start_hiding(&self) -> Option<u64> {
        UxHider::start().ok().map(|hider| self.hide_with(hider))
    }

    /// Guarda `hider` como o esconderijo atual. Um anterior para sem mostrar
    /// nada: o novo cobre as mesmas janelas.
    pub(super) fn hide_with(&self, hider: UxHider) -> u64 {
        let mut shared = self.lock();
        shared.ux_guard.hider_id += 1;
        let id = shared.ux_guard.hider_id;
        // Soltar o velho só posta uma mensagem para a thread dele.
        shared.ux_guard.hider = Some((id, hider));
        id
    }

    /// Para o esconderijo `id` (`None`: qualquer um). Com `restore`, mostra
    /// de novo as janelas escondidas.
    pub(super) fn stop_hiding(&self, id: Option<u64>, restore: bool) {
        let hider = {
            let mut shared = self.lock();
            match (&shared.ux_guard.hider, id) {
                (Some((current, _)), Some(id)) if *current != id => None,
                _ => shared.ux_guard.hider.take(),
            }
        };
        if let Some((_, hider)) = hider {
            hider.stop(restore);
        }
    }

    /// Decide o fechamento adiado, no instante `now`. Em
    /// [`ScheduledKill::Kill`], já liga a guarda.
    fn take_scheduled_kill(
        &self,
        session: u64,
        generation: u64,
        connected_at: Instant,
        delay: Duration,
        now: Instant,
    ) -> ScheduledKill {
        let mut shared = self.lock();
        if shared.session != session || shared.ux_guard.generation != generation {
            return ScheduledKill::Cancelled;
        }
        if !may_kill(&shared) {
            return ScheduledKill::Wait;
        }
        // O Vanguard derruba a sessão (7003) se a UI some antes de o vigia
        // dele se ligar (#579, #661).
        if !vanguard_allows_kill(
            shared.state.vanguard.as_ref(),
            shared.ux_guard.vanguard_ready_at,
            shared.ux_guard.watcher_confirmed,
            connected_at,
            now,
            delay,
        ) {
            return ScheduledKill::Wait;
        }
        shared.ux_guard.active = true;
        shared.ux_guard.kill_pending = false;
        shared.ux_guard.generation += 1;
        ScheduledKill::Kill
    }

    /// [`ux_context`] do estado atual.
    fn ux_context_now(&self) -> String {
        ux_context(&self.lock())
    }

    /// [`ux_context`] e [`vanguard_context`] do estado atual, para o log do
    /// primeiro `kill-ux`.
    fn first_kill_context_now(&self) -> String {
        let shared = self.lock();
        format!("{}, {}", ux_context(&shared), vanguard_context(&shared))
    }

    /// Se a sequência `generation` da conexão `session` ainda vale.
    fn sequence_current(shared: &super::Shared, session: u64, generation: u64) -> bool {
        shared.ux_guard.active
            && shared.ux_guard.generation == generation
            && shared.session == session
    }

    /// O que o próximo passo da sequência `generation` da conexão `session`
    /// faz.
    fn guard_step(&self, session: u64, generation: u64) -> GuardStep {
        let shared = self.lock();
        let phase = shared.state.gameflow_phase;
        if !Self::sequence_current(&shared, session, generation) {
            GuardStep::Stop
        } else if phase == Some(GameflowPhase::Reconnect) {
            GuardStep::Hide
        } else if !covered(phase) || Self::patching(&shared) {
            // Fora do app, ou o LoL atualizando (#575): a UI fica. O fim do
            // patch arma a guarda de novo.
            GuardStep::Stop
        } else if game_starting_now(&shared) {
            GuardStep::Hide
        } else {
            GuardStep::Kill
        }
    }

    /// Se a sequência `generation` ainda deve esconder a janela depois do
    /// último passo: enquanto vale e a seleção não acabou.
    fn keeps_hiding(&self, session: u64, generation: u64) -> bool {
        let shared = self.lock();
        Self::sequence_current(&shared, session, generation)
            && hides_until_game(shared.state.gameflow_phase)
    }
}

/// O que fazer quando a espera do primeiro `kill-ux` acaba.
#[derive(Debug, PartialEq, Eq)]
enum ScheduledKill {
    /// Fechar a UI agora.
    Kill,
    /// Fase fora do app: olhar de novo depois de [`KILL_UX_RETRY`].
    Wait,
    /// A conexão caiu ou o jogador reabriu a UI.
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::connection::{ConnectionEvent, DisconnectReason};
    use crate::wamp::EventType;
    use serde_json::json;

    #[test]
    fn primeiro_kill_ux_espera_o_vanguard() {
        use crate::vanguard::Vanguard;
        let t0 = Instant::now();
        let secs = Duration::from_secs;
        let ok = Vanguard {
            connected: true,
            state: "CONNECTED".into(),
            status: Some(0),
        };
        let off = Vanguard {
            connected: false,
            state: "NOT_CONNECTED".into(),
            status: None,
        };
        let delay = crate::connection::DEFAULT_KILL_UX_DELAY;
        let ready = t0 + secs(27);
        let allows = |vanguard: Option<&Vanguard>, ready_at, watcher, now| {
            vanguard_allows_kill(vanguard, ready_at, watcher, t0, now, delay)
        };
        // Com o vigia ligado e o Vanguard conectado há pouco: espera a folga.
        assert!(!allows(Some(&ok), Some(ready), true, ready + secs(23)));
        assert!(allows(Some(&ok), Some(ready), true, ready + secs(60)));
        // Sem sinal do vigia (#661) a folga não basta: espera passar o prazo
        // em que o Vanguard cai sem ele (até 150 s depois de conectar).
        assert!(!allows(Some(&ok), Some(ready), false, ready + secs(60)));
        assert!(!allows(Some(&ok), Some(ready), false, ready + secs(151)));
        assert!(allows(Some(&ok), Some(ready), false, ready + secs(180)));
        // Desconectado: não fecha nunca.
        assert!(!allows(Some(&off), None, true, t0 + secs(600)));
        // Sem resposta da LCU: fecha depois da espera máxima.
        assert!(!allows(None, None, false, t0 + secs(30)));
        assert!(allows(None, None, false, t0 + secs(120)));
    }

    #[test]
    fn vigia_do_vanguard_precisa_de_dois_sinais() {
        let t0 = Instant::now();
        let secs = Duration::from_secs;
        let mut guard = UxGuard::default();
        // Sem o Vanguard conectado, é o LoL gravando o ajuste ao subir.
        assert!(!guard.note_watcher(t0));
        assert!(!guard.note_watcher(t0 + secs(30)));
        assert!(!guard.watcher_confirmed);

        guard.vanguard_ready_at = Some(t0 + secs(40));
        // O primeiro sinal não basta, nem outro logo em seguida.
        assert!(!guard.note_watcher(t0 + secs(41)));
        assert!(!guard.note_watcher(t0 + secs(42)));
        assert!(!guard.watcher_confirmed);
        // O sinal seguinte do vigia, 30 s depois do primeiro, confirma.
        assert!(guard.note_watcher(t0 + secs(71)));
        assert!(guard.watcher_confirmed);
        // Confirmado, os outros sinais não mudam nada.
        assert!(!guard.note_watcher(t0 + secs(101)));
        assert!(guard.watcher_confirmed);

        // O Vanguard caiu: os sinais contam do zero.
        guard.forget_watcher();
        assert!(!guard.watcher_confirmed);
        assert!(!guard.note_watcher(t0 + secs(131)));
        assert!(guard.note_watcher(t0 + secs(161)));
    }

    fn vanguard_event(state: &str) -> ConnectionEvent {
        event(
            crate::vanguard::SESSION_PATH,
            EventType::Update,
            json!({"state": state, "vanguardStatus": 0}),
        )
    }

    #[tokio::test]
    async fn evento_do_ajuste_e_o_sinal_do_vigia() {
        let hub = Hub::new();
        // O conteúdo do ajuste não é lido.
        let sinal = || {
            event(
                crate::vanguard::WATCHER_SIGNAL_PATH,
                EventType::Update,
                json!({}),
            )
        };
        // O LoL grava o ajuste ao subir, antes de o Vanguard conectar.
        hub.apply(sinal()).await;
        assert_eq!(hub.lock().ux_guard.watcher_first_at, None);

        hub.apply(vanguard_event("CONNECTED")).await;
        hub.apply(sinal()).await;
        let first = hub.lock().ux_guard.watcher_first_at;
        let first = first.expect("o sinal do vigia não contou");
        assert!(!hub.lock().ux_guard.watcher_confirmed);
        // O sinal seguinte, 30 s depois, confirma o vigia.
        hub.note_vanguard_watcher(first + Duration::from_secs(30));
        assert!(hub.lock().ux_guard.watcher_confirmed);

        // O Vanguard caiu (7003): o vigia vai ter de se ligar de novo.
        hub.apply(vanguard_event("NOT_CONNECTED")).await;
        assert!(!hub.lock().ux_guard.watcher_confirmed);
        assert_eq!(hub.lock().ux_guard.watcher_first_at, None);
    }

    /// Hub no lobby com o fechamento adiado armado e o Vanguard conectado.
    /// Devolve também a sequência atual e quando o Vanguard conectou.
    async fn com_o_vanguard_conectado() -> (Hub, u64, u64, Instant) {
        let hub = Hub::new();
        hub.apply(vanguard_event("CONNECTED")).await;
        let (session, generation, ready) = {
            let mut shared = hub.lock();
            shared.ux_guard.kill_pending = true;
            shared.state.gameflow_phase = Some(GameflowPhase::Lobby);
            let ready = shared.ux_guard.vanguard_ready_at;
            (shared.session, shared.ux_guard.generation, ready.unwrap())
        };
        (hub, session, generation, ready)
    }

    #[tokio::test]
    async fn primeiro_kill_ux_espera_o_vigia_do_vanguard() {
        let (hub, session, generation, ready) = com_o_vanguard_conectado().await;
        let delay = crate::connection::DEFAULT_KILL_UX_DELAY;
        let secs = Duration::from_secs;
        let decide =
            |after| hub.take_scheduled_kill(session, generation, ready, delay, ready + after);
        // A folga do Vanguard venceu, mas o vigia não se ligou: a primeira
        // tentativa dele falhou, e a segunda vem aos 91 s (#661).
        assert_eq!(decide(secs(60)), ScheduledKill::Wait);
        assert_eq!(decide(secs(90)), ScheduledKill::Wait);
        // Um sinal só ainda não libera.
        hub.note_vanguard_watcher(ready + secs(91));
        assert_eq!(decide(secs(95)), ScheduledKill::Wait);
        // O segundo sinal confirma o vigia, e a UI fecha.
        hub.note_vanguard_watcher(ready + secs(121));
        assert_eq!(decide(secs(125)), ScheduledKill::Kill);
        assert!(hub.lock().ux_guard.active);
        assert!(!hub.lock().ux_guard.kill_pending);
    }

    #[tokio::test]
    async fn sem_sinal_do_vigia_fecha_depois_do_prazo_do_vanguard() {
        let (hub, session, generation, ready) = com_o_vanguard_conectado().await;
        let delay = crate::connection::DEFAULT_KILL_UX_DELAY;
        let secs = Duration::from_secs;
        let decide =
            |after| hub.take_scheduled_kill(session, generation, ready, delay, ready + after);
        // Sem o vigia, o Vanguard cai até 150 s depois de conectar.
        assert_eq!(decide(secs(151)), ScheduledKill::Wait);
        // Passou disso ainda conectado: fecha, mesmo sem sinal.
        assert_eq!(decide(secs(180)), ScheduledKill::Kill);

        // Se ele caiu no prazo, a UI não fecha: o aviso pede para reabrir o
        // LoL.
        let (hub, session, generation, ready) = com_o_vanguard_conectado().await;
        hub.apply(vanguard_event("NOT_CONNECTED")).await;
        let decide =
            |after| hub.take_scheduled_kill(session, generation, ready, delay, ready + after);
        assert_eq!(decide(secs(180)), ScheduledKill::Wait);
        assert_eq!(decide(secs(600)), ScheduledKill::Wait);
    }

    #[test]
    fn fases_que_reabrem_e_fases_cobertas() {
        assert!(reopens_ux(Some(GameflowPhase::ReadyCheck)));
        assert!(reopens_ux(Some(GameflowPhase::ChampSelect)));
        // A UI sobe no fim da seleção e o LoL a mostra na partida (#349).
        assert!(reopens_ux(Some(GameflowPhase::InProgress)));
        // E sobe de novo quando o jogo fecha (#448).
        assert!(reopens_ux(Some(GameflowPhase::Reconnect)));
        assert!(reopens_ux(Some(GameflowPhase::WaitingForStats)));
        assert!(!reopens_ux(Some(GameflowPhase::GameStart)));
        assert!(!reopens_ux(Some(GameflowPhase::Matchmaking)));
        assert!(!reopens_ux(None));
        for phase in [
            GameflowPhase::Lobby,
            GameflowPhase::Matchmaking,
            GameflowPhase::ReadyCheck,
            GameflowPhase::ChampSelect,
            GameflowPhase::InProgress,
            // O pós-jogo tem tela própria (#180, #448).
            GameflowPhase::WaitingForStats,
            GameflowPhase::PreEndOfGame,
            GameflowPhase::EndOfGame,
        ] {
            assert!(covered(Some(phase)), "{phase:?}");
        }
        // Abrindo a partida e na reconexão a UI oficial fica como estiver.
        for phase in [GameflowPhase::GameStart, GameflowPhase::Reconnect] {
            assert!(!covered(Some(phase)), "{phase:?}");
        }
        // A seleção, a abertura da partida e a reconexão seguram o
        // esconderijo.
        assert!(hides_until_game(Some(GameflowPhase::ChampSelect)));
        assert!(hides_until_game(Some(GameflowPhase::GameStart)));
        assert!(hides_until_game(Some(GameflowPhase::Reconnect)));
        assert!(!hides_until_game(Some(GameflowPhase::InProgress)));
        assert!(!hides_until_game(Some(GameflowPhase::Lobby)));
        assert!(!hides_until_game(None));
    }

    /// Liga a guarda numa conexão e devolve a sequência atual.
    fn sequencia(hub: &Hub, phase: GameflowPhase) -> (u64, u64) {
        let mut shared = hub.lock();
        shared.ux_guard.active = true;
        shared.ux_guard.generation += 1;
        shared.state.gameflow_phase = Some(phase);
        (shared.session, shared.ux_guard.generation)
    }

    #[test]
    fn fim_da_selecao_so_esconde_e_segue_escondendo() {
        let hub = Hub::new();
        let (session, generation) = sequencia(&hub, GameflowPhase::ChampSelect);
        assert_eq!(hub.guard_step(session, generation), GuardStep::Kill);

        // Faltando 10 s para a partida: não fecha, mas não para (#342, #448).
        let agora = crate::champ_select::now_ms();
        let mut raw = champ_select("FINALIZATION", 1);
        raw["timer"]["adjustedTimeLeftInPhase"] = json!(10_000);
        hub.lock().state.champ_select = Some(ChampSelect::from_json_at(&raw, agora).unwrap());
        assert_eq!(hub.guard_step(session, generation), GuardStep::Hide);
        assert!(hub.keeps_hiding(session, generation));

        // A partida começou: a sequência da seleção para de esconder.
        hub.lock().state.gameflow_phase = Some(GameflowPhase::InProgress);
        assert!(!hub.keeps_hiding(session, generation));
    }

    #[test]
    fn pos_jogo_fecha_a_ui_e_reconexao_so_esconde() {
        let hub = Hub::new();
        for phase in [
            GameflowPhase::WaitingForStats,
            GameflowPhase::PreEndOfGame,
            GameflowPhase::EndOfGame,
        ] {
            let (session, generation) = sequencia(&hub, phase);
            assert_eq!(hub.guard_step(session, generation), GuardStep::Kill);
            assert!(!hub.keeps_hiding(session, generation), "{phase:?}");
        }
        // Na reconexão a UI fica, escondida, até a fase acabar.
        let (session, generation) = sequencia(&hub, GameflowPhase::Reconnect);
        assert_eq!(hub.guard_step(session, generation), GuardStep::Hide);
        assert!(hub.keeps_hiding(session, generation));
        hub.lock().state.gameflow_phase = Some(GameflowPhase::WaitingForStats);
        assert!(!hub.keeps_hiding(session, generation));
        // Fora das fases do app a sequência para.
        let (session, generation) = sequencia(&hub, GameflowPhase::GameStart);
        assert_eq!(hub.guard_step(session, generation), GuardStep::Stop);
    }

    #[test]
    fn sequencia_velha_ou_guarda_desligada_param() {
        let hub = Hub::new();
        let (session, generation) = sequencia(&hub, GameflowPhase::ChampSelect);
        // Uma sequência mais nova assumiu.
        hub.lock().ux_guard.generation += 1;
        assert_eq!(hub.guard_step(session, generation), GuardStep::Stop);
        assert!(!hub.keeps_hiding(session, generation));

        let (session, generation) = sequencia(&hub, GameflowPhase::ChampSelect);
        hub.lock().ux_guard.active = false;
        assert_eq!(hub.guard_step(session, generation), GuardStep::Stop);
        assert!(!hub.keeps_hiding(session, generation));
    }

    fn selecao(phase: &str, left_ms: u32) -> ChampSelect {
        let mut raw = champ_select(phase, 1);
        raw["timer"]["adjustedTimeLeftInPhase"] = json!(left_ms);
        ChampSelect::from_json_at(&raw, 1_000_000).unwrap()
    }

    #[test]
    fn nao_fecha_a_ui_quando_a_partida_vai_comecar() {
        let agora = 1_000_000;
        // Sem seleção, no planejamento e no começo da finalização: pode fechar.
        assert!(!game_starting(None, agora));
        assert!(!game_starting(Some(&selecao("BAN_PICK", 5_000)), agora));
        assert!(!game_starting(
            Some(&selecao("FINALIZATION", 30_000)),
            agora
        ));
        // O `ShowMain` do fim da finalização chega com 10 s restando (#342).
        assert!(game_starting(Some(&selecao("FINALIZATION", 10_000)), agora));
        assert!(game_starting(
            Some(&selecao("FINALIZATION", 30_000)),
            agora + 20_000
        ));
        assert!(game_starting(Some(&selecao("GAME_STARTING", 0)), agora));
        // Finalização sem limite (personalizada): o fim não está perto.
        let mut sem_limite = selecao("FINALIZATION", 0);
        sem_limite.timer.is_infinite = true;
        assert!(!game_starting(Some(&sem_limite), agora));
    }

    #[test]
    fn so_pedidos_para_mostrar_armam() {
        assert!(asks_to_show(&json!({"state": "ShowMain", "requestId": 3})));
        assert!(asks_to_show(&json!({"state": "Show"})));
        assert!(!asks_to_show(&json!({"state": "HideAll"})));
        assert!(!asks_to_show(&json!({"state": "MinimizeAll"})));
        assert!(!asks_to_show(&json!(null)));
    }

    #[tokio::test]
    async fn emergencia_e_queda_desligam_a_guarda() {
        let hub = Hub::new();
        assert!(!hub.lock().ux_guard.active);
        hub.set_ux_guard(true);
        assert!(hub.lock().ux_guard.active);
        // Sem LoL o launchUx falha, mas o jogador pediu a UI: a guarda sai.
        let _ = hub.launch_ux().await;
        assert!(!hub.lock().ux_guard.active);

        hub.set_ux_guard(true);
        hub.apply(ConnectionEvent::Disconnected(DisconnectReason::Closed))
            .await;
        assert!(!hub.lock().ux_guard.active);
    }

    #[tokio::test]
    async fn fechamento_adiado_pendente_ate_fechar_ou_desligar() {
        let hub = Hub::new();
        // Fechamento adiado armado: a UI ainda não foi fechada.
        {
            let mut shared = hub.lock();
            shared.ux_guard.kill_pending = true;
            shared.state.gameflow_phase = Some(GameflowPhase::Lobby);
        }
        let (session, generation) = {
            let shared = hub.lock();
            (shared.session, shared.ux_guard.generation)
        };
        let now = Instant::now();
        assert_eq!(
            hub.take_scheduled_kill(session, generation, now, Duration::ZERO, now),
            ScheduledKill::Kill
        );
        assert!(!hub.lock().ux_guard.kill_pending);
        assert!(hub.lock().ux_guard.active);

        // O jogador reabriu a UI oficial antes do fechamento.
        hub.lock().ux_guard.kill_pending = true;
        let _ = hub.launch_ux().await;
        assert!(!hub.lock().ux_guard.kill_pending);

        // O LoL caiu antes do fechamento.
        hub.lock().ux_guard.kill_pending = true;
        hub.apply(ConnectionEvent::Disconnected(DisconnectReason::Closed))
            .await;
        assert!(!hub.lock().ux_guard.kill_pending);
    }

    /// Esconderijo de mentira que anota como parou.
    fn fake_hider(log: &std::sync::Arc<std::sync::Mutex<Vec<bool>>>) -> UxHider {
        let log = std::sync::Arc::clone(log);
        UxHider::from_stop(move |restore| log.lock().unwrap().push(restore))
    }

    #[tokio::test]
    async fn emergencia_mostra_as_janelas_escondidas() {
        let hub = Hub::new();
        let log = Default::default();
        hub.hide_with(fake_hider(&log));
        let _ = hub.launch_ux().await;
        assert_eq!(*log.lock().unwrap(), vec![true]);
        assert!(hub.lock().ux_guard.hider.is_none());
    }

    #[tokio::test]
    async fn queda_para_de_esconder_sem_mostrar() {
        let hub = Hub::new();
        let log = Default::default();
        hub.hide_with(fake_hider(&log));
        hub.apply(ConnectionEvent::Disconnected(DisconnectReason::Closed))
            .await;
        assert_eq!(*log.lock().unwrap(), vec![false]);
    }

    #[test]
    fn so_o_dono_para_o_esconderijo() {
        let hub = Hub::new();
        let log = Default::default();
        let old = hub.hide_with(fake_hider(&log));
        // Um esconderijo novo substitui o velho, que para sem mostrar.
        let new = hub.hide_with(fake_hider(&log));
        assert_eq!(*log.lock().unwrap(), vec![false]);
        // A tarefa do velho não para o novo.
        hub.stop_hiding(Some(old), false);
        assert!(hub.lock().ux_guard.hider.is_some());
        hub.stop_hiding(Some(new), false);
        assert!(hub.lock().ux_guard.hider.is_none());
        assert_eq!(*log.lock().unwrap(), vec![false, false]);
    }

    #[tokio::test]
    async fn sem_conexao_nada_e_armado() {
        let hub = Hub::new();
        hub.set_ux_guard(true);
        let before = hub.lock().ux_guard.generation;
        hub.apply(event(
            crate::ux::UX_STATE_REQUEST_PATH,
            EventType::Create,
            json!({"state": "ShowMain"}),
        ))
        .await;
        hub.apply(event(
            super::super::GAMEFLOW_PHASE_PATH,
            EventType::Update,
            json!("ReadyCheck"),
        ))
        .await;
        assert_eq!(hub.lock().ux_guard.generation, before);
    }
}
