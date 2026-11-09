//! Chat do grupo no lobby (#206).
//!
//! A sala chega pelos eventos do WAMP. Como uma mensagem perdida (um evento
//! que chegou antes de a sala abrir, ou uma sala que abriu com outro `id`)
//! deixaria o chat parado, o agente também relê as mensagens da sala de
//! tempos em tempos ([`SYNC_INTERVAL`]) enquanto há grupo, e tenta de novo
//! entrar na sala quando ela não abre.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::{not_connected, Hub, Shared};

use serde_json::Value;

use crate::chat::{ChatUri, SendMessageError};
use crate::gameflow::GameflowPhase;
use crate::lobby_chat::{self, LobbyChat, LobbyChatMessage};
use crate::player_profile;
use crate::wamp::{EventType, LcuEvent};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

/// Tentativas de entrar na mesma sala do grupo por conexão.
const MAX_JOIN_ATTEMPTS: u8 = 10;

/// Espera mínima entre duas tentativas de entrar na sala. Os eventos do
/// lobby chegam em rajada e não podem gastar todas as tentativas de uma vez.
const JOIN_RETRY: Duration = Duration::from_secs(3);

/// De quanto em quanto tempo o agente relê as mensagens da sala do grupo.
const SYNC_INTERVAL: Duration = Duration::from_secs(2);

/// Tentativas de achar o nome de um jogador do chat por conexão.
const MAX_NAME_ATTEMPTS: u8 = 2;

/// O que o agente guarda da sala do grupo além do estado da UI.
#[derive(Debug, Default)]
pub(super) struct LobbyChatSync {
    /// Sala em que o agente tentou entrar (o `multiUserChatId` do lobby),
    /// quantas vezes e quando foi a última.
    join_chat_id: String,
    join_attempts: u8,
    last_join: Option<Instant>,
    /// `multiUserChatId` do lobby quando a sala aberta abriu. Se o lobby
    /// troca de sala (outro grupo, outra fila), a aberta ficou velha.
    owner: String,
    /// Riot ID dos jogadores do chat, buscado pelo `puuid`.
    names: HashMap<String, (String, String)>,
    names_in_flight: HashSet<String>,
    name_attempts: HashMap<String, u8>,
}

impl Hub {
    /// Sala do chat do grupo (#206): abre com o `Create` da conversa do
    /// lobby, junta as mensagens dela e fecha com o `Delete`. Devolve `false`
    /// quando o evento não é da sala, para ele seguir para o chat 1:1.
    pub(super) fn apply_lobby_chat_event(&self, uri: &ChatUri, event: &LcuEvent) -> bool {
        let deleted = event.event_type == EventType::Delete;
        let handled = match uri {
            ChatUri::Conversation(id) if deleted => self.close_lobby_chat(Some(id.as_str())),
            ChatUri::Conversation(id) => {
                let lobby_chat_id = self
                    .lock()
                    .state
                    .lobby
                    .as_ref()
                    .map(|l| l.chat_id.clone())
                    .unwrap_or_default();
                let open = self
                    .lock()
                    .state
                    .lobby_chat
                    .as_ref()
                    .is_some_and(|r| r.id == *id);
                if !open && !lobby_chat::is_room(&event.data, &lobby_chat_id) {
                    return false;
                }
                // O `id` do URI é o que a LCU aceita no envio. Uma sala nova
                // troca a anterior (que não fechou, se o `Delete` se perdeu).
                // Cada mensagem nova também atualiza a conversa com a
                // `lastMessage`: ela entra mesmo se o evento da mensagem se
                // perder.
                self.update(|shared| {
                    let opened = open_room(shared, id, false);
                    let last = event.data.get("lastMessage").filter(|m| m.is_object());
                    last.and_then(|m| write_lobby_messages(shared, id, m))
                        .or(opened)
                });
                true
            }
            ChatUri::Messages(id) | ChatUri::Message(id) => {
                let mut handled = false;
                self.update(|shared| {
                    let is_room = shared
                        .state
                        .lobby_chat
                        .as_ref()
                        .is_some_and(|r| r.id == *id);
                    handled = is_room;
                    // Mensagem apagada: o histórico fica como está.
                    if !is_room || deleted {
                        return None;
                    }
                    write_lobby_messages(shared, id, &event.data)
                });
                handled
            }
        };
        if handled {
            self.spawn_fetch_lobby_chat_names();
        }
        handled
    }

    /// Fecha a sala do grupo: a de `id`, ou qualquer uma com `None` (saiu do
    /// lobby, LoL fechou). Devolve se fechou alguma.
    pub(super) fn close_lobby_chat(&self, id: Option<&str>) -> bool {
        let mut closed = false;
        self.update(|shared| {
            let room = shared.state.lobby_chat.as_ref()?;
            if id.is_some_and(|id| id != room.id) {
                return None;
            }
            shared.state.lobby_chat = None;
            shared.lobby_chat_sync.owner.clear();
            closed = true;
            Some(ServerMessage::LobbyChat { lobby_chat: None })
        });
        closed
    }

    /// Aplica a sala relida pelo REST, se a conexão ainda é a mesma.
    /// `joined` diz que o agente acabou de entrar nela pelo lobby atual.
    pub(super) fn restore_lobby_chat(
        &self,
        session: u64,
        id: String,
        messages: &Value,
        joined: bool,
    ) {
        self.update(|shared| {
            if shared.session != session {
                return None;
            }
            let opened = open_room(shared, &id, joined);
            // Sem mensagem nova, a sala nova ainda precisa ser publicada.
            write_lobby_messages(shared, &id, messages).or(opened)
        });
        self.spawn_fetch_lobby_chat_names();
    }

    /// Entra na sala do grupo quando há lobby e a sala ainda não está aberta
    /// (#206). Quem entra é a UI oficial; com ela fechada pelo `kill-ux`, a
    /// sala nunca abre e o chat fica esperando. Uma sala aberta de outro
    /// lobby (o jogador trocou de grupo ou de fila) é fechada antes. Tenta no
    /// máximo [`MAX_JOIN_ATTEMPTS`] vezes por sala em cada conexão, com
    /// [`JOIN_RETRY`] entre uma e outra.
    pub(super) async fn ensure_lobby_chat(&self) {
        let (client, session, lobby) = {
            let mut shared = self.lock();
            let Some(lobby) = shared.state.lobby.clone() else {
                return;
            };
            if lobby.chat_id.is_empty() {
                return;
            }
            if shared.state.lobby_chat.is_some() {
                let owner = &shared.lobby_chat_sync.owner;
                if owner.is_empty() || *owner == lobby.chat_id {
                    return;
                }
                shared.state.lobby_chat = None;
                shared.lobby_chat_sync.owner.clear();
                // Sem UI conectada o envio falha, e tudo bem.
                let _ = self
                    .inner
                    .tx
                    .send(ServerMessage::LobbyChat { lobby_chat: None });
            }
            let Some(client) = shared.client.clone() else {
                return;
            };
            let sync = &mut shared.lobby_chat_sync;
            if sync.join_chat_id != lobby.chat_id {
                sync.join_chat_id.clone_from(&lobby.chat_id);
                sync.join_attempts = 0;
                sync.last_join = None;
            }
            if sync.join_attempts >= MAX_JOIN_ATTEMPTS
                || sync.last_join.is_some_and(|t| t.elapsed() < JOIN_RETRY)
            {
                return;
            }
            sync.join_attempts += 1;
            sync.last_join = Some(Instant::now());
            (client, shared.session, lobby)
        };
        // Num erro, o próximo evento do lobby ou a releitura tenta de novo.
        if let Ok(Some((id, messages))) = lobby_chat::join_room(&client, &lobby).await {
            self.restore_lobby_chat(session, id, &messages, true);
        }
    }

    /// Relê a sala do grupo a cada [`SYNC_INTERVAL`] enquanto a conexão
    /// `session` durar.
    pub(super) fn spawn_lobby_chat_sync(&self, session: u64) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let hub = self.clone();
        runtime.spawn(async move {
            loop {
                tokio::time::sleep(SYNC_INTERVAL).await;
                if hub.lock().session != session {
                    break;
                }
                hub.sync_lobby_chat(session).await;
            }
        });
    }

    /// Uma releitura da sala do grupo: acha a sala aberta na LCU ou entra
    /// nela, e junta as mensagens que o WAMP não trouxe.
    async fn sync_lobby_chat(&self, session: u64) {
        let (client, chat_id, room_id, poll) = {
            let shared = self.lock();
            if shared.session != session {
                return;
            }
            let Some(client) = shared.client.clone() else {
                return;
            };
            let Some(lobby) = shared.state.lobby.as_ref() else {
                return;
            };
            // Sozinho no lobby não chega mensagem de ninguém, e em jogo
            // ninguém olha o chat do grupo.
            let poll = lobby.members.len() >= 2
                && shared.state.gameflow_phase != Some(GameflowPhase::InProgress);
            (
                client,
                lobby.chat_id.clone(),
                shared.state.lobby_chat.as_ref().map(|r| r.id.clone()),
                poll,
            )
        };
        if chat_id.is_empty() {
            return;
        }
        // Só a sala deste lobby: uma de outro grupo que ficou aberta na LCU
        // não serve.
        if room_id.is_none() {
            if let Ok(Some((id, messages))) = lobby_chat::current_room(&client, &chat_id).await {
                if lobby_chat::room_belongs_to(&id, &chat_id) {
                    self.restore_lobby_chat(session, id, &messages, false);
                    return;
                }
            }
        }
        self.ensure_lobby_chat().await;
        let room_id = self.lock().state.lobby_chat.as_ref().map(|r| r.id.clone());
        if let (Some(id), true) = (room_id, poll) {
            if let Ok(messages) = lobby_chat::messages(&client, &id).await {
                self.restore_lobby_chat(session, id, &messages, false);
            }
        }
    }

    /// Busca o Riot ID de quem mandou mensagem e ainda está sem nome, numa
    /// tarefa separada, e publica a sala de novo com os nomes.
    fn spawn_fetch_lobby_chat_names(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (client, session, puuids) = {
            let mut shared = self.lock();
            let Some(client) = shared.client.clone() else {
                return;
            };
            let Some(room) = shared.state.lobby_chat.as_ref() else {
                return;
            };
            let senders = room.unnamed_senders();
            let sync = &mut shared.lobby_chat_sync;
            let puuids: Vec<String> = senders
                .into_iter()
                .filter(|p| {
                    player_profile::valid_puuid(p)
                        && !sync.names.contains_key(p)
                        && !sync.names_in_flight.contains(p)
                        && sync.name_attempts.get(p).copied().unwrap_or(0) < MAX_NAME_ATTEMPTS
                })
                .collect();
            for puuid in &puuids {
                sync.names_in_flight.insert(puuid.clone());
                *sync.name_attempts.entry(puuid.clone()).or_default() += 1;
            }
            (client, shared.session, puuids)
        };
        for puuid in puuids {
            let hub = self.clone();
            let client = client.clone();
            runtime.spawn(async move {
                let summoner = player_profile::summoner(&client, &puuid).await;
                hub.update(|shared| {
                    if shared.session != session {
                        return None;
                    }
                    let sync = &mut shared.lobby_chat_sync;
                    sync.names_in_flight.remove(&puuid);
                    let Ok(Some((summoner, _))) = summoner else {
                        return None;
                    };
                    if summoner.game_name.is_empty() {
                        return None;
                    }
                    sync.names
                        .insert(puuid, (summoner.game_name, summoner.tag_line));
                    write_names(shared)
                });
            });
        }
    }

    /// Manda uma mensagem no chat do grupo (comando `sendLobbyMessage`,
    /// #206). Publica a mensagem que a LCU respondeu antes de devolver, sem
    /// esperar o evento do WAMP.
    pub async fn send_lobby_message(&self, body: &str) -> CommandResult {
        if body.trim().is_empty() {
            return CommandResult::error(ErrorCode::InvalidCommand, "mensagem vazia");
        }
        let (client, session, room_id) = {
            let shared = self.lock();
            (
                shared.client.clone(),
                shared.session,
                shared.state.lobby_chat.as_ref().map(|r| r.id.clone()),
            )
        };
        let Some(client) = client else {
            return not_connected();
        };
        let Some(room_id) = room_id else {
            return CommandResult::error(
                ErrorCode::InvalidCommand,
                "o chat do grupo não está aberto",
            );
        };
        match lobby_chat::send_message(&client, &room_id, body).await {
            Ok(message) => {
                // A mensagem do WAMP tem o mesmo `id` e não repete.
                if let Some(message) = message {
                    self.update(|shared| {
                        if shared.session != session {
                            return None;
                        }
                        write_lobby_messages(shared, &room_id, &message)
                    });
                }
                CommandResult::Ok
            }
            Err(SendMessageError::Invalid(invalid)) => {
                CommandResult::error(ErrorCode::InvalidCommand, invalid)
            }
            Err(SendMessageError::Lcu(error)) => {
                CommandResult::error(ErrorCode::Lcu, error.to_string())
            }
        }
    }
}

/// Abre a sala `id` no estado, se ela ainda não é a aberta, e guarda de qual
/// lobby ela é: o atual, se o `id` começa com o dele ou se o agente acabou
/// de entrar nela por ele (`joined`). Devolve a mensagem que publica a sala
/// nova.
fn open_room(shared: &mut Shared, id: &str, joined: bool) -> Option<ServerMessage> {
    if shared.state.lobby_chat.as_ref().is_some_and(|r| r.id == id) {
        return None;
    }
    let room = LobbyChat::new(id);
    shared.state.lobby_chat = Some(room.clone());
    shared.lobby_chat_sync.owner = shared
        .state
        .lobby
        .as_ref()
        .map(|l| l.chat_id.clone())
        .filter(|chat_id| joined || lobby_chat::room_belongs_to(id, chat_id))
        .unwrap_or_default();
    Some(ServerMessage::LobbyChat {
        lobby_chat: Some(room),
    })
}

/// O Riot ID de `puuid` que o agente já conhece: o jogador logado, um amigo
/// ou um nome já buscado.
fn known_name(shared: &Shared, puuid: &str) -> Option<(String, String)> {
    let state = &shared.state;
    if let Some(me) = state.summoner.as_ref().filter(|s| s.puuid == puuid) {
        if !me.game_name.is_empty() {
            return Some((me.game_name.clone(), me.tag_line.clone()));
        }
    }
    if let Some(friend) = state
        .friends
        .iter()
        .find(|f| f.puuid == puuid && !f.game_name.is_empty())
    {
        return Some((friend.game_name.clone(), friend.tag_line.clone()));
    }
    shared.lobby_chat_sync.names.get(puuid).cloned()
}

/// Preenche os nomes que faltam na sala aberta e a publica se algo mudou.
fn write_names(shared: &mut Shared) -> Option<ServerMessage> {
    let mut room = shared.state.lobby_chat.take()?;
    let changed = room.fill_names(|puuid| known_name(shared, puuid));
    let message = changed.then(|| ServerMessage::LobbyChat {
        lobby_chat: Some(room.clone()),
    });
    shared.state.lobby_chat = Some(room);
    message
}

/// Junta à sala do grupo `id` as mensagens de `value` (uma mensagem ou uma
/// lista) e preenche os nomes conhecidos. Nada muda se a sala aberta não for
/// `id`.
fn write_lobby_messages(shared: &mut Shared, id: &str, value: &Value) -> Option<ServerMessage> {
    if !shared.state.lobby_chat.as_ref().is_some_and(|r| r.id == id) {
        return None;
    }
    let my_puuid = shared.state.summoner.as_ref().map(|s| s.puuid.clone());
    let messages = LobbyChatMessage::list_from_json(value, my_puuid.as_deref());
    let mut room = shared.state.lobby_chat.take()?;
    let mut changed = room.merge_messages(messages);
    changed |= room.fill_names(|puuid| known_name(shared, puuid));
    let message = changed.then(|| ServerMessage::LobbyChat {
        lobby_chat: Some(room.clone()),
    });
    shared.state.lobby_chat = Some(room);
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby::Lobby;
    use serde_json::json;
    use tokio::sync::broadcast;

    const SALA: &str = "grupo-1@sec.br1.pvp.net";

    fn salas(rx: &mut broadcast::Receiver<ServerMessage>) -> Vec<Option<LobbyChat>> {
        std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|m| match m {
                ServerMessage::LobbyChat { lobby_chat } => Some(lobby_chat),
                _ => None,
            })
            .collect()
    }

    fn evento(uri: &str, event_type: EventType, data: Value) -> LcuEvent {
        LcuEvent {
            uri: uri.into(),
            event_type,
            data,
        }
    }

    fn mensagem(id: &str, puuid: &str) -> Value {
        json!({"id": id, "type": "chat", "body": "oi", "fromPuuid": puuid, "timestamp": "2026-11-07T20:00:00.000Z"})
    }

    #[test]
    fn abre_junta_mensagens_e_fecha_a_sala_do_lobby() {
        let hub = Hub::new();
        hub.set_lobby(Lobby::from_json(
            &json!({"gameConfig": {"queueId": 440}, "multiUserChatId": "grupo-1"}),
        ));
        let mut rx = hub.subscribe().1;
        let conversa = "/lol-chat/v1/conversations/grupo-1%40sec.br1.pvp.net";
        // Pelo `id` do lobby, mesmo com outro `type`.
        hub.apply_lcu_event(&evento(
            conversa,
            EventType::Create,
            json!({"id": SALA, "type": "outro"}),
        ));
        assert_eq!(salas(&mut rx), [Some(LobbyChat::new(SALA))]);

        hub.apply_lcu_event(&evento(
            &format!("{conversa}/messages/m1"),
            EventType::Create,
            mensagem("m1", "amigo"),
        ));
        let sala = hub.state().lobby_chat.unwrap();
        assert_eq!(sala.messages.len(), 1);
        assert_eq!(sala.messages[0].from_puuid, "amigo");
        assert!(!sala.messages[0].from_me);
        salas(&mut rx);

        hub.apply_lcu_event(&evento(conversa, EventType::Delete, Value::Null));
        assert_eq!(salas(&mut rx), [None]);
        assert_eq!(hub.state().lobby_chat, None);
    }

    #[test]
    fn sair_do_lobby_fecha_a_sala() {
        let hub = Hub::new();
        hub.apply_lcu_event(&evento(
            "/lol-chat/v1/conversations/x%40sec.br1.pvp.net",
            EventType::Create,
            json!({"id": "x@sec.br1.pvp.net", "type": "customGame"}),
        ));
        assert!(hub.state().lobby_chat.is_some());
        hub.apply_lcu_event(&evento(
            crate::lobby::LOBBY_PATH,
            EventType::Delete,
            Value::Null,
        ));
        assert_eq!(hub.state().lobby_chat, None);
    }

    #[test]
    fn conversas_de_amigos_seguem_para_o_chat() {
        let hub = Hub::new();
        hub.apply_lcu_event(&evento(
            "/lol-chat/v1/conversations/amigo%40br1.pvp.net",
            EventType::Create,
            json!({"id": "amigo@br1.pvp.net", "type": "chat"}),
        ));
        assert_eq!(hub.state().lobby_chat, None);
    }

    fn eu() -> crate::summoner::Summoner {
        serde_json::from_value(json!({
            "gameName": "Rafinha Lover", "tagLine": "mrt", "summonerLevel": 1,
            "puuid": "eu", "summonerId": 1,
        }))
        .unwrap()
    }

    #[test]
    fn a_conversa_atualizada_traz_a_ultima_mensagem() {
        let hub = Hub::new();
        hub.set_summoner(Some(eu()));
        let conversa = "/lol-chat/v1/conversations/grupo-1%40sec.br1.pvp.net";
        hub.apply_lcu_event(&evento(
            conversa,
            EventType::Create,
            json!({"id": SALA, "type": "customGame"}),
        ));
        // O evento da mensagem se perdeu; só a conversa chegou.
        hub.apply_lcu_event(&evento(
            conversa,
            EventType::Update,
            json!({"id": SALA, "type": "customGame", "lastMessage": mensagem("m1", "eu")}),
        ));
        let sala = hub.state().lobby_chat.unwrap();
        assert_eq!(sala.messages.len(), 1);
        assert!(sala.messages[0].from_me);
        // O nome do jogador logado entra sem pedir nada à LCU.
        assert_eq!(sala.messages[0].game_name, "Rafinha Lover");
        assert_eq!(sala.messages[0].tag_line, "mrt");
    }

    #[test]
    fn mostra_quem_entrou_e_saiu_da_sala() {
        let hub = Hub::new();
        let conversa = "/lol-chat/v1/conversations/grupo-1%40sec.br1.pvp.net";
        hub.apply_lcu_event(&evento(
            conversa,
            EventType::Create,
            json!({"id": SALA, "type": "customGame"}),
        ));
        hub.apply_lcu_event(&evento(
            &format!("{conversa}/messages"),
            EventType::Update,
            json!([
                {"id": "s1", "type": "system", "body": "joined_room", "fromPuuid": "ana", "timestamp": "2026-11-07T20:00:00.000Z"},
                {"id": "s2", "type": "system", "body": "outra_coisa", "fromPuuid": "ana", "timestamp": "2026-11-07T20:00:01.000Z"},
                {"id": "s3", "type": "system", "body": "left_room", "fromPuuid": "ana", "timestamp": "2026-11-07T20:00:02.000Z"},
            ]),
        ));
        let sala = hub.state().lobby_chat.unwrap();
        let tipos: Vec<_> = sala.messages.iter().map(|m| m.kind).collect();
        assert_eq!(
            tipos,
            [
                crate::lobby_chat::LobbyChatMessageKind::Joined,
                crate::lobby_chat::LobbyChatMessageKind::Left
            ]
        );
    }

    #[tokio::test]
    async fn trocar_de_lobby_fecha_a_sala_velha() {
        let hub = Hub::new();
        hub.set_lobby(Lobby::from_json(
            &json!({"gameConfig": {"queueId": 440}, "multiUserChatId": "grupo-1"}),
        ));
        hub.apply_lcu_event(&evento(
            "/lol-chat/v1/conversations/grupo-1%40sec.br1.pvp.net",
            EventType::Create,
            json!({"id": SALA, "type": "customGame"}),
        ));
        // O mesmo lobby não fecha a sala.
        hub.ensure_lobby_chat().await;
        assert!(hub.state().lobby_chat.is_some());
        // Outro grupo, sem o `Delete` da sala velha: o chat não pode ficar
        // preso nela.
        hub.set_lobby(Lobby::from_json(
            &json!({"gameConfig": {"queueId": 420}, "multiUserChatId": "grupo-2"}),
        ));
        hub.ensure_lobby_chat().await;
        assert_eq!(hub.state().lobby_chat, None);
    }

    #[tokio::test]
    async fn mandar_sem_sala_ou_vazio_nao_fala_com_a_lcu() {
        let hub = Hub::new();
        assert!(matches!(
            hub.send_lobby_message("  ").await,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ));
        assert!(matches!(
            hub.send_lobby_message("oi").await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }
}
