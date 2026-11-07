//! Chat do grupo no lobby (#206).

use super::{not_connected, Hub, Shared};

use serde_json::Value;

use crate::chat::{ChatUri, SendMessageError};
use crate::lobby_chat::{self, LobbyChat, LobbyChatMessage};
use crate::wamp::{EventType, LcuEvent};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    /// Sala do chat do grupo (#206): abre com o `Create` da conversa do
    /// lobby, junta as mensagens dela e fecha com o `Delete`. Devolve `false`
    /// quando o evento não é da sala, para ele seguir para o chat 1:1.
    pub(super) fn apply_lobby_chat_event(&self, uri: &ChatUri, event: &LcuEvent) -> bool {
        let deleted = event.event_type == EventType::Delete;
        match uri {
            ChatUri::Conversation(id) if deleted => self.close_lobby_chat(Some(id.as_str())),
            ChatUri::Conversation(id) => {
                let lobby_chat_id = self
                    .lock()
                    .state
                    .lobby
                    .as_ref()
                    .map(|l| l.chat_id.clone())
                    .unwrap_or_default();
                if !lobby_chat::is_room(&event.data, &lobby_chat_id) {
                    return false;
                }
                // O `id` do URI é o que a LCU aceita no envio. Uma sala nova
                // troca a anterior (que não fechou, se o `Delete` se perdeu).
                self.update(|shared| {
                    let state = &mut shared.state;
                    if state.lobby_chat.as_ref().is_some_and(|r| r.id == *id) {
                        return None;
                    }
                    let room = LobbyChat::new(id.clone());
                    state.lobby_chat = Some(room.clone());
                    Some(ServerMessage::LobbyChat {
                        lobby_chat: Some(room),
                    })
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
        }
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
            closed = true;
            Some(ServerMessage::LobbyChat { lobby_chat: None })
        });
        closed
    }

    /// Aplica a sala relida pelo REST ao conectar, se a conexão ainda é a
    /// mesma.
    pub(super) fn restore_lobby_chat(&self, session: u64, id: String, messages: &Value) {
        self.update(|shared| {
            if shared.session != session {
                return None;
            }
            let state = &mut shared.state;
            let known = state.lobby_chat.as_ref().is_some_and(|r| r.id == id);
            if !known {
                state.lobby_chat = Some(LobbyChat::new(id.clone()));
            }
            // Sem mensagem nova, a sala nova ainda precisa ser publicada.
            write_lobby_messages(shared, &id, messages).or_else(|| {
                (!known).then(|| ServerMessage::LobbyChat {
                    lobby_chat: shared.state.lobby_chat.clone(),
                })
            })
        });
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

/// Junta à sala do grupo `id` as mensagens de `value` (uma mensagem ou uma
/// lista). Nada muda se a sala aberta não for `id`.
fn write_lobby_messages(shared: &mut Shared, id: &str, value: &Value) -> Option<ServerMessage> {
    let state = &mut shared.state;
    let my_puuid = state.summoner.as_ref().map(|s| s.puuid.clone());
    let messages = LobbyChatMessage::list_from_json(value, my_puuid.as_deref());
    let room = state.lobby_chat.as_mut().filter(|r| r.id == id)?;
    if !room.merge_messages(messages) {
        return None;
    }
    Some(ServerMessage::LobbyChat {
        lobby_chat: Some(room.clone()),
    })
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
