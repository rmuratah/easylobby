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

/// Estado da guarda dentro do [`Hub`].
#[derive(Debug, Default)]
pub(super) struct UxGuard {
    /// O agente fechou a UI oficial nesta conexão e o jogador não a reabriu.
    pub(super) active: bool,
    /// Muda a cada sequência armada; uma sequência mais nova encerra as
    /// anteriores.
    pub(super) generation: u64,
    /// Esconderijo da janela da UI oficial em andamento, com o número que o
    /// identifica (#286).
    hider: Option<(u64, UxHider)>,
    /// Número do último esconderijo.
    hider_id: u64,
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

/// Se a guarda pode fechar a UI agora: numa fase que o app cobre e fora do
/// fim da seleção.
fn may_kill(shared: &super::Shared) -> bool {
    covered(shared.state.gameflow_phase) && !game_starting_now(shared)
}

/// [`game_starting`] com a seleção atual, agora.
fn game_starting_now(shared: &super::Shared) -> bool {
    game_starting(
        shared.state.champ_select.as_ref(),
        crate::champ_select::now_ms(),
    )
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
            (client, shared.session, shared.ux_guard.generation)
        };
        let hub = self.clone();
        runtime.spawn(async move {
            let start = Instant::now();
            // A UI leva alguns segundos para subir e fechar: escondida, o
            // jogador não a vê nesse meio-tempo.
            let hider = hub.start_hiding();
            let mut stopped = false;
            for delay in RECLOSE_AFTER {
                tokio::time::sleep_until(start + delay).await;
                match hub.guard_step(session, generation) {
                    GuardStep::Stop => {
                        stopped = true;
                        break;
                    }
                    // Fim da seleção ou reconexão: só esconde.
                    GuardStep::Hide => {}
                    // Uma falha não importa: a próxima tentativa cobre.
                    GuardStep::Kill => {
                        let _ = ux::kill_ux(&client).await;
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
        runtime.spawn(async move {
            // A UI precisa existir para o Vanguard, mas não precisa aparecer.
            let hider = hub.start_hiding();
            if hider.is_none() {
                let _ = ux::minimize_ux(&client).await;
            }
            tokio::time::sleep(delay).await;
            loop {
                match hub.take_scheduled_kill(session, generation) {
                    ScheduledKill::Cancelled => break,
                    ScheduledKill::Wait => tokio::time::sleep(KILL_UX_RETRY).await,
                    ScheduledKill::Kill => {
                        // Se falhar, a guarda (agora ligada) fecha na próxima
                        // vez que o LoL pedir a janela.
                        let _ = ux::kill_ux(&client).await;
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

    /// Decide o fechamento adiado. Em [`ScheduledKill::Kill`], já liga a
    /// guarda.
    fn take_scheduled_kill(&self, session: u64, generation: u64) -> ScheduledKill {
        let mut shared = self.lock();
        if shared.session != session || shared.ux_guard.generation != generation {
            return ScheduledKill::Cancelled;
        }
        if !may_kill(&shared) {
            return ScheduledKill::Wait;
        }
        shared.ux_guard.active = true;
        shared.ux_guard.generation += 1;
        ScheduledKill::Kill
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
        } else if !covered(phase) {
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
