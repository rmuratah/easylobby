//! Lobby e fila: criar, sair, cargos e busca de partida.

use super::{not_connected, Hub, Resource, Shared, Stamp, REFRESH_ATTEMPTS};
use crate::lobby::{self, Lobby, Position, Queue, SetPositionsError};
use crate::matchmaking::{self, Matchmaking};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    pub fn set_lobby(&self, lobby: Option<Lobby>) {
        self.update(|shared| write_lobby(shared, lobby));
    }

    pub(super) fn set_lobby_if(&self, stamp: Stamp, lobby: Option<Lobby>) -> bool {
        self.update_if(Resource::Lobby, stamp, |shared| write_lobby(shared, lobby))
    }

    /// Cria um lobby na fila `queue` ou troca a fila do lobby atual (comando
    /// `createLobby`, #17). Publica o lobby que a LCU respondeu antes de
    /// devolver, sem esperar o evento do WAMP.
    pub async fn create_lobby(&self, queue: Queue) -> CommandResult {
        let Some((client, stamp)) = self.begin(Resource::Lobby) else {
            return not_connected();
        };
        match lobby::create_lobby(&client, queue).await {
            Ok(lobby) => {
                if !(lobby.is_some() && self.set_lobby_if(stamp, lobby)) {
                    self.refresh_lobby().await;
                }
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Sai do lobby (comando `leaveLobby`, #17).
    pub async fn leave_lobby(&self) -> CommandResult {
        let Some((client, stamp)) = self.begin(Resource::Lobby) else {
            return not_connected();
        };
        match lobby::leave_lobby(&client).await {
            Ok(()) => {
                // Um lobby que o WAMP trouxe durante a espera pode ser só o
                // evento atrasado de um comando anterior; relê para saber.
                if !self.set_lobby_if(stamp, None) {
                    self.refresh_lobby().await;
                }
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Escolhe os cargos do jogador logado (comando `setPositions`, #18).
    /// Relê o lobby pelo REST e publica antes de devolver, sem esperar o
    /// evento do WAMP.
    pub async fn set_positions(&self, first: Position, second: Position) -> CommandResult {
        if let Err(invalid) = lobby::check_positions(first, second) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        match lobby::set_positions(&client, first, second).await {
            Ok(()) => {
                self.refresh_lobby().await;
                CommandResult::Ok
            }
            Err(SetPositionsError::Invalid(invalid)) => {
                CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string())
            }
            Err(SetPositionsError::Lcu(error)) => {
                CommandResult::error(ErrorCode::Lcu, error.to_string())
            }
        }
    }

    pub fn set_matchmaking(&self, matchmaking: Option<Matchmaking>) {
        self.update(|shared| write_matchmaking(shared, matchmaking));
    }

    pub(super) fn set_matchmaking_if(
        &self,
        stamp: Stamp,
        matchmaking: Option<Matchmaking>,
    ) -> bool {
        self.update_if(Resource::Matchmaking, stamp, |shared| {
            write_matchmaking(shared, matchmaking)
        })
    }

    /// Entra na fila com o lobby atual (comando `startQueue`, #19). Relê a
    /// busca pelo REST e a publica antes de devolver, sem esperar o evento do
    /// WAMP.
    pub async fn start_queue(&self) -> CommandResult {
        let Some(client) = self.client() else {
            return not_connected();
        };
        if let Err(error) = matchmaking::start_search(&client).await {
            return CommandResult::error(ErrorCode::Lcu, error.to_string());
        }
        self.refresh_matchmaking().await;
        CommandResult::Ok
    }

    /// Sai da fila (comando `stopQueue`, #19).
    pub async fn stop_queue(&self) -> CommandResult {
        let Some((client, stamp)) = self.begin(Resource::Matchmaking) else {
            return not_connected();
        };
        match matchmaking::stop_search(&client).await {
            Ok(()) => {
                // Uma busca que o WAMP trouxe durante a espera pode ser só o
                // evento atrasado de um comando anterior; relê para saber.
                if !self.set_matchmaking_if(stamp, None) {
                    self.refresh_matchmaking().await;
                }
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Relê o lobby pelo REST depois de um comando e publica antes da
    /// resposta.
    ///
    /// Um evento WAMP que chega durante a espera muda a revisão e faz o
    /// resultado ser descartado ([`Stamp`]), mas esse evento pode ser o
    /// atraso de um comando anterior (criar o lobby na fila A e logo trocar
    /// para B, com o evento de A chegando durante a troca). Sem reler, a
    /// resposta do comando sairia com o lobby velho. Tenta de novo enquanto
    /// eventos chegam no meio, até [`REFRESH_ATTEMPTS`] vezes; se a LCU
    /// falhar ou os eventos não pararem, o próximo evento corrige.
    pub(super) async fn refresh_lobby(&self) {
        for _ in 0..REFRESH_ATTEMPTS {
            let Some((client, stamp)) = self.begin(Resource::Lobby) else {
                return;
            };
            let Ok(lobby) = lobby::current_lobby(&client).await else {
                return;
            };
            if self.set_lobby_if(stamp, lobby) {
                return;
            }
        }
    }

    /// Relê a busca pelo REST depois de um comando; como
    /// [`Hub::refresh_lobby`].
    pub(super) async fn refresh_matchmaking(&self) {
        for _ in 0..REFRESH_ATTEMPTS {
            let Some((client, stamp)) = self.begin(Resource::Matchmaking) else {
                return;
            };
            let Ok(search) = matchmaking::current_search(&client).await else {
                return;
            };
            if self.set_matchmaking_if(stamp, search) {
                return;
            }
        }
    }
}

/// Troca o lobby, contando a escrita mesmo quando nada muda (#73).
pub(super) fn write_lobby(shared: &mut Shared, lobby: Option<Lobby>) -> Option<ServerMessage> {
    shared.lobby_rev += 1;
    if shared.state.lobby == lobby {
        return None;
    }
    shared.state.lobby = lobby.clone();
    Some(ServerMessage::Lobby { lobby })
}

/// Troca a busca, contando a escrita mesmo quando nada muda (#73).
pub(super) fn write_matchmaking(
    shared: &mut Shared,
    matchmaking: Option<Matchmaking>,
) -> Option<ServerMessage> {
    shared.matchmaking_rev += 1;
    if shared.state.matchmaking == matchmaking {
        return None;
    }
    shared.state.matchmaking = matchmaking.clone();
    Some(ServerMessage::Matchmaking { matchmaking })
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::lobby::LOBBY_PATH;
    use crate::matchmaking::SearchState;
    use crate::matchmaking::SEARCH_PATH;
    use crate::wamp::EventType;
    use serde_json::json;

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
            hub.set_positions(Position::Top, Position::Jungle).await,
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
    async fn cargos_invalidos_nem_precisam_do_lol() {
        let hub = Hub::new();
        assert!(matches!(
            hub.set_positions(Position::Top, Position::Top).await,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn segue_a_fila() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let search = |state: &str, time: f64| {
            json!({
                "searchState": state,
                "queueId": 420,
                "timeInQueue": time,
                "estimatedQueueTime": 94.5,
                "errors": [],
            })
        };
        hub.apply(event(
            SEARCH_PATH,
            EventType::Create,
            search("Searching", 0.0),
        ))
        .await;
        hub.apply(event(
            SEARCH_PATH,
            EventType::Update,
            search("Searching", 1.0),
        ))
        .await;
        // Repetido não gera mensagem.
        hub.apply(event(
            SEARCH_PATH,
            EventType::Update,
            search("Searching", 1.0),
        ))
        .await;
        hub.apply(event(SEARCH_PATH, EventType::Update, search("Found", 1.0)))
            .await;
        hub.apply(event(SEARCH_PATH, EventType::Delete, json!(null)))
            .await;

        let seen: Vec<Option<(SearchState, f64)>> = std::iter::from_fn(|| rx.try_recv().ok())
            .map(|m| match m {
                ServerMessage::Matchmaking { matchmaking } => {
                    matchmaking.map(|m| (m.search_state, m.time_in_queue))
                }
                other => panic!("esperava matchmaking: {other:?}"),
            })
            .collect();
        assert_eq!(
            seen,
            [
                Some((SearchState::Searching, 0.0)),
                Some((SearchState::Searching, 1.0)),
                Some((SearchState::Found, 1.0)),
                None,
            ]
        );

        // `Invalid` também é "fora da fila".
        hub.apply(event(
            SEARCH_PATH,
            EventType::Update,
            search("Searching", 5.0),
        ))
        .await;
        hub.apply(event(
            SEARCH_PATH,
            EventType::Update,
            search("Invalid", 0.0),
        ))
        .await;
        assert_eq!(hub.state().matchmaking, None);
    }

    #[tokio::test]
    async fn comandos_de_fila_sem_lol_respondem_not_connected() {
        let hub = Hub::new();
        for result in [hub.start_queue().await, hub.stop_queue().await] {
            assert!(matches!(
                result,
                CommandResult::Error {
                    code: ErrorCode::NotConnected,
                    ..
                }
            ));
        }
    }
}
