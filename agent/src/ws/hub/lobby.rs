//! Lobby e fila: criar, sair, cargos e busca de partida.

use super::{not_connected, Hub, Resource, Shared, Stamp, REFRESH_ATTEMPTS};
use crate::custom_game::{self, CustomGameOptions};
use crate::lobby::{self, Lobby, Position, Queue, SetPositionsError, SubteamSlot};
use crate::lobby_link::{self, LobbyLink};
use crate::matchmaking::{self, Matchmaking};
use crate::queues::{GameQueue, MODE_SCREEN_QUEUES};
use crate::quick_play::{self, PlayerSlotInput, SetSlotsError};
use crate::training::{self, TutorialPart};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    pub fn set_lobby(&self, lobby: Option<Lobby>) {
        self.update(|shared| write_lobby(shared, lobby));
        self.drop_lobby_link_without_lobby();
        self.spawn_fetch_lobby_ranks();
    }

    pub(super) fn set_lobby_if(&self, stamp: Stamp, lobby: Option<Lobby>) -> bool {
        let applied = self.update_if(Resource::Lobby, stamp, |shared| write_lobby(shared, lobby));
        self.drop_lobby_link_without_lobby();
        self.spawn_fetch_lobby_ranks();
        applied
    }

    pub fn set_lobby_link(&self, link: Option<LobbyLink>) {
        self.update(|shared| write_lobby_link(shared, link));
    }

    /// O link do lobby (#226) some junto com o lobby.
    fn drop_lobby_link_without_lobby(&self) {
        self.update(|shared| {
            if shared.state.lobby.is_some() {
                return None;
            }
            write_lobby_link(shared, None)
        });
    }

    /// Gera (ou reaproveita) o link para outra pessoa entrar no lobby
    /// (comando `createLobbyLink`, #226). Fora de lobby recusa sem falar com
    /// a LCU. O link chega numa mensagem `lobbyLink` antes da resposta.
    pub async fn create_lobby_link(&self) -> CommandResult {
        let (client, session) = {
            let shared = self.lock();
            if shared.state.lobby.is_none() {
                return CommandResult::error(ErrorCode::InvalidCommand, "fora de lobby");
            }
            let Some(client) = shared.client.clone() else {
                return not_connected();
            };
            (client, shared.session)
        };
        match lobby_link::create(&client).await {
            Ok(Some(link)) => {
                let mut published = false;
                self.update(|shared| {
                    // O lobby pode ter fechado, ou o LoL caído, enquanto a
                    // LCU respondia.
                    if shared.session != session || shared.state.lobby.is_none() {
                        return None;
                    }
                    published = true;
                    write_lobby_link(shared, Some(link))
                });
                if published {
                    CommandResult::Ok
                } else {
                    CommandResult::error(ErrorCode::InvalidCommand, "o lobby fechou")
                }
            }
            Ok(None) => CommandResult::error(ErrorCode::Lcu, "a LCU não mandou o código do lobby"),
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Cria um lobby na fila `queue` ou troca a fila do lobby atual (comando
    /// `createLobby`, #17). Publica o lobby que a LCU respondeu antes de
    /// devolver, sem esperar o evento do WAMP.
    pub async fn create_lobby(&self, queue: Queue) -> CommandResult {
        self.create_lobby_in(queue.id()).await
    }

    /// Como [`Hub::create_lobby`], com o `queueId` de uma das filas
    /// disponíveis (comando `createLobbyById`, #146). Recusa sem falar com a
    /// LCU uma fila que não está na lista.
    pub async fn create_lobby_by_id(&self, queue_id: u32) -> CommandResult {
        let listed = self
            .lock()
            .state
            .queues
            .iter()
            .find(|q| q.queue_id == queue_id)
            .map(|q| q.available);
        match listed {
            // Fila fixa da tela que a lista ainda não trouxe (lida antes do
            // login, por exemplo): a LCU decide se está aberta (#210).
            None if MODE_SCREEN_QUEUES.contains(&queue_id) => self.create_lobby_in(queue_id).await,
            None => CommandResult::error(
                ErrorCode::InvalidCommand,
                format!("a fila {queue_id} não está disponível agora"),
            ),
            Some(false) => CommandResult::error(
                ErrorCode::InvalidCommand,
                format!("a fila {queue_id} está fechada agora"),
            ),
            Some(true) => self.create_lobby_in(queue_id).await,
        }
    }

    /// Inicia uma parte do Tutorial (comando `startTutorial`, #209 e #228):
    /// a de `queue_id`, ou a próxima que falta. Cria o lobby na fila da parte
    /// e entra na busca. O lobby e a busca são publicados antes de devolver.
    pub async fn start_tutorial(&self, queue_id: Option<u32>) -> CommandResult {
        if let Some(queue_id) = queue_id {
            if !training::TUTORIAL_QUEUES.contains(&queue_id) {
                return CommandResult::error(
                    ErrorCode::InvalidCommand,
                    format!("a fila {queue_id} não é uma parte do Tutorial"),
                );
            }
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        let queue_id = match queue_id {
            Some(queue_id) => queue_id,
            None => training::next_tutorial(&client).await,
        };
        if let error @ CommandResult::Error { .. } = self.create_lobby_in(queue_id).await {
            return error;
        }
        self.start_queue().await
    }

    /// Abre o Modo de Treino (comando `startPracticeTool`, #209 e #229): cria
    /// a personalizada e deixa o jogador no lobby dela, com as equipes e os
    /// bots. Publica o lobby antes de devolver; a partida começa com
    /// `startCustomGame`.
    pub async fn start_practice_tool(&self) -> CommandResult {
        let Some((client, stamp)) = self.begin(Resource::Lobby) else {
            return not_connected();
        };
        match training::create_practice_tool(&client).await {
            Ok(lobby) => {
                if !(lobby.is_some() && self.set_lobby_if(stamp, lobby)) {
                    self.refresh_lobby().await;
                }
                self.spawn_refresh_available_bots();
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Cria uma partida personalizada (comando `createCustomGame`, #262) e
    /// deixa o jogador no lobby dela, como o Modo de Treino. Opções inválidas
    /// nem chegam na LCU.
    pub async fn create_custom_game(&self, options: CustomGameOptions) -> CommandResult {
        if let Err(message) = options.validate() {
            return CommandResult::error(ErrorCode::InvalidCommand, message);
        }
        let Some((client, stamp)) = self.begin(Resource::Lobby) else {
            return not_connected();
        };
        match custom_game::create(&client, &options).await {
            Ok(lobby) => {
                if !(lobby.is_some() && self.set_lobby_if(stamp, lobby)) {
                    self.refresh_lobby().await;
                }
                self.spawn_refresh_available_bots();
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    async fn create_lobby_in(&self, queue_id: u32) -> CommandResult {
        let Some((client, stamp)) = self.begin(Resource::Lobby) else {
            return not_connected();
        };
        match lobby::create_lobby_by_id(&client, queue_id).await {
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
    ///
    /// Com uma busca aberta (por exemplo, parada pela punição de dodge ou de
    /// abandono, com o tempo correndo), sai da busca antes: a UI deixa sair
    /// do lobby nesse estado, e a busca parada não pode ficar presa na tela
    /// depois que o lobby some. Se a LCU recusar sair da busca, tenta sair do
    /// lobby mesmo assim.
    pub async fn leave_lobby(&self) -> CommandResult {
        let Some(client) = self.client() else {
            return not_connected();
        };
        let in_search = self.lock().state.matchmaking.is_some();
        if in_search {
            let _ = matchmaking::stop_search(&client).await;
        }
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
                if in_search {
                    self.refresh_matchmaking().await;
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

    /// Escolhe as posições e os campeões do Jogo Dinâmico (comando
    /// `setPlayerSlots`, #218). Relê o lobby pelo REST e publica antes de
    /// devolver, sem esperar o evento do WAMP.
    pub async fn set_player_slots(&self, slots: Vec<PlayerSlotInput>) -> CommandResult {
        if let Err(invalid) = quick_play::check_slots(&slots) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let (client, current, pages) = {
            let shared = self.lock();
            let Some(client) = shared.client.clone() else {
                return not_connected();
            };
            let state = &shared.state;
            let me = state.summoner.as_ref().map(|s| s.puuid.as_str());
            let current = state
                .lobby
                .as_ref()
                .and_then(|l| l.members.iter().find(|m| Some(m.puuid.as_str()) == me))
                .map(|m| m.player_slots.clone())
                .unwrap_or_default();
            (client, current, state.rune_pages.clone())
        };
        match quick_play::set_player_slots(&client, &slots, &current, pages.as_ref()).await {
            Ok(()) => {
                self.refresh_lobby().await;
                CommandResult::Ok
            }
            Err(SetSlotsError::Invalid(invalid)) => {
                CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string())
            }
            Err(SetSlotsError::Lcu(error)) => {
                CommandResult::error(ErrorCode::Lcu, error.to_string())
            }
        }
    }

    /// Abre ou fecha o lobby (comando `setLobbyOpen`, #225). Só o líder pode:
    /// fora de lobby ou sem ser o líder, recusa sem falar com a LCU. Relê o
    /// lobby pelo REST e publica antes de devolver.
    pub async fn set_lobby_open(&self, open: bool) -> CommandResult {
        let leader = self.lock().state.lobby.as_ref().map(|l| l.is_leader);
        match leader {
            None => return CommandResult::error(ErrorCode::InvalidCommand, "fora de lobby"),
            Some(false) => {
                return CommandResult::error(
                    ErrorCode::InvalidCommand,
                    "só o líder abre ou fecha o lobby",
                )
            }
            Some(true) => {}
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        match lobby::set_lobby_open(&client, open).await {
            Ok(()) => {
                self.refresh_lobby().await;
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Remove um membro do grupo ou passa a liderança para ele (comandos
    /// `kickLobbyMember` e `promoteLobbyMember`, #267). Só o líder pode, e só
    /// com outro jogador do lobby (nas filas comuns ou nas equipes e
    /// espectadores da personalizada): senão recusa sem falar com a LCU.
    /// Relê o lobby pelo REST e publica antes de devolver.
    pub async fn lobby_member_action(
        &self,
        summoner_id: u64,
        action: lobby::MemberAction,
    ) -> CommandResult {
        let check = {
            let shared = self.lock();
            let me = shared.state.summoner.as_ref();
            match shared.state.lobby.as_ref() {
                None => Err("fora de lobby"),
                Some(l) if !l.is_leader => Err(match action {
                    lobby::MemberAction::Kick => "só o líder remove jogadores do grupo",
                    lobby::MemberAction::Promote => "só o líder passa a liderança",
                }),
                Some(l) if !has_player(l, summoner_id) => Err("esse jogador não está no lobby"),
                Some(_) if me.is_some_and(|me| me.summoner_id == summoner_id) => {
                    Err("não dá para fazer isso com o próprio jogador")
                }
                Some(_) => Ok(()),
            }
        };
        if let Err(message) = check {
            return CommandResult::error(ErrorCode::InvalidCommand, message);
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        match lobby::member_action(&client, summoner_id, action).await {
            Ok(()) => {
                self.refresh_lobby().await;
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Troca o jogador logado de equipe ou de vaga no lobby da Arena (comando
    /// `setSubteamSlot`, #224). Fora de lobby ou num lobby sem equipes,
    /// recusa sem falar com a LCU. Relê o lobby pelo REST e publica antes de
    /// devolver.
    pub async fn set_subteam_slot(&self, slot: SubteamSlot) -> CommandResult {
        let subteams = self.lock().state.lobby.as_ref().map(|l| l.subteams);
        match subteams {
            None => return CommandResult::error(ErrorCode::InvalidCommand, "fora de lobby"),
            Some(None) => {
                return CommandResult::error(
                    ErrorCode::InvalidCommand,
                    "este lobby não tem equipes",
                )
            }
            Some(Some(_)) => {}
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        match lobby::set_subteam_slot(&client, slot).await {
            Ok(()) => {
                self.refresh_lobby().await;
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Publica o progresso do Tutorial (#228).
    pub fn set_tutorial(&self, tutorial: Option<Vec<TutorialPart>>) {
        self.update(|shared| {
            if shared.state.tutorial == tutorial {
                return None;
            }
            shared.state.tutorial = tutorial.clone();
            Some(ServerMessage::Tutorial { tutorial })
        });
    }

    /// Relê em segundo plano o progresso do Tutorial (#228): uma parte
    /// concluída muda o progresso, e a LCU não manda evento dele.
    pub(super) fn spawn_refresh_tutorial(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            let tutorial = training::tutorial_progress(&client).await;
            if hub.lock().session == session {
                hub.set_tutorial(tutorial);
            }
        });
    }

    /// Publica os campeões que dá para pôr nos slots do Jogo Dinâmico (#218).
    pub fn set_playable_champions(&self, champions: Option<Vec<u32>>) {
        self.update(|shared| {
            if shared.state.playable_champions == champions {
                return None;
            }
            shared.state.playable_champions = champions.clone();
            Some(ServerMessage::PlayableChampions {
                playable_champions: champions,
            })
        });
    }

    /// Relê em segundo plano os campeões que dá para pôr nos slots (#218):
    /// um campeão comprado ou a rotação nova mudam a lista, e a LCU não
    /// manda evento dela.
    pub(super) fn spawn_refresh_playable_champions(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            if let Ok(Some(champions)) = quick_play::playable_champions(&client).await {
                if hub.lock().session == session {
                    hub.set_playable_champions(Some(champions));
                }
            }
        });
    }

    /// Publica as filas em que dá para criar lobby (#146).
    pub fn set_queues(&self, queues: Vec<GameQueue>) {
        self.update(|shared| write_queues(shared, queues));
    }

    /// Como [`Hub::set_queues`], mas só se nada mudou as filas desde
    /// `stamp`: a releitura da conexão não apaga a lista que um login no
    /// meio dela já releu (#277).
    pub(super) fn set_queues_if(&self, stamp: Stamp, queues: Vec<GameQueue>) -> bool {
        self.update_if(Resource::Queues, stamp, |shared| {
            write_queues(shared, queues)
        })
    }

    /// Como [`Hub::set_queues`], mas só se a conexão ainda é a `session` da
    /// releitura.
    pub(super) fn restore_queues(&self, session: u64, queues: Vec<GameQueue>) {
        self.update(|shared| {
            if shared.session != session {
                return None;
            }
            write_queues(shared, queues)
        });
    }

    /// Relê a lista de filas pelo REST em segundo plano (#210). A lista lida
    /// ao conectar pode vir vazia ou incompleta quando o LoL ainda está
    /// subindo ou antes do login, e a LCU nem sempre manda o evento depois.
    pub(super) fn spawn_refresh_queues(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            if let Ok(Some(queues)) = crate::queues::available(&client).await {
                hub.restore_queues(session, queues);
            }
        });
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

/// Troca as filas, contando a escrita mesmo quando nada muda (#277).
fn write_queues(shared: &mut Shared, queues: Vec<GameQueue>) -> Option<ServerMessage> {
    shared.queues_rev += 1;
    if shared.state.queues == queues {
        return None;
    }
    shared.state.queues = queues.clone();
    Some(ServerMessage::Queues { queues })
}

/// Troca o lobby, contando a escrita mesmo quando nada muda (#73).
pub(super) fn write_lobby(shared: &mut Shared, mut lobby: Option<Lobby>) -> Option<ServerMessage> {
    shared.lobby_rev += 1;
    match lobby.as_mut() {
        Some(lobby) => super::lobby_ranks::fill_ranks(shared, lobby),
        // Sem lobby, os ranks buscados não valem mais (#275).
        None => shared.lobby_ranks.clear(),
    }
    if shared.state.lobby == lobby {
        return None;
    }
    shared.state.lobby = lobby.clone();
    Some(ServerMessage::Lobby { lobby })
}

fn write_lobby_link(shared: &mut Shared, link: Option<LobbyLink>) -> Option<ServerMessage> {
    if shared.state.lobby_link == link {
        return None;
    }
    shared.state.lobby_link = link.clone();
    Some(ServerMessage::LobbyLink { lobby_link: link })
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

/// Se `summoner_id` é um jogador (não um bot) do lobby: em `members` ou, na
/// personalizada, nas equipes e entre os espectadores.
fn has_player(lobby: &Lobby, summoner_id: u64) -> bool {
    summoner_id != 0
        && (lobby.members.iter().any(|m| m.summoner_id == summoner_id)
            || lobby.custom.as_ref().is_some_and(|c| {
                c.team1
                    .iter()
                    .chain(&c.team2)
                    .chain(&c.spectators)
                    .any(|m| m.bot.is_none() && m.summoner_id == summoner_id)
            }))
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::custom_game::{AramMap, CustomMap, CustomPickType, SpectatorPolicy};
    use crate::lobby::LOBBY_PATH;
    use crate::matchmaking::SearchState;
    use crate::matchmaking::SEARCH_PATH;
    use crate::queues::GAME_QUEUES_PATH;
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
            hub.start_tutorial(None).await,
            hub.start_tutorial(Some(2010)).await,
            hub.start_practice_tool().await,
            hub.create_custom_game(custom_options()).await,
            hub.set_player_slots(vec![slot(22, Position::Bottom), slot(266, Position::Top)])
                .await,
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

    fn custom_options() -> CustomGameOptions {
        CustomGameOptions {
            name: "Partida de Fulano".into(),
            password: String::new(),
            map: CustomMap::SummonersRift,
            pick_type: CustomPickType::BlindPick,
            aram_map: AramMap::HowlingAbyss,
            team_size: 5,
            spectators: SpectatorPolicy::None,
            hide_from_public_list: false,
            spectator_delay: true,
        }
    }

    #[tokio::test]
    async fn personalizada_invalida_nem_chega_na_lcu() {
        let hub = Hub::new();
        for options in [
            CustomGameOptions {
                name: " ".into(),
                ..custom_options()
            },
            CustomGameOptions {
                team_size: 6,
                ..custom_options()
            },
        ] {
            assert!(matches!(
                hub.create_custom_game(options).await,
                CommandResult::Error {
                    code: ErrorCode::InvalidCommand,
                    ..
                }
            ));
        }
    }

    fn slot(champion_id: u32, position: Position) -> PlayerSlotInput {
        PlayerSlotInput {
            champion_id,
            position,
            spell1_id: None,
            spell2_id: None,
            rune_page_id: None,
        }
    }

    #[tokio::test]
    async fn slots_invalidos_nem_chegam_na_lcu() {
        let hub = Hub::new();
        let result = hub
            .set_player_slots(vec![slot(22, Position::Top), slot(266, Position::Top)])
            .await;
        assert!(matches!(
            result,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn publica_os_campeoes_jogaveis() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.set_playable_champions(Some(vec![1, 22]));
        hub.set_playable_champions(Some(vec![1, 22]));
        let messages: Vec<ServerMessage> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert_eq!(
            messages,
            [ServerMessage::PlayableChampions {
                playable_champions: Some(vec![1, 22])
            }]
        );
        assert_eq!(hub.state().playable_champions, Some(vec![1, 22]));
    }

    fn queues_json() -> serde_json::Value {
        let queue = |id: u32, name: &str, mode: &str| {
            json!({
                "id": id, "name": name, "gameMode": mode, "gameSelectCategory": "kPvP",
                "gameSelectModeGroup": "kARAM", "mapId": 12,
                "queueAvailability": "Available", "isEnabled": true, "isVisible": true,
            })
        };
        json!([
            queue(2400, "ARAM: Desordem", "KIWI"),
            queue(450, "ARAM", "ARAM")
        ])
    }

    #[tokio::test]
    async fn segue_as_filas_disponiveis() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.apply(event(GAME_QUEUES_PATH, EventType::Update, queues_json()))
            .await;
        // A mesma lista de novo não manda nada.
        hub.apply(event(GAME_QUEUES_PATH, EventType::Update, queues_json()))
            .await;
        // Nem um `Delete` ou um JSON que não é lista.
        hub.apply(event(GAME_QUEUES_PATH, EventType::Delete, json!(null)))
            .await;
        hub.apply(event(GAME_QUEUES_PATH, EventType::Update, json!({})))
            .await;

        let messages: Vec<ServerMessage> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        let [ServerMessage::Queues { queues }] = messages.as_slice() else {
            panic!("esperava uma mensagem queues: {messages:?}");
        };
        let ids: Vec<u32> = queues.iter().map(|q| q.queue_id).collect();
        assert_eq!(ids, [450, 2400]);
        assert_eq!(queues[0].queue, Some(Queue::Aram));
        assert_eq!(hub.state().queues, *queues);
    }

    #[tokio::test]
    async fn releitura_nao_apaga_as_filas_relidas_no_login() {
        // A releitura da conexão tira o carimbo antes das leituras (#277).
        let hub = Hub::new();
        let stamp = hub.lock().stamp(Resource::Queues);
        // O login no meio dela relê as filas em segundo plano.
        hub.apply(event(GAME_QUEUES_PATH, EventType::Update, queues_json()))
            .await;
        // A lista vazia, lida antes do login, chega depois e é descartada.
        assert!(!hub.set_queues_if(stamp, Vec::new()));
        assert_eq!(hub.state().queues.len(), 2);
        // Sem nada no meio, a releitura entra.
        let stamp = hub.lock().stamp(Resource::Queues);
        assert!(hub.set_queues_if(stamp, Vec::new()));
        assert!(hub.state().queues.is_empty());
    }

    #[tokio::test]
    async fn criar_lobby_por_id_so_com_fila_da_lista() {
        let hub = Hub::new();
        let not_listed = hub.create_lobby_by_id(4310).await;
        assert!(
            matches!(
                &not_listed,
                CommandResult::Error {
                    code: ErrorCode::InvalidCommand,
                    message,
                } if message.contains("4310")
            ),
            "{not_listed:?}"
        );
        // Fila fixa da tela de modos fora da lista (lida antes do login): a
        // LCU decide (#210), e aqui ela não está conectada.
        assert!(matches!(
            hub.create_lobby_by_id(2400).await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
        hub.apply(event(GAME_QUEUES_PATH, EventType::Update, queues_json()))
            .await;
        // Na lista, segue para a LCU, que aqui não está conectada.
        assert!(matches!(
            hub.create_lobby_by_id(2400).await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn criar_lobby_por_id_recusa_fila_fechada() {
        let hub = Hub::new();
        let closed = json!([{
            "id": 1750, "name": "Arena 3x6", "gameMode": "CHERRY", "gameSelectCategory": "kPvP",
            "queueAvailability": "PlatformDisabled", "isEnabled": false, "isVisible": false,
        }]);
        hub.apply(event(GAME_QUEUES_PATH, EventType::Update, closed))
            .await;
        assert!(!hub.state().queues[0].available);
        let result = hub.create_lobby_by_id(1750).await;
        assert!(
            matches!(
                &result,
                CommandResult::Error {
                    code: ErrorCode::InvalidCommand,
                    message,
                } if message.contains("fechada")
            ),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn tutorial_so_com_fila_de_parte() {
        let hub = Hub::new();
        assert!(matches!(
            hub.start_tutorial(Some(420)).await,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn publica_o_progresso_do_tutorial() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let parts = training::tutorial_parts(&json!([])).unwrap();
        hub.set_tutorial(Some(parts.clone()));
        hub.set_tutorial(Some(parts.clone()));
        let messages: Vec<ServerMessage> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert_eq!(
            messages,
            [ServerMessage::Tutorial {
                tutorial: Some(parts.clone())
            }]
        );
        assert_eq!(hub.state().tutorial, Some(parts));
    }

    #[tokio::test]
    async fn abrir_o_lobby_so_para_o_lider() {
        let hub = Hub::new();
        let invalid = |result: CommandResult| {
            matches!(
                result,
                CommandResult::Error {
                    code: ErrorCode::InvalidCommand,
                    ..
                }
            )
        };
        assert!(invalid(hub.set_lobby_open(true).await), "fora de lobby");
        let lobby = |leader: bool| {
            json!({
                "gameConfig": {"queueId": 400},
                "localMember": {"puuid": "p", "isLeader": leader},
                "members": [{"puuid": "p", "isLeader": leader}],
                "partyType": "closed",
            })
        };
        hub.apply(event(LOBBY_PATH, EventType::Create, lobby(false)))
            .await;
        assert!(invalid(hub.set_lobby_open(true).await), "não é o líder");
        hub.apply(event(LOBBY_PATH, EventType::Update, lobby(true)))
            .await;
        assert!(matches!(
            hub.set_lobby_open(true).await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn remover_e_promover_so_o_lider_com_outro_jogador() {
        use crate::lobby::MemberAction::{Kick, Promote};
        let hub = Hub::new();
        let code = |result: CommandResult| match result {
            CommandResult::Error { code, .. } => Some(code),
            CommandResult::Ok => None,
        };
        for action in [Kick, Promote] {
            assert_eq!(
                code(hub.lobby_member_action(2, action).await),
                Some(ErrorCode::InvalidCommand),
                "fora de lobby"
            );
        }
        hub.set_summoner(Some(
            serde_json::from_value(json!({
                "gameName": "Eu", "tagLine": "BR1", "puuid": "p", "summonerId": 1,
                "summonerLevel": 30, "profileIconId": 1,
            }))
            .unwrap(),
        ));
        let lobby = |leader: bool| {
            json!({
                "gameConfig": {"queueId": 400},
                "localMember": {"puuid": "p", "summonerId": 1, "isLeader": leader},
                "members": [
                    {"puuid": "p", "summonerId": 1, "isLeader": leader},
                    {"puuid": "amigo", "summonerId": 2, "isLeader": !leader},
                ],
            })
        };
        hub.apply(event(LOBBY_PATH, EventType::Create, lobby(false)))
            .await;
        for action in [Kick, Promote] {
            assert_eq!(
                code(hub.lobby_member_action(2, action).await),
                Some(ErrorCode::InvalidCommand),
                "não é o líder"
            );
        }
        hub.apply(event(LOBBY_PATH, EventType::Update, lobby(true)))
            .await;
        for action in [Kick, Promote] {
            assert_eq!(
                code(hub.lobby_member_action(1, action).await),
                Some(ErrorCode::InvalidCommand),
                "o próprio jogador"
            );
            assert_eq!(
                code(hub.lobby_member_action(3, action).await),
                Some(ErrorCode::InvalidCommand),
                "quem não está no lobby"
            );
            assert_eq!(
                code(hub.lobby_member_action(2, action).await),
                Some(ErrorCode::NotConnected),
                "um amigo do lobby, com o LoL fechado"
            );
        }
    }

    #[tokio::test]
    async fn link_do_lobby_so_dentro_de_lobby_e_some_com_ele() {
        let hub = Hub::new();
        let code = |result: CommandResult| match result {
            CommandResult::Error { code, .. } => Some(code),
            CommandResult::Ok => None,
        };
        assert_eq!(
            code(hub.create_lobby_link().await),
            Some(ErrorCode::InvalidCommand),
            "fora de lobby"
        );
        let lobby = json!({"gameConfig": {"queueId": 400, "gameMode": "CLASSIC"}});
        hub.apply(event(LOBBY_PATH, EventType::Create, lobby.clone()))
            .await;
        assert_eq!(
            code(hub.create_lobby_link().await),
            Some(ErrorCode::NotConnected)
        );

        let (_, mut rx) = hub.subscribe();
        let link = LobbyLink {
            url: "https://gg.riotgames.com/LOL?joinCode=ABCD".into(),
            join_code: "ABCD".into(),
            expires_at: None,
        };
        hub.set_lobby_link(Some(link.clone()));
        // Trocar o lobby não leva o link.
        hub.apply(event(
            LOBBY_PATH,
            EventType::Update,
            json!({"gameConfig": {"queueId": 430, "gameMode": "CLASSIC"}}),
        ))
        .await;
        assert_eq!(hub.state().lobby_link, Some(link.clone()));
        hub.apply(event(LOBBY_PATH, EventType::Delete, json!(null)))
            .await;
        assert_eq!(hub.state().lobby_link, None);
        let links: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|m| match m {
                ServerMessage::LobbyLink { lobby_link } => Some(lobby_link),
                _ => None,
            })
            .collect();
        assert_eq!(links, [Some(link), None]);
    }

    #[tokio::test]
    async fn trocar_de_vaga_so_no_lobby_com_equipes() {
        let hub = Hub::new();
        let slot = SubteamSlot {
            team: 2,
            position: 1,
        };
        let code = |result: CommandResult| match result {
            CommandResult::Error { code, .. } => Some(code),
            CommandResult::Ok => None,
        };
        assert_eq!(
            code(hub.set_subteam_slot(slot).await),
            Some(ErrorCode::InvalidCommand),
            "fora de lobby"
        );
        hub.apply(event(
            LOBBY_PATH,
            EventType::Create,
            json!({"gameConfig": {"queueId": 400, "gameMode": "CLASSIC", "maxTeamSize": 5}}),
        ))
        .await;
        assert_eq!(
            code(hub.set_subteam_slot(slot).await),
            Some(ErrorCode::InvalidCommand),
            "lobby sem equipes"
        );
        hub.apply(event(
            LOBBY_PATH,
            EventType::Update,
            json!({"gameConfig": {
                "queueId": 1750, "gameMode": "CHERRY",
                "maxLobbySize": 18, "maxTeamSize": 3, "numberOfTeamsInLobby": 6,
            }}),
        ))
        .await;
        assert_eq!(
            code(hub.set_subteam_slot(slot).await),
            Some(ErrorCode::NotConnected)
        );
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
