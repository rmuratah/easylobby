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
//! quando a conexão cai, e só age nas fases que o app cobre: no pós-jogo, por
//! exemplo, a UI oficial fica como estiver (#180).
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

use std::time::Duration;

use tokio::time::Instant;

use super::Hub;
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
        Some(GameflowPhase::ReadyCheck | GameflowPhase::ChampSelect)
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
        )
    )
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
            for delay in RECLOSE_AFTER {
                tokio::time::sleep_until(start + delay).await;
                if !hub.still_guarding(session, generation) {
                    break;
                }
                // Uma falha não importa: a próxima tentativa cobre.
                let _ = ux::kill_ux(&client).await;
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
        if !covered(shared.state.gameflow_phase) {
            return ScheduledKill::Wait;
        }
        shared.ux_guard.active = true;
        shared.ux_guard.generation += 1;
        ScheduledKill::Kill
    }

    /// Se a sequência `generation` da conexão `session` ainda deve fechar a UI.
    fn still_guarding(&self, session: u64, generation: u64) -> bool {
        let shared = self.lock();
        shared.ux_guard.active
            && shared.ux_guard.generation == generation
            && shared.session == session
            && covered(shared.state.gameflow_phase)
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
        assert!(!reopens_ux(Some(GameflowPhase::Matchmaking)));
        assert!(!reopens_ux(None));
        for phase in [
            GameflowPhase::Lobby,
            GameflowPhase::Matchmaking,
            GameflowPhase::ReadyCheck,
            GameflowPhase::ChampSelect,
        ] {
            assert!(covered(Some(phase)), "{phase:?}");
        }
        // No pós-jogo e na partida a UI oficial fica como estiver.
        for phase in [
            GameflowPhase::InProgress,
            GameflowPhase::WaitingForStats,
            GameflowPhase::PreEndOfGame,
            GameflowPhase::EndOfGame,
        ] {
            assert!(!covered(Some(phase)), "{phase:?}");
        }
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
