//! Chat 1:1 com amigos (#31): as conversas abertas e o histórico da sessão.
//!
//! A LCU guarda as conversas em `/lol-chat/v1/conversations`:
//!
//! - `GET /lol-chat/v1/conversations` lista as conversas abertas. Além das
//!   conversas com amigos (`type: "chat"`), vêm as salas da seleção de
//!   campeões (`championSelect`), de partida personalizada e de grupo; o
//!   easylobby só usa as de `type: "chat"`.
//! - `GET /lol-chat/v1/conversations/{id}/messages` devolve as mensagens que
//!   a LCU tem da conversa (as desta sessão do LoL e as que ela trouxe do
//!   histórico do servidor).
//! - `POST /lol-chat/v1/conversations/{id}/messages {"body", "type": "chat"}`
//!   manda uma mensagem. Se a conversa ainda não está aberta, a LCU abre.
//! - `PUT /lol-chat/v1/conversations/active {"id"}` marca a conversa que o
//!   jogador está vendo (a LCU zera as não lidas dela) e `DELETE` no mesmo
//!   caminho desmarca. É o que a UI oficial faz ao abrir e fechar a janela
//!   da conversa.
//!
//! O `id` de uma conversa 1:1 é o `pid` do amigo no chat (`<puuid>@<região>`,
//! como `1234-abcd@br1.pvp.net`). Nos caminhos REST e nos URIs do WAMP ele
//! vai codificado (`%40` no lugar de `@`); veja [`conversation_path`].
//!
//! Pelo WAMP chegam a conversa (`Create`, `Update` com a última mensagem e as
//! não lidas, `Delete` quando fecha) em `/lol-chat/v1/conversations/{id}`,
//! cada mensagem nova em `/lol-chat/v1/conversations/{id}/messages/{msg}` e,
//! às vezes, a lista inteira em `/lol-chat/v1/conversations/{id}/messages`.
//! O [`Hub`](crate::ws::Hub) junta tudo numa [`Conversation`] por amigo, com
//! as mensagens da sessão em ordem (até [`MAX_MESSAGES`]).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint das conversas, e começo dos URIs dos eventos WAMP do chat.
pub const CONVERSATIONS_PATH: &str = "/lol-chat/v1/conversations";

/// Conversa que o jogador está vendo.
pub const ACTIVE_CONVERSATION_PATH: &str = "/lol-chat/v1/conversations/active";

/// Quantas mensagens o agente guarda por conversa. As mais velhas saem
/// primeiro.
pub const MAX_MESSAGES: usize = 200;

/// `type` da LCU para conversas e mensagens 1:1.
const CHAT_TYPE: &str = "chat";

/// Uma conversa 1:1 com um amigo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    /// Identificador da conversa na LCU, o `pid` do amigo no chat. É o que
    /// vai nos comandos `sendChatMessage` e `setActiveConversation`.
    pub id: String,
    /// Riot ID do amigo: nome e tag (`gameName#tagLine`).
    pub game_name: String,
    pub tag_line: String,
    /// Mensagens não lidas, como a LCU conta.
    pub unread_count: u32,
    /// Mensagens da sessão, da mais velha para a mais nova (até
    /// [`MAX_MESSAGES`]).
    pub messages: Vec<ChatMessage>,
    /// `pid` do amigo, para saber quem mandou cada mensagem. Na LCU é igual
    /// ao `id`; fica separado porque as fixtures sanitizadas trocam os dois
    /// por valores diferentes. Não vai para a UI.
    #[serde(skip)]
    #[ts(skip)]
    pub friend_pid: String,
}

/// Uma mensagem de uma conversa 1:1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    /// Identificador da mensagem na LCU, único dentro da conversa.
    pub id: String,
    pub body: String,
    /// `true` quando foi o jogador logado que mandou.
    pub from_me: bool,
    /// Quando foi mandada, em ISO 8601 UTC (`2026-11-05T01:13:41.755Z`).
    pub timestamp: String,
}

impl Conversation {
    /// Lê uma conversa do `data` de um evento WAMP ou de uma resposta REST,
    /// sem mensagens. `None` quando não é uma conversa 1:1 (sala da seleção
    /// de campeões, de grupo...) ou quando o valor não tem o formato
    /// esperado.
    pub fn from_json(value: &Value) -> Option<Self> {
        let raw = LcuConversation::deserialize(value).ok()?;
        if raw.kind != CHAT_TYPE || raw.id.is_empty() {
            return None;
        }
        let friend_pid = if raw.pid.is_empty() {
            raw.id.clone()
        } else {
            raw.pid
        };
        Some(Conversation {
            id: raw.id,
            game_name: raw.game_name,
            tag_line: raw.game_tag,
            unread_count: raw.unread_message_count,
            messages: Vec::new(),
            friend_pid,
        })
    }

    /// A última mensagem que vem junto com a conversa (`lastMessage`), já
    /// lida para esta conversa.
    pub fn last_message_from_json(&self, value: &Value) -> Option<ChatMessage> {
        ChatMessage::from_json(value.get("lastMessage")?, &self.friend_pid)
    }

    /// Junta `messages` ao histórico: troca as que já existiam (mesmo `id`),
    /// acrescenta as novas, ordena por horário e corta em [`MAX_MESSAGES`].
    /// Devolve se algo mudou.
    pub fn merge_messages(&mut self, messages: impl IntoIterator<Item = ChatMessage>) -> bool {
        let mut changed = false;
        for message in messages {
            match self.messages.iter_mut().find(|m| m.id == message.id) {
                Some(existing) if *existing == message => {}
                Some(existing) => {
                    *existing = message;
                    changed = true;
                }
                None => {
                    self.messages.push(message);
                    changed = true;
                }
            }
        }
        if changed {
            // Estável: mensagens do mesmo milissegundo ficam na ordem em que
            // chegaram. O horário da LCU tem sempre o mesmo formato, então a
            // ordem do texto é a ordem do tempo.
            self.messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            if self.messages.len() > MAX_MESSAGES {
                let excess = self.messages.len() - MAX_MESSAGES;
                self.messages.drain(..excess);
            }
        }
        changed
    }
}

impl ChatMessage {
    /// Lê uma mensagem da conversa com o amigo de `pid` `friend_pid`.
    /// `None` para mensagens de sistema (`type: "system"`, como os avisos de
    /// entrada na sala) e valores sem o formato esperado.
    pub fn from_json(value: &Value, friend_pid: &str) -> Option<Self> {
        let raw = LcuMessage::deserialize(value).ok()?;
        if raw.kind != CHAT_TYPE || raw.id.is_empty() {
            return None;
        }
        // Numa conversa 1:1 só há duas pessoas: o que não veio do amigo veio
        // do jogador logado. A LCU às vezes manda o `pid` em `fromId` e às
        // vezes o `puuid`; `fromPid` é sempre o `pid`.
        let from = if raw.from_pid.is_empty() {
            raw.from_id
        } else {
            raw.from_pid
        };
        Some(ChatMessage {
            id: raw.id,
            body: raw.body,
            from_me: from != friend_pid,
            timestamp: raw.timestamp,
        })
    }

    /// Lê uma lista de mensagens, pulando as que [`ChatMessage::from_json`]
    /// recusa.
    pub fn list_from_json(value: &Value, friend_pid: &str) -> Vec<Self> {
        value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|m| ChatMessage::from_json(m, friend_pid))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A conversa como a LCU manda. Os outros campos (`mucJwtDto`, `password`,
/// `targetRegion`...) não são usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcuConversation {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    pid: String,
    #[serde(default)]
    game_name: String,
    #[serde(default)]
    game_tag: String,
    #[serde(default)]
    unread_message_count: u32,
}

/// A mensagem como a LCU manda.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcuMessage {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    from_id: String,
    #[serde(default)]
    from_pid: String,
    #[serde(default)]
    timestamp: String,
}

/// Recurso do chat que um evento WAMP descreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatUri {
    /// `/lol-chat/v1/conversations/{id}`: a conversa.
    Conversation(String),
    /// `/lol-chat/v1/conversations/{id}/messages`: a lista de mensagens.
    Messages(String),
    /// `/lol-chat/v1/conversations/{id}/messages/{msg}`: uma mensagem.
    Message(String),
}

impl ChatUri {
    /// Lê o URI de um evento WAMP. `None` para os outros recursos do chat
    /// (participantes, conversa ativa, amigos...).
    pub fn parse(uri: &str) -> Option<Self> {
        let rest = uri.strip_prefix(CONVERSATIONS_PATH)?.strip_prefix('/')?;
        let mut parts = rest.split('/');
        let id = decode(parts.next()?)?;
        if id.is_empty() || id == "active" {
            return None;
        }
        match (parts.next(), parts.next(), parts.next()) {
            (None, _, _) => Some(ChatUri::Conversation(id)),
            (Some("messages"), None, _) => Some(ChatUri::Messages(id)),
            (Some("messages"), Some(msg), None) if !msg.is_empty() => Some(ChatUri::Message(id)),
            _ => None,
        }
    }
}

/// Caminho REST da conversa `id`, com o `id` codificado.
pub fn conversation_path(id: &str) -> String {
    format!("{CONVERSATIONS_PATH}/{}", encode(id))
}

/// As conversas 1:1 abertas, sem mensagens (`GET /lol-chat/v1/conversations`).
pub async fn conversations(lcu: &LcuClient) -> Result<Vec<Conversation>, LcuError> {
    let body: Value = lcu.get(CONVERSATIONS_PATH).await?;
    Ok(body
        .as_array()
        .map(|items| items.iter().filter_map(Conversation::from_json).collect())
        .unwrap_or_default())
}

/// As mensagens que a LCU tem da conversa
/// (`GET /lol-chat/v1/conversations/{id}/messages`).
pub async fn messages(
    lcu: &LcuClient,
    conversation: &Conversation,
) -> Result<Vec<ChatMessage>, LcuError> {
    let body: Value = lcu
        .get(&format!("{}/messages", conversation_path(&conversation.id)))
        .await?;
    Ok(ChatMessage::list_from_json(&body, &conversation.friend_pid))
}

/// Por que uma mensagem não foi mandada.
#[derive(Debug, thiserror::Error)]
pub enum SendMessageError {
    /// Mensagem vazia ou conversa sem `id`. Nada foi mandado para a LCU.
    #[error("{0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Lcu(#[from] LcuError),
}

/// Confere a mensagem antes de mandar: conversa com `id` e texto que não é
/// só espaço.
pub fn check_message(conversation_id: &str, body: &str) -> Result<(), SendMessageError> {
    if conversation_id.trim().is_empty() {
        return Err(SendMessageError::Invalid("conversa sem id"));
    }
    if body.trim().is_empty() {
        return Err(SendMessageError::Invalid("mensagem vazia"));
    }
    Ok(())
}

/// Manda `body` na conversa `id`
/// (`POST /lol-chat/v1/conversations/{id}/messages`), depois de conferir com
/// [`check_message`]. Devolve a mensagem que a LCU respondeu, quando ela
/// responde uma; a mensagem também chega pelo WAMP.
pub async fn send_message(
    lcu: &LcuClient,
    id: &str,
    body: &str,
) -> Result<Option<ChatMessage>, SendMessageError> {
    check_message(id, body)?;
    let response: Option<Value> = lcu
        .post(
            &format!("{}/messages", conversation_path(id)),
            &json!({ "body": body, "type": CHAT_TYPE }),
        )
        .await?;
    Ok(response.and_then(|m| ChatMessage::from_json(&m, id)))
}

/// Marca a conversa que o jogador está vendo
/// (`PUT /lol-chat/v1/conversations/active`), ou desmarca com `None`
/// (`DELETE`).
pub async fn set_active(lcu: &LcuClient, id: Option<&str>) -> Result<(), LcuError> {
    match id {
        Some(id) => lcu
            .put::<_, Option<Value>>(ACTIVE_CONVERSATION_PATH, &json!({ "id": id }))
            .await
            .map(|_| ()),
        None => lcu
            .delete::<Option<Value>>(ACTIVE_CONVERSATION_PATH)
            .await
            .map(|_| ()),
    }
}

/// Codifica um segmento de caminho: só letras, dígitos e `-._~` passam.
pub(crate) fn encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Desfaz [`encode`] (e qualquer `%XX`). `None` se o resultado não for UTF-8
/// ou se um `%` não vier seguido de dois dígitos hexadecimais.
fn decode(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMIGO: &str = "amigo-puuid@br1.pvp.net";

    fn conversa(kind: &str, unread: u32) -> Value {
        json!({
            "gameName": "Amigo",
            "gameTag": "BR1",
            "id": AMIGO,
            "isMuted": false,
            "lastMessage": null,
            "mucJwtDto": "x",
            "name": "",
            "pid": AMIGO,
            "targetRegion": "br1",
            "type": kind,
            "unreadMessageCount": unread,
        })
    }

    fn mensagem(id: &str, from_pid: &str, kind: &str, timestamp: &str) -> Value {
        json!({
            "body": format!("oi {id}"),
            "fromId": "qualquer-coisa",
            "fromPid": from_pid,
            "fromPuuid": "puuid",
            "fromSummonerId": 0,
            "id": id,
            "isHistorical": false,
            "timestamp": timestamp,
            "type": kind,
        })
    }

    #[test]
    fn le_so_conversas_1_1() {
        let c = Conversation::from_json(&conversa("chat", 2)).unwrap();
        assert_eq!(c.id, AMIGO);
        assert_eq!(
            (c.game_name.as_str(), c.tag_line.as_str()),
            ("Amigo", "BR1")
        );
        assert_eq!(c.unread_count, 2);
        assert!(c.messages.is_empty());
        assert_eq!(
            Conversation::from_json(&conversa("championSelect", 0)),
            None
        );
        assert_eq!(Conversation::from_json(&json!({})), None);
        assert_eq!(Conversation::from_json(&Value::Null), None);
    }

    #[test]
    fn le_mensagens_e_quem_mandou() {
        let t = "2026-11-05T01:13:41.755Z";
        let dele = ChatMessage::from_json(&mensagem("1", AMIGO, "chat", t), AMIGO).unwrap();
        assert!(!dele.from_me);
        assert_eq!(dele.body, "oi 1");
        assert_eq!(dele.timestamp, t);
        let minha = ChatMessage::from_json(&mensagem("2", "eu@br1.pvp.net", "chat", t), AMIGO);
        assert!(minha.unwrap().from_me);
        // Sem `fromPid`, vale o `fromId`.
        let mut sem_pid = mensagem("3", "", "chat", t);
        sem_pid["fromId"] = json!(AMIGO);
        assert!(!ChatMessage::from_json(&sem_pid, AMIGO).unwrap().from_me);
        // Mensagens de sistema ficam de fora.
        assert_eq!(
            ChatMessage::from_json(&mensagem("4", AMIGO, "system", t), AMIGO),
            None
        );
        let lista = json!([
            mensagem("1", AMIGO, "chat", t),
            mensagem("4", AMIGO, "system", t),
            {"lixo": true},
        ]);
        assert_eq!(ChatMessage::list_from_json(&lista, AMIGO).len(), 1);
    }

    #[test]
    fn junta_o_historico_em_ordem_e_sem_repetir() {
        let mut c = Conversation::from_json(&conversa("chat", 0)).unwrap();
        let m = |id: &str, t: &str| {
            ChatMessage::from_json(&mensagem(id, AMIGO, "chat", t), AMIGO).unwrap()
        };
        assert!(c.merge_messages([m("b", "2026-11-05T01:00:02.000Z")]));
        assert!(c.merge_messages([
            m("a", "2026-11-05T01:00:01.000Z"),
            m("b", "2026-11-05T01:00:02.000Z"),
        ]));
        // Tudo repetido: nada muda.
        assert!(!c.merge_messages([m("a", "2026-11-05T01:00:01.000Z")]));
        let ids: Vec<&str> = c.messages.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);

        // Passou do limite: saem as mais velhas.
        let muitas = (0..MAX_MESSAGES + 5).map(|i| {
            m(
                &format!("n{i:04}"),
                &format!("2026-11-05T02:{:02}:{:02}.000Z", i / 60, i % 60),
            )
        });
        assert!(c.merge_messages(muitas));
        assert_eq!(c.messages.len(), MAX_MESSAGES);
        assert_eq!(c.messages[0].id, "n0005");
    }

    #[test]
    fn last_message_vem_junto_com_a_conversa() {
        let mut data = conversa("chat", 1);
        data["lastMessage"] = mensagem("9", AMIGO, "chat", "2026-11-05T01:00:00.000Z");
        let c = Conversation::from_json(&data).unwrap();
        assert_eq!(c.last_message_from_json(&data).unwrap().id, "9");
        assert_eq!(c.last_message_from_json(&conversa("chat", 0)), None);
    }

    #[test]
    fn le_os_uris_do_chat() {
        let id = "amigo-puuid%40br1.pvp.net";
        let base = format!("{CONVERSATIONS_PATH}/{id}");
        assert_eq!(
            ChatUri::parse(&base),
            Some(ChatUri::Conversation(AMIGO.into()))
        );
        assert_eq!(
            ChatUri::parse(&format!("{base}/messages")),
            Some(ChatUri::Messages(AMIGO.into()))
        );
        assert_eq!(
            ChatUri::parse(&format!("{base}/messages/1793841223953%3A6")),
            Some(ChatUri::Message(AMIGO.into()))
        );
        for outro in [
            format!("{base}/participants"),
            format!("{base}/participants/x"),
            format!("{base}/messages/"),
            ACTIVE_CONVERSATION_PATH.to_owned(),
            CONVERSATIONS_PATH.to_owned(),
            format!("{CONVERSATIONS_PATH}/"),
            "/lol-chat/v1/friends/x".to_owned(),
            format!("{CONVERSATIONS_PATH}/quebrado%4"),
        ] {
            assert_eq!(ChatUri::parse(&outro), None, "{outro}");
        }
    }

    #[test]
    fn codifica_o_id_no_caminho() {
        assert_eq!(
            conversation_path(AMIGO),
            "/lol-chat/v1/conversations/amigo-puuid%40br1.pvp.net"
        );
        assert_eq!(
            conversation_path("a/b?c"),
            "/lol-chat/v1/conversations/a%2Fb%3Fc"
        );
        assert_eq!(decode(&encode("ç@ ~")).unwrap(), "ç@ ~");
    }

    #[test]
    fn confere_a_mensagem_antes_de_mandar() {
        assert!(check_message(AMIGO, "oi").is_ok());
        assert!(matches!(
            check_message(AMIGO, "  \n"),
            Err(SendMessageError::Invalid(_))
        ));
        assert!(matches!(
            check_message("", "oi"),
            Err(SendMessageError::Invalid(_))
        ));
    }
}
