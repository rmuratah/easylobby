//! Estado que o agente mostra para a UI e quem o mantém atualizado.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::broadcast;

use crate::champ_select::{self, ChampSelect, CHAMP_SELECT_SESSION_PATH};
use crate::connection::{ConnectionEvent, LcuConnection};
use crate::friends::{self, Friend};
use crate::gameflow::{self, GameflowPhase};
use crate::lobby::{self, Lobby, Position, Queue, SetPositionsError, LOBBY_PATH};
use crate::matchmaking::{self, Matchmaking, SEARCH_PATH};
use crate::ready_check::{self, ReadyCheck, READY_CHECK_PATH};
use crate::rest::{LcuClient, LcuError};
use crate::summoner::{self, Summoner, CURRENT_SUMMONER_PATH};
use crate::ux;
use crate::wamp::{EventType, LcuEvent};

use super::protocol::{AgentState, CommandResult, ErrorCode, LcuStatus, ServerMessage};

/// Fase do gameflow; mora em [`crate::gameflow`] desde o #16.
pub use crate::gameflow::GAMEFLOW_PHASE_PATH;

/// Quantas mensagens uma UI lenta pode ficar devendo antes de receber um
/// `hello` novo no lugar delas.
const BROADCAST_CAPACITY: usize = 256;

/// Quantas vezes um comando relê o lobby ou a busca quando eventos chegam
/// no meio da leitura (veja [`Hub::refresh_lobby`]).
const REFRESH_ATTEMPTS: usize = 3;

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
    /// Muda a cada `Connected` e `Disconnected` (#73).
    session: u64,
    /// Contam as escritas no lobby e na busca, mesmo as que não mudam nada
    /// (#73). Veja [`Stamp`].
    lobby_rev: u64,
    matchmaking_rev: u64,
}

/// Recurso do estado que um comando relê ou muda pelo REST.
#[derive(Debug, Clone, Copy)]
enum Resource {
    Lobby,
    Matchmaking,
}

impl Shared {
    fn rev_mut(&mut self, resource: Resource) -> &mut u64 {
        match resource {
            Resource::Lobby => &mut self.lobby_rev,
            Resource::Matchmaking => &mut self.matchmaking_rev,
        }
    }

    fn stamp(&mut self, resource: Resource) -> Stamp {
        Stamp {
            session: self.session,
            rev: *self.rev_mut(resource),
        }
    }
}

/// Sessão e revisão de um recurso no momento em que uma requisição REST
/// saiu (#73).
///
/// Os comandos esperam a LCU fora da trava, e enquanto isso o WAMP e a queda
/// da conexão continuam mudando o estado. Um resultado REST só é aplicado se,
/// com a trava pega, a sessão e a revisão ainda forem as do carimbo: nesse
/// caso nada mudou o recurso durante a espera e a resposta é a notícia mais
/// nova. Se algo mudou, o estado já publicado é pelo menos tão novo quanto a
/// resposta, e ela é descartada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    session: u64,
    rev: u64,
}

impl Hub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                shared: Mutex::new(Shared {
                    state: AgentState::default(),
                    client: None,
                    session: 0,
                    lobby_rev: 0,
                    matchmaking_rev: 0,
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

    pub fn set_gameflow_phase(&self, phase: Option<GameflowPhase>) {
        self.update(|shared| {
            if shared.state.gameflow_phase == phase {
                return None;
            }
            shared.state.gameflow_phase = phase;
            Some(ServerMessage::GameflowPhase { phase })
        });
    }

    pub fn set_ready_check(&self, ready_check: Option<ReadyCheck>) {
        self.update(|shared| {
            if shared.state.ready_check == ready_check {
                return None;
            }
            shared.state.ready_check = ready_check.clone();
            Some(ServerMessage::ReadyCheck { ready_check })
        });
    }

    /// Aceita a partida encontrada (comando `acceptReadyCheck`, #20).
    pub async fn accept_ready_check(&self) -> CommandResult {
        match self.client() {
            Some(client) => command_result(ready_check::accept(&client).await),
            None => not_connected(),
        }
    }

    /// Recusa a partida encontrada (comando `declineReadyCheck`, #20).
    pub async fn decline_ready_check(&self) -> CommandResult {
        match self.client() {
            Some(client) => command_result(ready_check::decline(&client).await),
            None => not_connected(),
        }
    }

    pub fn set_lobby(&self, lobby: Option<Lobby>) {
        self.update(|shared| write_lobby(shared, lobby));
    }

    /// Cliente da conexão atual e o carimbo de `resource`, lidos juntos.
    /// Tire o carimbo logo antes da requisição cujo resultado vai aplicar.
    fn begin(&self, resource: Resource) -> Option<(LcuClient, Stamp)> {
        let mut shared = self.lock();
        let client = shared.client.clone()?;
        Some((client, shared.stamp(resource)))
    }

    /// Aplica o resultado de uma requisição REST só se nada mudou o recurso
    /// desde `stamp` (veja [`Stamp`]). A conferência, a mudança e a
    /// publicação acontecem com a mesma trava. Devolve se aplicou.
    fn update_if(
        &self,
        resource: Resource,
        stamp: Stamp,
        f: impl FnOnce(&mut Shared) -> Option<ServerMessage>,
    ) -> bool {
        let mut applied = false;
        self.update(|shared| {
            if shared.stamp(resource) != stamp {
                return None;
            }
            applied = true;
            f(shared)
        });
        applied
    }

    fn set_lobby_if(&self, stamp: Stamp, lobby: Option<Lobby>) -> bool {
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

    fn set_matchmaking_if(&self, stamp: Stamp, matchmaking: Option<Matchmaking>) -> bool {
        self.update_if(Resource::Matchmaking, stamp, |shared| {
            write_matchmaking(shared, matchmaking)
        })
    }

    /// Troca a lista de amigos inteira (#30), que deve vir na ordem de
    /// [`friends::sort_friends`].
    pub fn set_friends(&self, list: Vec<Friend>) {
        self.update(|shared| {
            if shared.state.friends == list {
                return None;
            }
            shared.state.friends = list.clone();
            Some(ServerMessage::Friends { friends: list })
        });
    }

    /// Põe ou atualiza um amigo na lista, mantendo a ordem. Publica a lista
    /// inteira só se algo que a UI mostra mudou.
    fn upsert_friend(&self, friend: Friend) {
        self.update(|shared| {
            let list = &mut shared.state.friends;
            match list.iter_mut().find(|f| f.id == friend.id) {
                Some(current) if *current == friend => return None,
                Some(current) => *current = friend,
                None => list.push(friend),
            }
            friends::sort_friends(list);
            Some(ServerMessage::Friends {
                friends: list.clone(),
            })
        });
    }

    /// Tira um amigo da lista (amizade desfeita).
    fn remove_friend(&self, id: &str) {
        self.update(|shared| {
            let list = &mut shared.state.friends;
            let before = list.len();
            list.retain(|f| f.id != id);
            if list.len() == before {
                return None;
            }
            Some(ServerMessage::Friends {
                friends: list.clone(),
            })
        });
    }

    pub fn set_champ_select(&self, champ_select: Option<ChampSelect>) {
        self.update(|shared| {
            if shared.state.champ_select == champ_select {
                return None;
            }
            shared.state.champ_select = champ_select.clone();
            Some(ServerMessage::ChampSelect { champ_select })
        });
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
    async fn refresh_lobby(&self) {
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
    async fn refresh_matchmaking(&self) {
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

    /// Atualiza o estado com o que a [`LcuConnection`] devolveu.
    ///
    /// Num `Connected`, relê pelo REST o invocador, a fase, o lobby, a fila,
    /// o ready check e a seleção de campeões, porque os eventos de antes da
    /// conexão se perderam. Num `Disconnected`, limpa o
    /// que dependia do LoL.
    pub async fn apply(&self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Connected(session) => {
                let client = session.client.clone();
                let (lobby_stamp, search_stamp) = {
                    let mut shared = self.lock();
                    shared.client = Some(session.client);
                    shared.session += 1;
                    (
                        shared.stamp(Resource::Lobby),
                        shared.stamp(Resource::Matchmaking),
                    )
                };
                self.set_lcu_status(LcuStatus::Connected {
                    session: u32::try_from(session.id).unwrap_or(u32::MAX),
                });
                let (summoner, phase, lobby, search, friend_list) = tokio::join!(
                    summoner::current_summoner(&client),
                    gameflow::current_phase(&client),
                    lobby::current_lobby(&client),
                    matchmaking::current_search(&client),
                    friends::friends(&client),
                );
                // 404 antes do login; qualquer outro erro também deixa vazio,
                // e o próximo evento da LCU corrige.
                self.set_summoner(summoner.ok());
                let phase = phase.ok();
                self.set_gameflow_phase(phase);
                // Um comando que publicou durante a releitura é mais novo.
                self.set_lobby_if(lobby_stamp, lobby.ok().flatten());
                self.set_matchmaking_if(search_stamp, search.ok().flatten());
                // Com o chat ainda conectando a LCU pode responder erro ou
                // lista vazia; os eventos de cada amigo completam a lista.
                // Nenhum comando mexe nos amigos e os eventos só são
                // aplicados depois deste `apply`, então não há corrida.
                self.set_friends(friend_list.unwrap_or_default());
                // Só existe ready check aberto nessa fase; nas outras a LCU
                // responde 404 ou `Invalid`, e não vale o pedido a mais.
                if phase == Some(GameflowPhase::ReadyCheck) {
                    let ready_check = ready_check::current(&client).await;
                    self.set_ready_check(ready_check.ok().flatten());
                }
                // A sessão da seleção também só existe na fase dela (fora,
                // 404). Ela pode abrir um pouco antes da fase mudar, e o
                // evento `Create` cobre esse caso.
                if phase == Some(GameflowPhase::ChampSelect) {
                    let session = champ_select::current_session(&client).await;
                    self.set_champ_select(session.ok().flatten());
                }
            }
            ConnectionEvent::Event(event) => self.apply_lcu_event(&event),
            ConnectionEvent::Disconnected(reason) => {
                {
                    let mut shared = self.lock();
                    shared.client = None;
                    shared.session += 1;
                }
                self.set_summoner(None);
                self.set_gameflow_phase(None);
                self.set_ready_check(None);
                self.set_lobby(None);
                self.set_matchmaking(None);
                self.set_champ_select(None);
                self.set_friends(Vec::new());
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
                if let Some(phase) = GameflowPhase::from_json(&event.data) {
                    self.set_gameflow_phase(Some(phase));
                }
            }
            READY_CHECK_PATH if deleted => self.set_ready_check(None),
            READY_CHECK_PATH => self.set_ready_check(ReadyCheck::from_json(&event.data)),
            LOBBY_PATH if deleted => self.set_lobby(None),
            LOBBY_PATH => {
                if let Some(lobby) = Lobby::from_json(&event.data) {
                    self.set_lobby(Some(lobby));
                }
            }
            SEARCH_PATH if deleted => self.set_matchmaking(None),
            // `Invalid` (sem busca) também vira `None`.
            SEARCH_PATH => self.set_matchmaking(Matchmaking::from_json(&event.data)),
            // O `Delete` da LCU vem com uma sessão vazia em `data`; vale o
            // tipo do evento.
            CHAMP_SELECT_SESSION_PATH if deleted => self.set_champ_select(None),
            CHAMP_SELECT_SESSION_PATH => {
                if let Some(session) = ChampSelect::from_json(&event.data) {
                    self.set_champ_select(Some(session));
                }
            }
            uri if uri.starts_with(friends::FRIEND_EVENT_PREFIX) => {
                // `Delete` pode vir sem o amigo em `data`; o id está no URI.
                if deleted {
                    if let Some(id) = friends::friend_id_from_uri(uri) {
                        self.remove_friend(&id);
                    }
                } else if friends::friend_id_from_uri(uri).is_some() {
                    if let Some(friend) = Friend::from_json(&event.data) {
                        self.upsert_friend(friend);
                    }
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

/// Troca o lobby, contando a escrita mesmo quando nada muda (#73).
fn write_lobby(shared: &mut Shared, lobby: Option<Lobby>) -> Option<ServerMessage> {
    shared.lobby_rev += 1;
    if shared.state.lobby == lobby {
        return None;
    }
    shared.state.lobby = lobby.clone();
    Some(ServerMessage::Lobby { lobby })
}

/// Troca a busca, contando a escrita mesmo quando nada muda (#73).
fn write_matchmaking(
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

fn not_connected() -> CommandResult {
    CommandResult::error(ErrorCode::NotConnected, "o LoL não está aberto")
}

fn command_result(result: Result<(), LcuError>) -> CommandResult {
    match result {
        Ok(()) => CommandResult::Ok,
        Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
    }
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
    use crate::matchmaking::SearchState;
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
                phase: Some(GameflowPhase::Lobby)
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
    async fn segue_uma_partida_inteira_e_fases_novas() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let partida = [
            "Lobby",
            "Matchmaking",
            "ReadyCheck",
            "ChampSelect",
            "GameStart",
            "InProgress",
            "WaitingForStats",
            "PreEndOfGame",
            "EndOfGame",
            "None",
        ];
        for phase in partida {
            hub.apply(event(GAMEFLOW_PHASE_PATH, EventType::Update, json!(phase)))
                .await;
        }
        let mut seen = Vec::new();
        while let Ok(ServerMessage::GameflowPhase { phase: Some(phase) }) = rx.try_recv() {
            seen.push(phase.as_str());
        }
        assert_eq!(seen, partida);
        assert_eq!(hub.state().gameflow_phase, Some(GameflowPhase::Idle));

        // Fase de um patch novo: a UI fica sabendo que mudou, sem erro.
        hub.apply(event(
            GAMEFLOW_PHASE_PATH,
            EventType::Update,
            json!("FaseDeUmPatchNovo"),
        ))
        .await;
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::GameflowPhase {
                phase: Some(GameflowPhase::Unknown)
            }
        );
        // `data` que nem é string é ignorado.
        hub.apply(event(GAMEFLOW_PHASE_PATH, EventType::Update, json!(42)))
            .await;
        assert!(rx.try_recv().is_err());
        assert_eq!(hub.state().gameflow_phase, Some(GameflowPhase::Unknown));
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

    #[tokio::test]
    async fn queda_limpa_o_estado() {
        let hub = Hub::new();
        hub.set_gameflow_phase(Some(GameflowPhase::ChampSelect));
        hub.set_lobby(Lobby::from_json(&json!({"gameConfig": {"queueId": 420}})));
        hub.set_matchmaking(Matchmaking::from_json(&json!({"searchState": "Searching"})));
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        let state = hub.state();
        assert_eq!(state.gameflow_phase, None);
        assert_eq!(state.lobby, None);
        assert_eq!(state.matchmaking, None);
        assert!(matches!(state.lcu, LcuStatus::Disconnected { .. }));
        assert!(hub.client().is_none());
    }

    #[tokio::test]
    async fn resultado_rest_velho_e_descartado() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let search = |time: f64| {
            Matchmaking::from_json(&json!({"searchState": "Searching", "timeInQueue": time}))
        };
        let stamp = |resource| hub.lock().stamp(resource);

        // Nada mudou desde o carimbo: aplica e publica.
        let s = stamp(Resource::Matchmaking);
        assert!(hub.set_matchmaking_if(s, search(1.0)));
        assert!(matches!(
            rx.try_recv(),
            Ok(ServerMessage::Matchmaking { .. })
        ));

        // O WAMP mudou durante a espera: a resposta velha não volta.
        let s = stamp(Resource::Matchmaking);
        hub.set_matchmaking(None);
        assert!(!hub.set_matchmaking_if(s, search(2.0)));
        assert_eq!(hub.state().matchmaking, None);

        // Uma escrita igual, que não gera mensagem, também conta.
        let s = stamp(Resource::Matchmaking);
        hub.set_matchmaking(None);
        assert!(!hub.set_matchmaking_if(s, search(3.0)));

        // O carimbo de um recurso não depende do outro.
        let lobby = || Lobby::from_json(&json!({"gameConfig": {"queueId": 420}}));
        let s = stamp(Resource::Lobby);
        hub.set_matchmaking(search(4.0));
        assert!(hub.set_lobby_if(s, lobby()));

        // A conexão caiu: nada da sessão antiga entra.
        let s = stamp(Resource::Lobby);
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert!(!hub.set_lobby_if(s, lobby()));
        assert_eq!(hub.state().lobby, None);
    }

    fn ready_check(state: &str, response: &str, timer: f64) -> ConnectionEvent {
        event(
            READY_CHECK_PATH,
            EventType::Update,
            json!({
                "declinerIds": [],
                "dodgeWarning": "None",
                "playerResponse": response,
                "state": state,
                "suppressUx": false,
                "timer": timer,
            }),
        )
    }

    #[tokio::test]
    async fn segue_o_ready_check() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        // Na fila a LCU já manda o ready check com `Invalid`: nada muda.
        hub.apply(ready_check("Invalid", "None", 0.0)).await;
        assert!(rx.try_recv().is_err());

        hub.apply(ready_check("InProgress", "None", 0.0)).await;
        hub.apply(ready_check("InProgress", "Accepted", 1.0)).await;
        hub.apply(ready_check("EveryoneReady", "Accepted", 2.0))
            .await;
        hub.apply(event(READY_CHECK_PATH, EventType::Delete, json!(null)))
            .await;

        let mut seen = Vec::new();
        while let Ok(ServerMessage::ReadyCheck { ready_check }) = rx.try_recv() {
            seen.push(ready_check.map(|c| (c.state.as_str(), c.player_response.as_str(), c.timer)));
        }
        assert_eq!(
            seen,
            [
                Some(("InProgress", "None", 0.0)),
                Some(("InProgress", "Accepted", 1.0)),
                Some(("EveryoneReady", "Accepted", 2.0)),
                None,
            ]
        );
        assert_eq!(hub.state().ready_check, None);
    }

    #[tokio::test]
    async fn queda_fecha_o_ready_check() {
        let hub = Hub::new();
        hub.apply(ready_check("InProgress", "None", 4.0)).await;
        assert!(hub.state().ready_check.is_some());
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(hub.state().ready_check, None);
    }

    #[tokio::test]
    async fn aceitar_e_recusar_sem_lol_respondem_not_connected() {
        let hub = Hub::new();
        for result in [
            hub.accept_ready_check().await,
            hub.decline_ready_check().await,
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

    fn champ_select(phase: &str, my_champion: u32) -> serde_json::Value {
        json!({
            "id": "sessao",
            "queueId": 420,
            "localPlayerCellId": 0,
            "timer": {"phase": phase, "adjustedTimeLeftInPhase": 25000, "totalTimeInPhase": 25000},
            "myTeam": [{"cellId": 0, "championId": my_champion, "assignedPosition": "top"}],
            "theirTeam": [{"cellId": 5, "championId": 0}],
            "actions": [[{"id": 1, "type": "pick", "actorCellId": 0, "championId": my_champion,
                          "completed": false, "isInProgress": true, "isAllyAction": true}]],
        })
    }

    #[tokio::test]
    async fn segue_a_selecao_de_campeoes() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Create,
            champ_select("PLANNING", 0),
        ))
        .await;
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Update,
            champ_select("BAN_PICK", 86),
        ))
        .await;
        // JSON que não é sessão: ignorado.
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Update,
            json!({}),
        ))
        .await;
        // O `Delete` da LCU traz uma sessão vazia em `data`.
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Delete,
            json!({"actions": [], "myTeam": [], "timer": {"phase": ""}}),
        ))
        .await;

        let seen: Vec<Option<(&'static str, Option<u32>)>> =
            std::iter::from_fn(|| rx.try_recv().ok())
                .map(|m| match m {
                    ServerMessage::ChampSelect { champ_select } => champ_select.map(|cs| {
                        (
                            cs.timer.phase.as_str(),
                            cs.pending_action.and_then(|a| a.champion_id),
                        )
                    }),
                    other => panic!("esperava champSelect: {other:?}"),
                })
                .collect();
        assert_eq!(
            seen,
            [Some(("PLANNING", None)), Some(("BAN_PICK", Some(86))), None]
        );
        assert_eq!(hub.state().champ_select, None);
    }

    #[tokio::test]
    async fn queda_fecha_a_selecao_de_campeoes() {
        let hub = Hub::new();
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Update,
            champ_select("BAN_PICK", 0),
        ))
        .await;
        assert!(hub.state().champ_select.is_some());
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(hub.state().champ_select, None);
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

    fn friend(id: &str, availability: &str, game_status: &str) -> serde_json::Value {
        json!({
            "id": id,
            "pid": id,
            "puuid": format!("puuid-{id}"),
            "gameName": id,
            "gameTag": "BR1",
            "icon": 1,
            "availability": availability,
            "product": "league_of_legends",
            "productName": "League of Legends",
            "lol": {"gameStatus": game_status},
            "time": 1,
        })
    }

    fn friend_ids(message: ServerMessage) -> Vec<String> {
        match message {
            ServerMessage::Friends { friends } => friends.into_iter().map(|f| f.id).collect(),
            other => panic!("esperava a lista de amigos: {other:?}"),
        }
    }

    #[tokio::test]
    async fn segue_os_amigos() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.set_friends(Friend::list_from_json(&json!([
            friend("b@br1.pvp.net", "chat", "outOfGame"),
            friend("c@br1.pvp.net", "offline", ""),
        ])));
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["b@br1.pvp.net", "c@br1.pvp.net"]
        );

        // Alguém ficou online: entra na posição certa. O URI vem escapado.
        hub.apply(event(
            "/lol-chat/v1/friends/c%40br1.pvp.net",
            EventType::Update,
            friend("c@br1.pvp.net", "chat", "inQueue"),
        ))
        .await;
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["b@br1.pvp.net", "c@br1.pvp.net"]
        );
        assert_eq!(
            hub.state().friends[1].activity,
            crate::friends::Friend::from_json(&friend("c@br1.pvp.net", "chat", "inQueue"))
                .unwrap()
                .activity
        );

        // Só o `time` mudou: nada que a UI mostre, nenhuma mensagem.
        let mut same = friend("c@br1.pvp.net", "chat", "inQueue");
        same["time"] = json!(2);
        hub.apply(event(
            "/lol-chat/v1/friends/c%40br1.pvp.net",
            EventType::Update,
            same,
        ))
        .await;
        assert!(rx.try_recv().is_err());

        // Amigo novo, e um que vai para o fim da lista.
        hub.apply(event(
            "/lol-chat/v1/friends/a%40br1.pvp.net",
            EventType::Create,
            friend("a@br1.pvp.net", "chat", "outOfGame"),
        ))
        .await;
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["a@br1.pvp.net", "b@br1.pvp.net", "c@br1.pvp.net"]
        );
        hub.apply(event(
            "/lol-chat/v1/friends/a%40br1.pvp.net",
            EventType::Update,
            friend("a@br1.pvp.net", "offline", ""),
        ))
        .await;
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["b@br1.pvp.net", "c@br1.pvp.net", "a@br1.pvp.net"]
        );

        // Amizade desfeita, com `data` vazio.
        hub.apply(event(
            "/lol-chat/v1/friends/b%40br1.pvp.net",
            EventType::Delete,
            json!(null),
        ))
        .await;
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["c@br1.pvp.net", "a@br1.pvp.net"]
        );

        // Outros recursos do chat e lixo: ignorados.
        hub.apply(event(
            "/lol-chat/v1/friend-counts",
            EventType::Update,
            json!({"numFriends": 2}),
        ))
        .await;
        hub.apply(event(
            "/lol-chat/v1/friends/x%40br1.pvp.net",
            EventType::Update,
            json!("lixo"),
        ))
        .await;
        hub.apply(event(
            "/lol-chat/v1/friends/c%40br1.pvp.net/algo",
            EventType::Delete,
            json!(null),
        ))
        .await;
        assert!(rx.try_recv().is_err());

        // Queda do LoL esvazia a lista.
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        let mut cleared = false;
        while let Ok(message) = rx.try_recv() {
            if let ServerMessage::Friends { friends } = message {
                assert!(friends.is_empty());
                cleared = true;
            }
        }
        assert!(cleared);
        assert!(hub.state().friends.is_empty());
    }
}
