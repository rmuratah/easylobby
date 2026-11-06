//! Chat da sala da seleção de campeões (#27).

use super::{not_connected, Hub, Shared};

use serde_json::Value;

use crate::champ_select_chat::{self, ChampSelectChat, ChampSelectChatMessage, Senders};
use crate::chat::{ChatUri, SendMessageError};
use crate::wamp::{EventType, LcuEvent};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    /// Quem trocou de célula na seleção leva as mensagens do chat junto
    /// (#27). Roda depois de toda escrita da seleção, venha ela do WAMP ou da
    /// releitura de um comando, e depois de a seleção ser publicada, para a
    /// UI já ter as células novas quando a sala chegar.
    pub(super) fn resolve_champ_select_chat_senders(&self) {
        self.update(|shared| {
            let state = &mut shared.state;
            let senders = Senders {
                my_puuid: state.summoner.as_ref().map(|s| s.puuid.as_str()),
                champ_select: state.champ_select.as_ref(),
            };
            let room = state.champ_select_chat.as_mut()?;
            if !room.resolve_senders(&senders) {
                return None;
            }
            Some(ServerMessage::ChampSelectChat {
                champ_select_chat: Some(room.clone()),
            })
        });
    }

    /// Sala do chat da seleção (#27): abre com o `Create` de uma conversa
    /// `championSelect`, junta as mensagens dela e fecha com o `Delete`.
    /// Devolve `false` quando o evento não é da sala, para ele seguir para o
    /// chat 1:1.
    pub(super) fn apply_champ_select_chat_event(&self, uri: &ChatUri, event: &LcuEvent) -> bool {
        let deleted = event.event_type == EventType::Delete;
        match uri {
            ChatUri::Conversation(id) if deleted => self.close_champ_select_chat(Some(id.as_str())),
            ChatUri::Conversation(id) => {
                if !champ_select_chat::is_room(&event.data) {
                    return false;
                }
                // O `id` do URI é o que a LCU aceita no envio. Uma sala nova
                // troca a anterior (que não fechou, se o `Delete` se perdeu).
                self.update(|shared| {
                    let state = &mut shared.state;
                    if state
                        .champ_select_chat
                        .as_ref()
                        .is_some_and(|r| r.id == *id)
                    {
                        return None;
                    }
                    let room = ChampSelectChat::new(id.clone());
                    state.champ_select_chat = Some(room.clone());
                    Some(ServerMessage::ChampSelectChat {
                        champ_select_chat: Some(room),
                    })
                });
                true
            }
            ChatUri::Messages(id) | ChatUri::Message(id) => {
                let mut handled = false;
                self.update(|shared| {
                    let is_room = shared
                        .state
                        .champ_select_chat
                        .as_ref()
                        .is_some_and(|r| r.id == *id);
                    handled = is_room;
                    // Mensagem apagada: o histórico fica como está.
                    if !is_room || deleted {
                        return None;
                    }
                    write_champ_select_messages(shared, id, &event.data)
                });
                handled
            }
        }
    }

    /// Fecha a sala da seleção: a de `id`, ou qualquer uma com `None` (o LoL
    /// fechou). Devolve se fechou alguma.
    pub(super) fn close_champ_select_chat(&self, id: Option<&str>) -> bool {
        let mut closed = false;
        self.update(|shared| {
            let room = shared.state.champ_select_chat.as_ref()?;
            if id.is_some_and(|id| id != room.id) {
                return None;
            }
            shared.state.champ_select_chat = None;
            closed = true;
            Some(ServerMessage::ChampSelectChat {
                champ_select_chat: None,
            })
        });
        closed
    }

    /// Aplica a sala relida pelo REST ao conectar, se a conexão ainda é a
    /// mesma. Como no chat 1:1, nada da conexão antiga entra depois de uma
    /// queda.
    pub(super) fn restore_champ_select_chat(&self, session: u64, id: String, messages: &Value) {
        self.update(|shared| {
            if shared.session != session {
                return None;
            }
            let state = &mut shared.state;
            let known = state.champ_select_chat.as_ref().is_some_and(|r| r.id == id);
            if !known {
                state.champ_select_chat = Some(ChampSelectChat::new(id.clone()));
            }
            // Sem mensagem nova, a sala nova ainda precisa ser publicada.
            write_champ_select_messages(shared, &id, messages).or_else(|| {
                (!known).then(|| ServerMessage::ChampSelectChat {
                    champ_select_chat: shared.state.champ_select_chat.clone(),
                })
            })
        });
    }

    /// Manda uma mensagem na sala da seleção aberta (comando
    /// `sendChampSelectMessage`, #27). Publica a mensagem que a LCU
    /// respondeu antes de devolver, sem esperar o evento do WAMP.
    pub async fn send_champ_select_message(&self, body: &str) -> CommandResult {
        if body.trim().is_empty() {
            return CommandResult::error(ErrorCode::InvalidCommand, "mensagem vazia");
        }
        let (client, session, room_id) = {
            let shared = self.lock();
            (
                shared.client.clone(),
                shared.session,
                shared
                    .state
                    .champ_select_chat
                    .as_ref()
                    .map(|r| r.id.clone()),
            )
        };
        let Some(client) = client else {
            return not_connected();
        };
        let Some(room_id) = room_id else {
            return CommandResult::error(
                ErrorCode::InvalidCommand,
                "o chat da seleção de campeões não está aberto",
            );
        };
        match champ_select_chat::send_message(&client, &room_id, body).await {
            Ok(message) => {
                // A mensagem do WAMP tem o mesmo `id` e não repete. Só entra
                // se a conexão e a sala ainda forem as mesmas.
                if let Some(message) = message {
                    self.update(|shared| {
                        if shared.session != session {
                            return None;
                        }
                        write_champ_select_messages(shared, &room_id, &message)
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

/// As conversas 1:1 abertas, cada uma com as mensagens que a LCU tem. Uma
/// conversa cujas mensagens não vieram entra sem elas.
/// Junta à sala da seleção `id` as mensagens de `value` (uma mensagem ou uma
/// lista), lidas com o jogador logado e o time da seleção atuais. Nada muda se
/// a sala aberta não for `id`.
pub(super) fn write_champ_select_messages(
    shared: &mut Shared,
    id: &str,
    value: &Value,
) -> Option<ServerMessage> {
    let state = &mut shared.state;
    let senders = Senders {
        my_puuid: state.summoner.as_ref().map(|s| s.puuid.as_str()),
        champ_select: state.champ_select.as_ref(),
    };
    let messages = ChampSelectChatMessage::list_from_json(value, &senders);
    let room = state.champ_select_chat.as_mut().filter(|r| r.id == id)?;
    if !room.merge_messages(messages) {
        return None;
    }
    Some(ServerMessage::ChampSelectChat {
        champ_select_chat: Some(room.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::champ_select::ChampSelect;
    use crate::champ_select::CHAMP_SELECT_SESSION_PATH;
    use crate::connection::ConnectionEvent;
    use crate::summoner::CURRENT_SUMMONER_PATH;
    use crate::ws::hub::Resource;
    use serde_json::json;
    use tokio::sync::broadcast;

    const SALA: &str = "sala@lol-champ-select.br1.pvp.net";
    const SALA_URI: &str = "/lol-chat/v1/conversations/sala%40lol-champ-select.br1.pvp.net";

    fn mensagem_da_sala(id: &str, puuid: &str, obfuscated: &str, kind: &str) -> serde_json::Value {
        json!({
            "body": format!("oi {id}"), "fromId": "0", "fromObfuscatedPuuid": obfuscated,
            "fromPid": "", "fromPuuid": puuid, "fromSummonerId": 0, "id": id,
            "isHistorical": false, "timestamp": format!("2026-11-05T01:14:4{id}.000Z"),
            "type": kind,
        })
    }

    fn salas(rx: &mut broadcast::Receiver<ServerMessage>) -> Vec<Option<ChampSelectChat>> {
        std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|m| match m {
                ServerMessage::ChampSelectChat { champ_select_chat } => Some(champ_select_chat),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn segue_o_chat_da_selecao() {
        let hub = Hub::new();
        hub.apply(event(
            CURRENT_SUMMONER_PATH,
            EventType::Update,
            json!({"gameName": "Eu", "tagLine": "BR1", "puuid": "eu-puuid", "summonerId": 1}),
        ))
        .await;
        // O jogador logado na célula 2 e um aliado escondido na 0.
        let mut sessao = champ_select("BAN_PICK", 0);
        sessao["localPlayerCellId"] = json!(2);
        sessao["myTeam"] = json!([
            {"cellId": 0, "puuid": "", "obfuscatedPuuid": "aliado", "assignedPosition": "top"},
            {"cellId": 2, "puuid": "eu-puuid", "obfuscatedPuuid": "", "assignedPosition": "middle"},
        ]);
        hub.apply(event(CHAMP_SELECT_SESSION_PATH, EventType::Create, sessao))
            .await;
        let (state, mut rx) = hub.subscribe();
        assert_eq!(state.champ_select_chat, None);

        // Mensagem de uma sala que ainda não abriu: não há onde guardar.
        hub.apply(event(
            &format!("{SALA_URI}/messages/1"),
            EventType::Create,
            mensagem_da_sala("1", "", "aliado", "chat"),
        ))
        .await;
        assert!(salas(&mut rx).is_empty());

        let mut dados = json!({"id": SALA, "type": "championSelect", "unreadMessageCount": 0});
        hub.apply(event(SALA_URI, EventType::Create, dados.clone()))
            .await;
        // Update da mesma sala não abre de novo.
        dados["unreadMessageCount"] = json!(1);
        hub.apply(event(SALA_URI, EventType::Update, dados)).await;
        assert_eq!(salas(&mut rx), [Some(ChampSelectChat::new(SALA))]);

        // Aviso de sistema fica de fora; o aliado e eu entramos.
        for (id, puuid, obfuscated, kind) in [
            ("2", "", "aliado", "system"),
            ("3", "", "aliado", "chat"),
            ("4", "eu-puuid", "", "chat"),
        ] {
            hub.apply(event(
                &format!("{SALA_URI}/messages/{id}"),
                EventType::Create,
                mensagem_da_sala(id, puuid, obfuscated, kind),
            ))
            .await;
        }
        // A lista inteira repetida não gera mensagem.
        hub.apply(event(
            &format!("{SALA_URI}/messages"),
            EventType::Update,
            json!([mensagem_da_sala("3", "", "aliado", "chat")]),
        ))
        .await;
        let vistas = salas(&mut rx);
        assert_eq!(vistas.len(), 2, "{vistas:?}");
        let sala = vistas.last().unwrap().clone().unwrap();
        let resumo: Vec<(&str, Option<i32>, bool)> = sala
            .messages
            .iter()
            .map(|m| (m.id.as_str(), m.from_cell_id, m.from_me))
            .collect();
        assert_eq!(resumo, [("3", Some(0), false), ("4", Some(2), true)]);
        assert_eq!(hub.state().champ_select_chat, Some(sala));
        assert!(hub.state().conversations.is_empty());

        // O aliado troca de lugar com o jogador logado: a sala chega de novo,
        // depois da seleção, com as células novas.
        let mut trocada = champ_select("BAN_PICK", 0);
        trocada["localPlayerCellId"] = json!(0);
        trocada["myTeam"] = json!([
            {"cellId": 2, "puuid": "", "obfuscatedPuuid": "aliado", "assignedPosition": "top"},
            {"cellId": 0, "puuid": "eu-puuid", "obfuscatedPuuid": "", "assignedPosition": "middle"},
        ]);
        hub.apply(event(CHAMP_SELECT_SESSION_PATH, EventType::Update, trocada))
            .await;
        let vistas: Vec<ServerMessage> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(matches!(
            vistas.as_slice(),
            [
                ServerMessage::ChampSelect { .. },
                ServerMessage::ChampSelectChat { .. }
            ]
        ));
        let celulas: Vec<Option<i32>> = hub
            .state()
            .champ_select_chat
            .unwrap()
            .messages
            .iter()
            .map(|m| m.from_cell_id)
            .collect();
        assert_eq!(celulas, [Some(2), Some(0)]);

        // A releitura de um comando (`set_champ_select_if`) também leva as
        // mensagens junto: o aliado volta para a célula 0.
        let mut de_volta = champ_select("BAN_PICK", 0);
        de_volta["localPlayerCellId"] = json!(2);
        de_volta["myTeam"] = json!([
            {"cellId": 0, "puuid": "", "obfuscatedPuuid": "aliado", "assignedPosition": "top"},
            {"cellId": 2, "puuid": "eu-puuid", "obfuscatedPuuid": "", "assignedPosition": "middle"},
        ]);
        let stamp = hub.lock().stamp(Resource::ChampSelect);
        assert!(hub.set_champ_select_if(stamp, ChampSelect::from_json(&de_volta)));
        let celulas: Vec<Option<i32>> = hub
            .state()
            .champ_select_chat
            .unwrap()
            .messages
            .iter()
            .map(|m| m.from_cell_id)
            .collect();
        assert_eq!(celulas, [Some(0), Some(2)]);
        let _ = salas(&mut rx);

        // Delete de outra conversa não fecha a sala; o da sala fecha.
        hub.apply(event(AMIGO_URI, EventType::Delete, json!(null)))
            .await;
        assert!(hub.state().champ_select_chat.is_some());
        hub.apply(event(SALA_URI, EventType::Delete, json!(null)))
            .await;
        assert_eq!(salas(&mut rx), [None]);
        // Mensagem atrasada da sala fechada: ignorada.
        hub.apply(event(
            &format!("{SALA_URI}/messages/5"),
            EventType::Create,
            mensagem_da_sala("5", "eu-puuid", "", "chat"),
        ))
        .await;
        assert!(salas(&mut rx).is_empty());
        assert!(hub.state().conversations.is_empty());

        // O LoL fecha com a sala aberta: ela some.
        hub.apply(event(
            SALA_URI,
            EventType::Create,
            json!({"id": SALA, "type": "championSelect"}),
        ))
        .await;
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(hub.state().champ_select_chat, None);
    }

    #[tokio::test]
    async fn chat_da_selecao_sem_lol_sem_sala_e_mensagem_vazia() {
        let hub = Hub::new();
        let code = |result: CommandResult| match result {
            CommandResult::Error { code, .. } => Some(code),
            CommandResult::Ok => None,
        };
        assert_eq!(
            code(hub.send_champ_select_message("  ").await),
            Some(ErrorCode::InvalidCommand)
        );
        assert_eq!(
            code(hub.send_champ_select_message("oi").await),
            Some(ErrorCode::NotConnected)
        );
    }
}
