//! Conversas 1:1 com amigos (#31).

use super::{command_result, not_connected, Hub, Shared};
use crate::chat::{self, ChatMessage, ChatUri, Conversation, SendMessageError};
use crate::rest::{LcuClient, LcuError};
use crate::wamp::{EventType, LcuEvent};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    /// Conversas 1:1 (#31): um evento do chat vira uma mensagem
    /// `conversation` ou `conversationClosed`.
    pub(super) fn apply_chat_event(&self, uri: ChatUri, event: &LcuEvent) {
        let deleted = event.event_type == EventType::Delete;
        match uri {
            ChatUri::Conversation(id) if deleted => self.close_conversation(&id),
            ChatUri::Conversation(id) => {
                // Salas da seleção de campeões e de grupo ficam de fora.
                let Some(mut conversation) = Conversation::from_json(&event.data) else {
                    return;
                };
                let last = conversation.last_message_from_json(&event.data);
                // O caminho do evento é o que a LCU aceita nos comandos.
                conversation.id = id;
                self.upsert_conversation(conversation, last);
            }
            // Mensagem apagada: o histórico da sessão fica como está.
            ChatUri::Messages(_) | ChatUri::Message(_) if deleted => {}
            ChatUri::Messages(id) => {
                self.merge_chat_messages(&id, |pid| ChatMessage::list_from_json(&event.data, pid))
            }
            ChatUri::Message(id) => self.merge_chat_messages(&id, |pid| {
                ChatMessage::from_json(&event.data, pid)
                    .into_iter()
                    .collect()
            }),
        }
    }

    /// Cria a conversa ou atualiza nome e não lidas, guardando o histórico,
    /// e junta `message` (a `lastMessage` do evento) a ele.
    pub(super) fn upsert_conversation(
        &self,
        conversation: Conversation,
        message: Option<ChatMessage>,
    ) {
        self.update(|shared| write_conversation(shared, conversation, message));
    }

    /// Junta ao histórico da conversa `id` as mensagens que `read` tirar do
    /// evento (ele recebe o `pid` do amigo). Mensagens de uma conversa que o
    /// agente não conhece (sala da seleção, de grupo) são ignoradas.
    pub(super) fn merge_chat_messages(
        &self,
        id: &str,
        read: impl FnOnce(&str) -> Vec<ChatMessage>,
    ) {
        self.update(|shared| write_chat_messages(shared, id, read));
    }

    pub(super) fn close_conversation(&self, id: &str) {
        self.update(|shared| {
            let conversations = &mut shared.state.conversations;
            let index = conversations.iter().position(|c| c.id == id)?;
            conversations.remove(index);
            Some(ServerMessage::ConversationClosed { id: id.to_owned() })
        });
    }

    /// Fecha todas as conversas (o LoL fechou).
    pub(super) fn clear_conversations(&self) {
        let ids: Vec<String> = self
            .lock()
            .state
            .conversations
            .iter()
            .map(|c| c.id.clone())
            .collect();
        for id in ids {
            self.close_conversation(&id);
        }
    }

    /// Aplica as conversas relidas pelo REST ao conectar, se a conexão ainda
    /// é a mesma. Uma conversa que o WAMP já trouxe é mais nova: dela só
    /// entram as mensagens, que se juntam sem repetir.
    pub(super) fn restore_conversations(
        &self,
        session: u64,
        conversations: Result<Vec<Conversation>, LcuError>,
    ) {
        // Qualquer erro deixa vazio; os próximos eventos trazem as conversas.
        let Ok(conversations) = conversations else {
            return;
        };
        for conversation in conversations {
            // A conferência e a escrita com a mesma trava: nada da conexão
            // antiga entra depois de uma queda.
            self.update(|shared| {
                if shared.session != session {
                    return None;
                }
                let known = shared
                    .state
                    .conversations
                    .iter()
                    .any(|c| c.id == conversation.id);
                if known {
                    let id = conversation.id.clone();
                    write_chat_messages(shared, &id, |_| conversation.messages)
                } else {
                    write_conversation(shared, conversation, None)
                }
            });
        }
    }

    /// Manda uma mensagem numa conversa 1:1 (comando `sendChatMessage`,
    /// #31). Publica a mensagem que a LCU respondeu antes de devolver, sem
    /// esperar o evento do WAMP, se a conexão ainda é a mesma.
    pub async fn send_chat_message(&self, conversation_id: &str, body: &str) -> CommandResult {
        if let Err(SendMessageError::Invalid(invalid)) = chat::check_message(conversation_id, body)
        {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid);
        }
        // O cliente e a sessão lidos juntos, como em `restore_conversations`.
        let Some((client, session)) = ({
            let shared = self.lock();
            shared.client.clone().map(|client| (client, shared.session))
        }) else {
            return not_connected();
        };
        match chat::send_message(&client, conversation_id, body).await {
            Ok(message) => {
                // A mensagem do WAMP tem o mesmo `id` e não repete. Se a
                // conexão mudou durante o envio, a resposta é da sessão
                // antiga e não entra numa conversa nova com o mesmo id (#87).
                if let Some(message) = message {
                    self.update(|shared| {
                        if shared.session != session {
                            return None;
                        }
                        write_chat_messages(shared, conversation_id, |_| vec![message])
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

    /// Marca a conversa que a UI está mostrando, ou nenhuma (comando
    /// `setActiveConversation`, #31).
    pub async fn set_active_conversation(&self, conversation_id: Option<&str>) -> CommandResult {
        match self.client() {
            Some(client) => command_result(chat::set_active(&client, conversation_id).await),
            None => not_connected(),
        }
    }
}

/// Cria a conversa ou atualiza nome e não lidas, guardando o histórico, e
/// junta `message` (a `lastMessage` do evento) a ele.
pub(super) fn write_conversation(
    shared: &mut Shared,
    conversation: Conversation,
    message: Option<ChatMessage>,
) -> Option<ServerMessage> {
    let conversations = &mut shared.state.conversations;
    let (index, created) = match conversations.iter().position(|c| c.id == conversation.id) {
        Some(index) => (index, false),
        None => {
            conversations.push(Conversation {
                messages: Vec::new(),
                ..conversation.clone()
            });
            (conversations.len() - 1, true)
        }
    };
    let current = &mut conversations[index];
    let before = current.clone();
    current.game_name = conversation.game_name;
    current.tag_line = conversation.tag_line;
    current.unread_count = conversation.unread_count;
    current.friend_pid = conversation.friend_pid;
    current.merge_messages(conversation.messages.into_iter().chain(message));
    // Conversa nova sempre é publicada, mesmo sem mensagens.
    if !created && *current == before {
        return None;
    }
    Some(ServerMessage::Conversation {
        conversation: current.clone(),
    })
}

/// Junta ao histórico da conversa `id` as mensagens de `read`, que recebe o
/// `pid` do amigo. Nada acontece se a conversa não existe.
pub(super) fn write_chat_messages(
    shared: &mut Shared,
    id: &str,
    read: impl FnOnce(&str) -> Vec<ChatMessage>,
) -> Option<ServerMessage> {
    let current = shared.state.conversations.iter_mut().find(|c| c.id == id)?;
    let messages = read(&current.friend_pid);
    if !current.merge_messages(messages) {
        return None;
    }
    Some(ServerMessage::Conversation {
        conversation: current.clone(),
    })
}

pub(super) async fn load_conversations(client: &LcuClient) -> Result<Vec<Conversation>, LcuError> {
    let mut conversations = chat::conversations(client).await?;
    let histories = futures_util::future::join_all(
        conversations
            .iter()
            .map(|conversation| chat::messages(client, conversation)),
    )
    .await;
    for (conversation, messages) in conversations.iter_mut().zip(histories) {
        conversation.merge_messages(messages.unwrap_or_default());
    }
    Ok(conversations)
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::connection::ConnectionEvent;
    use serde_json::json;
    use tokio::sync::broadcast;

    fn conversa(kind: &str, unread: u32, last: serde_json::Value) -> serde_json::Value {
        json!({
            "gameName": "Amigo", "gameTag": "BR1", "id": AMIGO, "pid": AMIGO,
            "isMuted": false, "lastMessage": last, "type": kind,
            "unreadMessageCount": unread,
        })
    }

    fn mensagem(id: &str, from_pid: &str, timestamp: &str) -> serde_json::Value {
        json!({
            "body": format!("oi {id}"), "fromId": from_pid, "fromPid": from_pid,
            "id": id, "isHistorical": false, "timestamp": timestamp, "type": "chat",
        })
    }

    fn conversas(rx: &mut broadcast::Receiver<ServerMessage>) -> Vec<ServerMessage> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[tokio::test]
    async fn segue_as_conversas_1_1() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let t1 = "2026-11-05T01:13:43.953Z";
        let t2 = "2026-11-05T01:13:56.197Z";

        // Sala da seleção de campeões: fora, e as mensagens dela também.
        let sala = "/lol-chat/v1/conversations/sala%40lol-champ-select.br1.pvp.net";
        let mut dados_sala = conversa("championSelect", 0, json!(null));
        dados_sala["id"] = json!("sala@lol-champ-select.br1.pvp.net");
        hub.apply(event(sala, EventType::Create, dados_sala)).await;
        hub.apply(event(
            &format!("{sala}/messages/m1"),
            EventType::Create,
            mensagem("m1", "x", t1),
        ))
        .await;
        // Mensagem antes da conversa: não há onde guardar.
        hub.apply(event(
            &format!("{AMIGO_URI}/messages/m0"),
            EventType::Create,
            mensagem("m0", AMIGO, t1),
        ))
        .await;
        // A sala vai para o chat da seleção (#27), nunca para as conversas.
        assert!(conversas(&mut rx)
            .iter()
            .all(|m| matches!(m, ServerMessage::ChampSelectChat { .. })));
        assert!(hub.state().conversations.is_empty());

        // A conversa abre, chega uma mensagem minha e uma do amigo.
        hub.apply(event(
            AMIGO_URI,
            EventType::Create,
            conversa("chat", 0, json!(null)),
        ))
        .await;
        hub.apply(event(
            &format!("{AMIGO_URI}/messages/1793841223953%3A6"),
            EventType::Create,
            mensagem("1793841223953:6", "eu@br1.pvp.net", t1),
        ))
        .await;
        let dele = mensagem("1793841419722:1", AMIGO, t2);
        hub.apply(event(
            AMIGO_URI,
            EventType::Update,
            conversa("chat", 1, dele.clone()),
        ))
        .await;
        // A mesma mensagem de novo, pelo URI dela e na lista inteira: nada muda.
        hub.apply(event(
            &format!("{AMIGO_URI}/messages/1793841419722%3A1"),
            EventType::Create,
            dele.clone(),
        ))
        .await;
        hub.apply(event(
            &format!("{AMIGO_URI}/messages"),
            EventType::Update,
            json!([dele]),
        ))
        .await;

        let seen = conversas(&mut rx);
        assert_eq!(seen.len(), 3, "{seen:?}");
        let ServerMessage::Conversation { conversation } = seen.last().unwrap() else {
            panic!("esperava conversation: {seen:?}");
        };
        assert_eq!(conversation.id, AMIGO);
        assert_eq!(conversation.unread_count, 1);
        let mensagens: Vec<(&str, bool)> = conversation
            .messages
            .iter()
            .map(|m| (m.id.as_str(), m.from_me))
            .collect();
        assert_eq!(
            mensagens,
            [("1793841223953:6", true), ("1793841419722:1", false)]
        );
        assert_eq!(hub.state().conversations.len(), 1);

        // Leu: o `Update` só muda as não lidas e mantém o histórico.
        hub.apply(event(
            AMIGO_URI,
            EventType::Update,
            conversa("chat", 0, mensagem("1793841419722:1", AMIGO, t2)),
        ))
        .await;
        match rx.try_recv().unwrap() {
            ServerMessage::Conversation { conversation } => {
                assert_eq!(conversation.unread_count, 0);
                assert_eq!(conversation.messages.len(), 2);
            }
            other => panic!("esperava conversation: {other:?}"),
        }

        hub.apply(event(AMIGO_URI, EventType::Delete, json!(null)))
            .await;
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::ConversationClosed { id: AMIGO.into() }
        );
        assert!(hub.state().conversations.is_empty());
    }

    #[tokio::test]
    async fn queda_fecha_as_conversas() {
        let hub = Hub::new();
        hub.apply(event(
            AMIGO_URI,
            EventType::Create,
            conversa("chat", 0, json!(null)),
        ))
        .await;
        let (_, mut rx) = hub.subscribe();
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert!(hub.state().conversations.is_empty());
        let fechadas: Vec<ServerMessage> = conversas(&mut rx)
            .into_iter()
            .filter(|m| matches!(m, ServerMessage::ConversationClosed { .. }))
            .collect();
        assert_eq!(
            fechadas,
            [ServerMessage::ConversationClosed { id: AMIGO.into() }]
        );
    }

    #[tokio::test]
    async fn comandos_de_chat_sem_lol_e_mensagem_vazia() {
        let hub = Hub::new();
        for result in [
            hub.send_chat_message(AMIGO, "oi").await,
            hub.set_active_conversation(Some(AMIGO)).await,
            hub.set_active_conversation(None).await,
        ] {
            assert!(matches!(
                result,
                CommandResult::Error {
                    code: ErrorCode::NotConnected,
                    ..
                }
            ));
        }
        // Mensagem vazia nem precisa do LoL.
        assert!(matches!(
            hub.send_chat_message(AMIGO, "   ").await,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ));
    }
}
