//! Estado que o agente mostra para a UI e quem o mantém atualizado.
//!
//! Este arquivo tem o [`Hub`] em si: o estado compartilhado, os carimbos
//! (#73), a releitura ao conectar e a escolha de quem trata cada evento da
//! LCU. Cada assunto mora no seu arquivo, com o seu `impl Hub` e os seus
//! testes (lobby e fila em `lobby.rs`, runas em `runes.rs` e assim por
//! diante). Uma feature nova mexe no arquivo do assunto dela, e aqui só
//! acrescenta o recurso, a releitura ou o tópico do evento, se precisar.

mod champ_select;
mod champ_select_chat;
mod chat;
mod exit;
mod friends;
mod invitations;
mod lobby;
mod lobby_chat;
mod profile;
mod ready_check;
mod runes;
mod ux_guard;

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::broadcast;

use crate::champ_select::{ChampSelect, CHAMP_SELECT_SESSION_PATH};
use crate::champions::{
    ChampSelectChampions, BANNABLE_CHAMPIONS_PATH, PICKABLE_CHAMPIONS_PATH, SUBSET_CHAMPIONS_PATH,
};
use crate::chat::ChatUri;
use crate::connection::{ConnectionEvent, LcuConnection};
use crate::friends::{Friend, FriendGroup, FriendGroupUri};
use crate::gameflow::{self, GameflowPhase};
use crate::invitations::{ReceivedInvitation, RECEIVED_INVITATIONS_PATH};
use crate::lobby::{Lobby, LOBBY_PATH};
use crate::matchmaking::{self, Matchmaking, SEARCH_PATH};
use crate::queues::GAME_QUEUES_PATH;
use crate::ranked::RANKED_STATS_PATH;
use crate::ready_check::{ReadyCheck, READY_CHECK_PATH};
use crate::rest::{LcuClient, LcuError};
use crate::runes::{CURRENT_PAGE_PATH, INVENTORY_PATH, PAGES_PATH};
use crate::summoner::{self, Summoner, CURRENT_SUMMONER_PATH};
use crate::summoner_spells::{self};
use crate::ux::{self, KillUx};
use crate::wamp::{EventType, LcuEvent};

pub use exit::ExitAction;

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
    /// O mesmo para os convites recebidos (#32).
    invitations_rev: u64,
    /// O mesmo para a seleção de campeões e as páginas de runas (#87).
    champ_select_rev: u64,
    rune_pages_rev: u64,
    /// Fecha de novo a UI oficial quando o LoL a reabre (#188).
    ux_guard: ux_guard::UxGuard,
    /// Sala do grupo em que o agente já tentou entrar nesta conexão, e
    /// quantas vezes (#206). Evita repetir o `POST` a cada evento do lobby.
    lobby_chat_join: (String, u8),
}

/// Recurso do estado que um comando relê ou muda pelo REST.
#[derive(Debug, Clone, Copy)]
enum Resource {
    Lobby,
    Matchmaking,
    Invitations,
    ChampSelect,
    RunePages,
}

impl Shared {
    fn rev_mut(&mut self, resource: Resource) -> &mut u64 {
        match resource {
            Resource::Lobby => &mut self.lobby_rev,
            Resource::Matchmaking => &mut self.matchmaking_rev,
            Resource::Invitations => &mut self.invitations_rev,
            Resource::ChampSelect => &mut self.champ_select_rev,
            Resource::RunePages => &mut self.rune_pages_rev,
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
                    invitations_rev: 0,
                    champ_select_rev: 0,
                    rune_pages_rev: 0,
                    ux_guard: ux_guard::UxGuard::default(),
                    lobby_chat_join: (String::new(), 0),
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
        // O jogador quer a UI oficial: o agente para de fechá-la (#188).
        self.set_ux_guard(false);
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

    /// Atualiza o estado com o que a [`LcuConnection`] devolveu.
    ///
    /// Num `Connected`, relê pelo REST o invocador, a fase, as filas
    /// disponíveis, o lobby, a fila, o ready check, a seleção de campeões, as
    /// páginas e as árvores de runas e o perfil, porque os eventos de antes da conexão
    /// se perderam. Num `Disconnected`, limpa o que dependia do LoL.
    pub async fn apply(&self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Connected(session) => {
                let client = session.client.clone();
                let (lobby_stamp, search_stamp, invitations_stamp, rune_pages_stamp, chat_session) = {
                    let mut shared = self.lock();
                    shared.client = Some(session.client);
                    shared.session += 1;
                    // Só guarda a UI oficial fechada se foi o agente que a
                    // fechou nesta conexão (#188). Com o fechamento adiado,
                    // a guarda liga quando ele acontecer.
                    shared.ux_guard.active = session.kill_ux.is_killed();
                    shared.ux_guard.generation += 1;
                    (
                        shared.stamp(Resource::Lobby),
                        shared.stamp(Resource::Matchmaking),
                        shared.stamp(Resource::Invitations),
                        shared.stamp(Resource::RunePages),
                        shared.session,
                    )
                };
                if let KillUx::Scheduled { delay } = session.kill_ux {
                    self.schedule_kill_ux(delay);
                }
                self.set_lcu_status(LcuStatus::Connected {
                    session: u32::try_from(session.id).unwrap_or(u32::MAX),
                });
                let (
                    summoner,
                    phase,
                    lobby,
                    search,
                    received,
                    conversations,
                    rune_pages,
                    rune_catalog,
                    spell_catalog,
                    champion_catalog,
                    friend_list,
                    friend_groups,
                    queues,
                    playable_champions,
                ) = tokio::join!(
                    summoner::current_summoner(&client),
                    gameflow::current_phase(&client),
                    crate::lobby::current_lobby(&client),
                    matchmaking::current_search(&client),
                    crate::invitations::received_invitations(&client),
                    chat::load_conversations(&client),
                    crate::runes::current_pages(&client),
                    crate::runes::catalog(&client),
                    summoner_spells::catalog(&client),
                    crate::champions::catalog(&client),
                    crate::friends::friends(&client),
                    crate::friends::groups(&client),
                    crate::queues::available(&client),
                    crate::quick_play::playable_champions(&client),
                );
                // 404 antes do login; qualquer outro erro também deixa vazio,
                // e o próximo evento da LCU corrige.
                self.set_summoner(summoner.ok());
                let phase = phase.ok();
                self.set_gameflow_phase(phase);
                if ux_guard::reopens_ux(phase) {
                    self.guard_ux();
                }
                // Antes do lobby, para a UI já achar a fila dele na lista.
                self.restore_queues(chat_session, queues.ok().flatten().unwrap_or_default());
                // Um comando que publicou durante a releitura é mais novo.
                self.set_lobby_if(lobby_stamp, lobby.ok().flatten());
                self.set_matchmaking_if(search_stamp, search.ok().flatten());
                self.set_received_invitations_if(invitations_stamp, received.unwrap_or_default());
                self.restore_conversations(chat_session, conversations);
                // O chat do grupo depois do lobby, que diz qual é a sala (#206).
                let lobby_chat_id = self.lock().state.lobby.as_ref().map(|l| l.chat_id.clone());
                if let Some(lobby_chat_id) = lobby_chat_id {
                    if let Ok(Some((id, messages))) =
                        crate::lobby_chat::current_room(&client, &lobby_chat_id).await
                    {
                        self.restore_lobby_chat(chat_session, id, &messages);
                    }
                }
                // Com a UI oficial fechada ninguém entrou na sala.
                self.ensure_lobby_chat().await;
                // O catálogo antes das páginas, para a UI já poder desenhá-las.
                self.set_rune_catalog(rune_catalog.ok().flatten());
                self.set_rune_pages_if(rune_pages_stamp, rune_pages.ok());
                // Antes da seleção, para a UI já poder desenhar os feitiços.
                self.set_summoner_spell_catalog(spell_catalog.ok().flatten());
                self.set_champion_catalog(champion_catalog.ok().flatten());
                // Antes do login a LCU responde erro; o login relê (#218).
                if let Ok(Some(champions)) = playable_champions {
                    self.set_playable_champions(Some(champions));
                }
                // Com o chat ainda conectando a LCU pode responder erro ou
                // lista vazia; os eventos de cada amigo completam a lista.
                // Nenhum comando mexe nos amigos e os eventos só são
                // aplicados depois deste `apply`, então não há corrida.
                self.set_friends(friend_list.unwrap_or_default());
                // Sem as pastas, a UI agrupa pelo nome da pasta de cada amigo.
                self.set_friend_groups(friend_groups.unwrap_or_default());
                // Só existe ready check aberto nessa fase; nas outras a LCU
                // responde 404 ou `Invalid`, e não vale o pedido a mais.
                if phase == Some(GameflowPhase::ReadyCheck) {
                    let ready_check = crate::ready_check::current(&client).await;
                    self.set_ready_check(ready_check.ok().flatten());
                }
                // A sessão da seleção também só existe na fase dela (fora,
                // 404). Ela pode abrir um pouco antes da fase mudar, e o
                // evento `Create` cobre esse caso.
                if phase == Some(GameflowPhase::ChampSelect) {
                    // Os campeões que dá para escolher e banir também só
                    // existem na seleção (#28).
                    let champions = crate::champions::current(&client).await;
                    self.restore_champ_select_champions(chat_session, champions);
                    if let Some((client, stamp)) = self.begin(Resource::ChampSelect) {
                        let session = crate::champ_select::current_session(&client).await;
                        self.set_champ_select_if(stamp, session.ok().flatten());
                        // A sala do chat depois da sessão, para as mensagens já
                        // acharem quem mandou (#27).
                        if let Ok(Some((id, messages))) =
                            crate::champ_select_chat::current_room(&client).await
                        {
                            self.restore_champ_select_chat(chat_session, id, &messages);
                        }
                    }
                }
                // Por último: o histórico é a leitura mais lenta e só a tela
                // de Perfil usa (#171, #172, #175).
                self.refresh_profile().await;
            }
            ConnectionEvent::Event(event) => {
                self.apply_lcu_event(&event);
                // As cartas do ARAM (#201) chegam pelo WAMP só bem depois de
                // a seleção abrir; o REST já tem.
                if event.uri == CHAMP_SELECT_SESSION_PATH {
                    self.refresh_champion_offers().await;
                }
                // Com a UI oficial fechada, o agente entra na sala do grupo.
                if event.uri == LOBBY_PATH {
                    self.ensure_lobby_chat().await;
                }
            }
            ConnectionEvent::Disconnected(reason) => {
                {
                    let mut shared = self.lock();
                    shared.client = None;
                    shared.session += 1;
                    shared.ux_guard.active = false;
                    shared.lobby_chat_join = (String::new(), 0);
                }
                self.set_summoner(None);
                self.set_gameflow_phase(None);
                self.set_ready_check(None);
                self.set_lobby(None);
                self.set_queues(Vec::new());
                self.set_matchmaking(None);
                self.set_champ_select(None);
                self.set_received_invitations(Vec::new());
                self.clear_conversations();
                self.close_champ_select_chat(None);
                self.close_lobby_chat(None);
                self.set_rune_pages(None);
                self.set_rune_catalog(None);
                self.set_summoner_spell_catalog(None);
                self.set_champion_catalog(None);
                self.set_playable_champions(None);
                self.set_champ_select_champions(ChampSelectChampions::default());
                self.set_friends(Vec::new());
                self.set_friend_groups(Vec::new());
                self.set_ranked(None);
                self.set_champion_masteries(Vec::new());
                self.set_match_history(None);
                self.set_match_details(None);
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
                    let logged_in = self.lock().state.summoner.is_none();
                    self.set_summoner(Some(summoner));
                    // Lida antes do login, a lista de filas vem vazia (#210).
                    if logged_in {
                        self.spawn_refresh_queues();
                        self.spawn_refresh_playable_champions();
                    }
                }
            }
            GAMEFLOW_PHASE_PATH if deleted => self.set_gameflow_phase(None),
            GAMEFLOW_PHASE_PATH => {
                if let Some(phase) = GameflowPhase::from_json(&event.data) {
                    let before = self.lock().state.gameflow_phase;
                    self.set_gameflow_phase(Some(phase));
                    // O LoL reabre a UI oficial na partida encontrada e na
                    // seleção (#188).
                    if before != Some(phase) && ux_guard::reopens_ux(Some(phase)) {
                        self.guard_ux();
                    }
                    // A partida terminou: histórico, ranks e maestria mudaram.
                    if profile::ends_a_game(before, Some(phase)) {
                        self.spawn_refresh_profile();
                    }
                    // De volta à escolha de modo: as filas abertas podem ter
                    // mudado sem evento (#210).
                    if before != Some(phase)
                        && matches!(phase, GameflowPhase::Idle | GameflowPhase::Lobby)
                    {
                        self.spawn_refresh_queues();
                        // Campeão comprado ou rotação nova (#218).
                        self.spawn_refresh_playable_champions();
                    }
                }
            }
            // Os ranks mudam no fim de uma ranqueada (#171).
            RANKED_STATS_PATH if deleted => {}
            RANKED_STATS_PATH => {
                if let Some(ranked) = crate::ranked::from_json(&event.data) {
                    self.set_ranked(Some(ranked));
                }
            }
            // O LoL pediu para mostrar ou esconder a UI oficial (#188).
            ux::UX_STATE_REQUEST_PATH if deleted => {}
            ux::UX_STATE_REQUEST_PATH => {
                if ux_guard::asks_to_show(&event.data) {
                    self.guard_ux();
                }
            }
            READY_CHECK_PATH if deleted => self.set_ready_check(None),
            READY_CHECK_PATH => self.set_ready_check(ReadyCheck::from_json(&event.data)),
            LOBBY_PATH if deleted => {
                self.set_lobby(None);
                // Sem lobby não há grupo; a sala fecha mesmo se o `Delete`
                // dela se perder.
                self.close_lobby_chat(None);
            }
            LOBBY_PATH => {
                if let Some(lobby) = Lobby::from_json(&event.data) {
                    self.set_lobby(Some(lobby));
                }
            }
            // Uma fila entrou ou saiu de rotação (#146).
            GAME_QUEUES_PATH if deleted => {}
            GAME_QUEUES_PATH => {
                if let Some(queues) = crate::queues::from_json(&event.data) {
                    self.set_queues(queues);
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
            PICKABLE_CHAMPIONS_PATH | BANNABLE_CHAMPIONS_PATH | SUBSET_CHAMPIONS_PATH => {
                self.apply_champ_select_champions_event(event)
            }
            RECEIVED_INVITATIONS_PATH if deleted => self.set_received_invitations(Vec::new()),
            RECEIVED_INVITATIONS_PATH => {
                if let Some(list) = ReceivedInvitation::list_from_json(&event.data) {
                    self.set_received_invitations(list);
                }
            }
            // A lista, a página em uso e o inventário chegam separados; cada
            // um muda a sua parte. `Delete` neles não acontece na prática.
            PAGES_PATH | CURRENT_PAGE_PATH | INVENTORY_PATH if deleted => {}
            PAGES_PATH => self.edit_rune_pages(|r| r.apply_pages(&event.data)),
            CURRENT_PAGE_PATH => self.edit_rune_pages(|r| r.apply_current_page(&event.data)),
            INVENTORY_PATH => self.edit_rune_pages(|r| r.apply_inventory(&event.data)),
            uri if uri.starts_with(crate::friends::FRIEND_EVENT_PREFIX) => {
                // `Delete` pode vir sem o amigo em `data`; o id está no URI.
                if deleted {
                    if let Some(id) = crate::friends::friend_id_from_uri(uri) {
                        self.remove_friend(&id);
                    }
                } else if crate::friends::friend_id_from_uri(uri).is_some() {
                    if let Some(friend) = Friend::from_json(&event.data) {
                        self.upsert_friend(friend);
                    }
                }
            }
            uri if uri.starts_with(crate::friends::FRIEND_GROUPS_PATH) => {
                match crate::friends::FriendGroupUri::parse(uri) {
                    Some(FriendGroupUri::List) if !deleted => {
                        if let Some(list) = FriendGroup::list_from_json(&event.data) {
                            self.set_friend_groups(list);
                        }
                    }
                    Some(FriendGroupUri::Group(id)) if deleted => self.remove_friend_group(id),
                    Some(FriendGroupUri::Group(_)) => {
                        if let Some(group) = FriendGroup::from_json(&event.data) {
                            self.upsert_friend_group(group);
                        }
                    }
                    _ => {}
                }
            }
            uri => {
                if let Some(chat_uri) = ChatUri::parse(uri) {
                    if !self.apply_champ_select_chat_event(&chat_uri, event)
                        && !self.apply_lobby_chat_event(&chat_uri, event)
                    {
                        self.apply_chat_event(chat_uri, event);
                    }
                }
            }
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

    use serde_json::json;
    pub(super) fn event(
        uri: &str,
        event_type: EventType,
        data: serde_json::Value,
    ) -> ConnectionEvent {
        ConnectionEvent::Event(LcuEvent {
            uri: uri.into(),
            event_type,
            data,
        })
    }

    /// Conversa 1:1 dos testes do chat e do chat da seleção.
    pub(super) const AMIGO: &str = "amigo-puuid@br1.pvp.net";
    pub(super) const AMIGO_URI: &str = "/lol-chat/v1/conversations/amigo-puuid%40br1.pvp.net";

    /// Sessão da seleção com um só jogador nosso, para os testes de cada
    /// assunto.
    pub(super) fn champ_select(phase: &str, my_champion: u32) -> serde_json::Value {
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
