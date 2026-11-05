//! Estado que o agente mostra para a UI e quem o mantém atualizado.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::broadcast;

use crate::connection::{ConnectionEvent, LcuConnection};
use crate::lobby::{self, Lobby, Queue, LOBBY_PATH};
use crate::rest::LcuClient;
use crate::summoner::{self, Summoner, CURRENT_SUMMONER_PATH};
use crate::ux;
use crate::wamp::{EventType, LcuEvent};

use super::protocol::{AgentState, CommandResult, ErrorCode, LcuStatus, ServerMessage};

/// Fase do gameflow (`"Lobby"`, `"ChampSelect"`...).
pub const GAMEFLOW_PHASE_PATH: &str = "/lol-gameflow/v1/gameflow-phase";

/// Quantas mensagens uma UI lenta pode ficar devendo antes de receber um
/// `hello` novo no lugar delas.
const BROADCAST_CAPACITY: usize = 256;

/// Estado atual do agente e o canal que avisa cada UI conectada quando ele
/// muda. Clonar é barato: os clones dividem o mesmo estado.
///
/// [`Hub::apply`] (ou [`drive`]) atualiza o estado a partir da
/// [`LcuConnection`]; o servidor `/ws` lê o estado e repassa as mudanças.
#[derive(Clone)]
pub struct Hub {
    inner: Arc<Inner>,
}

struct Inner {
    shared: Mutex<Shared>,
    tx: broadcast::Sender<ServerMessage>,
}

struct Shared {
    state: AgentState,
    /// Cliente REST da conexão atual, para os comandos da UI.
    client: Option<LcuClient>,
}

impl Hub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                shared: Mutex::new(Shared {
                    state: AgentState::default(),
                    client: None,
                }),
                tx,
            }),
        }
    }

    /// Cópia do estado atual.
    pub fn state(&self) -> AgentState {
        self.lock().state.clone()
    }

    /// Cliente REST da conexão atual, ou `None` com o LoL fechado.
    pub fn client(&self) -> Option<LcuClient> {
        self.lock().client.clone()
    }

    /// O estado atual e um receptor das mudanças a partir dele. Como as
    /// mudanças são publicadas com a mesma trava, nenhuma se perde nem chega
    /// repetida entre o estado e o receptor.
    pub fn subscribe(&self) -> (AgentState, broadcast::Receiver<ServerMessage>) {
        let shared = self.lock();
        (shared.state.clone(), self.inner.tx.subscribe())
    }

    /// Reabre a UI oficial pela conexão atual (comando `launchUx`, #13).
    pub async fn launch_ux(&self) -> CommandResult {
        let Some(client) = self.client() else {
            return CommandResult::error(ErrorCode::NotConnected, "o LoL não está aberto");
        };
        match ux::launch_ux(&client).await {
            Ok(()) => CommandResult::Ok,
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    pub fn set_lcu_status(&self, status: LcuStatus) {
        self.update(|shared| {
            if shared.state.lcu == status {
                return None;
            }
            shared.state.lcu = status.clone();
            Some(ServerMessage::Lcu { status })
        });
    }

    pub fn set_summoner(&self, summoner: Option<Summoner>) {
        self.update(|shared| {
            if shared.state.summoner == summoner {
                return None;
            }
            shared.state.summoner = summoner.clone();
            Some(ServerMessage::Summoner { summoner })
        });
    }

    pub fn set_gameflow_phase(&self, phase: Option<String>) {
        self.update(|shared| {
            if shared.state.gameflow_phase == phase {
                return None;
            }
            shared.state.gameflow_phase = phase.clone();
            Some(ServerMessage::GameflowPhase { phase })
        });
    }

    pub fn set_lobby(&self, lobby: Option<Lobby>) {
        self.update(|shared| {
            if shared.state.lobby == lobby {
                return None;
            }
            shared.state.lobby = lobby.clone();
            Some(ServerMessage::Lobby { lobby })
        });
    }

    /// Cria um lobby na fila `queue` ou troca a fila do lobby atual (comando
    /// `createLobby`, #17). Publica o lobby que a LCU respondeu antes de
    /// devolver, sem esperar o evento do WAMP.
    pub async fn create_lobby(&self, queue: Queue) -> CommandResult {
        let Some(client) = self.client() else {
            return not_connected();
        };
        match lobby::create_lobby(&client, queue).await {
            Ok(lobby) => {
                if lobby.is_some() {
                    self.set_lobby(lobby);
                }
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Sai do lobby (comando `leaveLobby`, #17).
    pub async fn leave_lobby(&self) -> CommandResult {
        let Some(client) = self.client() else {
            return not_connected();
        };
        match lobby::leave_lobby(&client).await {
            Ok(()) => {
                self.set_lobby(None);
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Atualiza o estado com o que a [`LcuConnection`] devolveu.
    ///
    /// Num `Connected`, relê pelo REST o invocador e a fase, porque os
    /// eventos de antes da conexão se perderam. Num `Disconnected`, limpa o
    /// que dependia do LoL.
    pub async fn apply(&self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Connected(session) => {
                let client = session.client.clone();
                self.lock().client = Some(session.client);
                self.set_lcu_status(LcuStatus::Connected {
                    session: u32::try_from(session.id).unwrap_or(u32::MAX),
                });
                let (summoner, phase, lobby) = tokio::join!(
                    summoner::current_summoner(&client),
                    client.get::<String>(GAMEFLOW_PHASE_PATH),
                    lobby::current_lobby(&client),
                );
                // 404 antes do login; qualquer outro erro também deixa vazio,
                // e o próximo evento da LCU corrige.
                self.set_summoner(summoner.ok());
                self.set_gameflow_phase(phase.ok());
                self.set_lobby(lobby.ok().flatten());
            }
            ConnectionEvent::Event(event) => self.apply_lcu_event(&event),
            ConnectionEvent::Disconnected(reason) => {
                self.lock().client = None;
                self.set_summoner(None);
                self.set_gameflow_phase(None);
                self.set_lobby(None);
                self.set_lcu_status(LcuStatus::Disconnected {
                    reason: reason.to_string(),
                });
            }
            ConnectionEvent::Retrying {
                attempt,
                delay,
                error,
            } => self.set_lcu_status(LcuStatus::Retrying {
                attempt,
                retry_in_ms: u32::try_from(delay.as_millis()).unwrap_or(u32::MAX),
                client_not_running: error.is_client_not_running(),
                error: error.to_string(),
            }),
        }
    }

    fn apply_lcu_event(&self, event: &LcuEvent) {
        let deleted = event.event_type == EventType::Delete;
        match event.uri.as_str() {
            CURRENT_SUMMONER_PATH if deleted => self.set_summoner(None),
            CURRENT_SUMMONER_PATH => {
                if let Ok(summoner) = serde_json::from_value(event.data.clone()) {
                    self.set_summoner(Some(summoner));
                }
            }
            GAMEFLOW_PHASE_PATH if deleted => self.set_gameflow_phase(None),
            GAMEFLOW_PHASE_PATH => {
                if let Some(phase) = event.data.as_str() {
                    self.set_gameflow_phase(Some(phase.to_owned()));
                }
            }
            LOBBY_PATH if deleted => self.set_lobby(None),
            LOBBY_PATH => {
                if let Some(lobby) = Lobby::from_json(&event.data) {
                    self.set_lobby(Some(lobby));
                }
            }
            _ => {}
        }
    }

    /// Muda o estado e publica a mensagem que `f` devolver, com a trava
    /// pega, para [`Hub::subscribe`] ver as duas coisas juntas.
    fn update(&self, f: impl FnOnce(&mut Shared) -> Option<ServerMessage>) {
        let mut shared = self.lock();
        if let Some(message) = f(&mut shared) {
            // Sem UI conectada o envio falha, e tudo bem.
            let _ = self.inner.tx.send(message);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        // Nenhum código segura a trava num ponto que possa entrar em pânico
        // no meio de uma mudança; o estado continua coerente.
        self.inner
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub").field("state", &self.state()).finish()
    }
}

fn not_connected() -> CommandResult {
    CommandResult::error(ErrorCode::NotConnected, "o LoL não está aberto")
}

/// Mantém o [`Hub`] atualizado com a conexão para sempre.
pub async fn drive(mut connection: LcuConnection, hub: Hub) {
    loop {
        hub.apply(connection.next().await).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(uri: &str, event_type: EventType, data: serde_json::Value) -> ConnectionEvent {
        ConnectionEvent::Event(LcuEvent {
            uri: uri.into(),
            event_type,
            data,
        })
    }

    #[tokio::test]
    async fn eventos_da_lcu_viram_mensagens() {
        let hub = Hub::new();
        let (state, mut rx) = hub.subscribe();
        assert_eq!(state, AgentState::default());

        hub.apply(event(
            GAMEFLOW_PHASE_PATH,
            EventType::Update,
            json!("Lobby"),
        ))
        .await;
        // Repetido não gera mensagem.
        hub.apply(event(
            GAMEFLOW_PHASE_PATH,
            EventType::Update,
            json!("Lobby"),
        ))
        .await;
        hub.apply(event(
            CURRENT_SUMMONER_PATH,
            EventType::Update,
            json!({
                "gameName": "Jogador",
                "tagLine": "BR1",
                "summonerLevel": 30,
                "puuid": "p",
                "summonerId": 1,
            }),
        ))
        .await;
        // Recurso que o hub não acompanha, ou JSON que não é lobby: ignorado.
        hub.apply(event(
            "/lol-lobby/v2/lobby/members",
            EventType::Update,
            json!([]),
        ))
        .await;
        hub.apply(event(LOBBY_PATH, EventType::Update, json!({})))
            .await;
        hub.apply(event(GAMEFLOW_PHASE_PATH, EventType::Delete, json!(null)))
            .await;

        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::GameflowPhase {
                phase: Some("Lobby".into())
            }
        );
        match rx.try_recv().unwrap() {
            ServerMessage::Summoner {
                summoner: Some(summoner),
            } => assert_eq!(summoner.name(), "Jogador#BR1"),
            other => panic!("esperava o invocador: {other:?}"),
        }
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::GameflowPhase { phase: None }
        );
        assert!(rx.try_recv().is_err());
        assert_eq!(hub.state().summoner.unwrap().tag_line, "BR1");
    }

    #[tokio::test]
    async fn segue_o_lobby() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let lobby = |queue_id: u32| {
            json!({
                "canStartActivity": true,
                "gameConfig": {"queueId": queue_id, "maxLobbySize": 5},
                "localMember": {"puuid": "p", "isLeader": true},
                "members": [{"puuid": "p", "summonerId": 1, "isLeader": true}],
            })
        };
        hub.apply(event(LOBBY_PATH, EventType::Create, lobby(400)))
            .await;
        hub.apply(event(LOBBY_PATH, EventType::Update, lobby(400)))
            .await;
        hub.apply(event(LOBBY_PATH, EventType::Update, lobby(450)))
            .await;
        hub.apply(event(LOBBY_PATH, EventType::Delete, json!(null)))
            .await;

        let queues: Vec<Option<Queue>> = std::iter::from_fn(|| rx.try_recv().ok())
            .map(|m| match m {
                ServerMessage::Lobby { lobby } => lobby.and_then(|l| l.queue),
                other => panic!("esperava lobby: {other:?}"),
            })
            .collect();
        assert_eq!(queues, [Some(Queue::NormalDraft), Some(Queue::Aram), None]);
        assert_eq!(hub.state().lobby, None);
    }

    #[tokio::test]
    async fn comandos_de_lobby_sem_lol_respondem_not_connected() {
        let hub = Hub::new();
        for result in [
            hub.create_lobby(Queue::RankedSolo).await,
            hub.leave_lobby().await,
        ] {
            assert!(matches!(
                result,
                CommandResult::Error {
                    code: ErrorCode::NotConnected,
                    ..
                }
            ));
        }
    }

    #[tokio::test]
    async fn queda_limpa_o_estado() {
        let hub = Hub::new();
        hub.set_gameflow_phase(Some("ChampSelect".into()));
        hub.set_lobby(Lobby::from_json(&json!({"gameConfig": {"queueId": 420}})));
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        let state = hub.state();
        assert_eq!(state.gameflow_phase, None);
        assert_eq!(state.lobby, None);
        assert!(matches!(state.lcu, LcuStatus::Disconnected { .. }));
        assert!(hub.client().is_none());
    }

    #[tokio::test]
    async fn launch_ux_sem_lol_responde_not_connected() {
        let hub = Hub::new();
        assert!(matches!(
            hub.launch_ux().await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }
}
