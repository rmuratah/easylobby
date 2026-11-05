//! Chat 1:1 da LCU falsa (#31): mandar mensagem e marcar a conversa ativa.
//!
//! - `POST /lol-chat/v1/conversations/{id}/messages {"body"}` cria a
//!   mensagem como vinda do jogador logado (o `pid` de `/lol-chat/v1/me`) e
//!   manda os eventos que a LCU manda: a conversa (criada, se ainda não
//!   existia, com o nome do amigo de `/lol-chat/v1/friends`), a mensagem e a
//!   lista de mensagens. Responde a mensagem criada.
//! - `PUT /lol-chat/v1/conversations/active {"id"}` marca a conversa ativa e
//!   zera as não lidas dela; `DELETE` no mesmo caminho desmarca.
//!
//! O amigo nunca responde: para ver mensagens chegando, use a reprodução da
//! gravação. As funções daqui não mexem no estado: devolvem a resposta e os
//! eventos, e quem chama emite os eventos.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::state::{LcuState, Response};

/// Conversas, e começo dos caminhos do chat.
pub const CONVERSATIONS: &str = "/lol-chat/v1/conversations";

/// Conversa ativa.
pub const ACTIVE: &str = "/lol-chat/v1/conversations/active";

const ME: &str = "/lol-chat/v1/me";
const FRIENDS: &str = "/lol-chat/v1/friends";

/// Numera as mensagens criadas pelo mock.
static NEXT_MESSAGE: AtomicU64 = AtomicU64::new(1);

/// Se `path` é `/lol-chat/v1/conversations/{id}/messages`; devolve o `{id}`
/// como veio (codificado).
pub fn messages_path(path: &str) -> Option<&str> {
    let id = path
        .strip_prefix(CONVERSATIONS)?
        .strip_prefix('/')?
        .strip_suffix("/messages")?;
    (!id.is_empty() && !id.contains('/') && id != "active").then_some(id)
}

/// `POST /lol-chat/v1/conversations/{id}/messages`, com `encoded_id` como
/// veio no caminho.
pub fn send(state: &LcuState, encoded_id: &str, body: &Value) -> (Response, Vec<EventFixture>) {
    let Some(text) = body["body"].as_str().filter(|t| !t.trim().is_empty()) else {
        return (
            Response::error(400, "mock-lcu: mensagem sem body"),
            Vec::new(),
        );
    };
    let id = decode(encoded_id);
    let conversation_uri = format!("{CONVERSATIONS}/{encoded_id}");
    let messages_uri = format!("{conversation_uri}/messages");

    let me = state.get(ME).body;
    let my_pid = me["pid"].as_str().unwrap_or("mock-pid").to_owned();
    let n = NEXT_MESSAGE.fetch_add(1, Ordering::Relaxed);
    let message_id = format!("{}:{n}", now_millis());
    let message = json!({
        "body": text,
        "fromId": my_pid,
        "fromObfuscatedPuuid": "",
        "fromObfuscatedSummonerId": 0,
        "fromPid": my_pid,
        "fromPuuid": me["puuid"],
        "fromSummonerId": 0,
        "id": message_id,
        "isHistorical": false,
        "timestamp": iso_now(),
        "type": "chat",
    });

    let current = state.get(&conversation_uri);
    let (event_type, mut conversation) = if current.status == 200 {
        ("Update", current.body)
    } else {
        ("Create", new_conversation(state, &id))
    };
    conversation["lastMessage"] = message.clone();

    let mut history = state.get(&messages_uri).body;
    if !history.is_array() {
        history = json!([]);
    }
    if let Some(items) = history.as_array_mut() {
        items.push(message.clone());
    }

    let events = vec![
        event(&conversation_uri, event_type, conversation),
        event(
            &format!("{messages_uri}/{}", encode(&message_id)),
            "Create",
            message.clone(),
        ),
        event(&messages_uri, "Update", history),
    ];
    (
        Response {
            status: 200,
            body: message,
        },
        events,
    )
}

/// `PUT /lol-chat/v1/conversations/active {"id"}`.
pub fn set_active(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let Some(id) = body["id"].as_str().filter(|id| !id.is_empty()) else {
        return (
            Response::error(400, "mock-lcu: conversa sem id"),
            Vec::new(),
        );
    };
    let mut events = vec![event(ACTIVE, "Update", json!({ "id": id }))];
    let uri = format!("{CONVERSATIONS}/{}", encode(id));
    let current = state.get(&uri);
    if current.status == 200 && current.body["unreadMessageCount"] != json!(0) {
        let mut conversation = current.body;
        conversation["unreadMessageCount"] = json!(0);
        events.push(event(&uri, "Update", conversation));
    }
    (no_content(), events)
}

/// `DELETE /lol-chat/v1/conversations/active`.
pub fn clear_active() -> (Response, Vec<EventFixture>) {
    (no_content(), vec![event(ACTIVE, "Delete", Value::Null)])
}

/// Conversa nova com o amigo `id`, com o nome dele quando está na lista de
/// amigos.
fn new_conversation(state: &LcuState, id: &str) -> Value {
    let friends = state.get(FRIENDS).body;
    let friend = friends
        .as_array()
        .and_then(|fs| fs.iter().find(|f| f["id"] == id || f["pid"] == id));
    let field = |name: &str| friend.map_or(json!(""), |f| f[name].clone());
    json!({
        "gameName": field("gameName"),
        "gameTag": field("gameTag"),
        "id": id,
        "inviterId": "",
        "isMuted": false,
        "lastMessage": null,
        "name": "",
        "password": "",
        "pid": id,
        "targetRegion": "",
        "type": "chat",
        "unreadMessageCount": 0,
    })
}

fn no_content() -> Response {
    Response {
        status: 204,
        body: Value::Null,
    }
}

fn event(uri: &str, event_type: &str, data: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.into(),
        event_type: event_type.into(),
        data,
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

/// Agora em ISO 8601 UTC com milissegundos, como a LCU
/// (`2026-11-05T01:13:41.755Z`).
fn iso_now() -> String {
    iso(now_millis())
}

fn iso(millis: u128) -> String {
    let secs = millis / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Dias desde 1970-01-01 para data civil (algoritmo de Howard Hinnant).
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        millis % 1000
    )
}

/// Codifica um segmento de caminho como a LCU (`@` vira `%40`).
fn encode(segment: &str) -> String {
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

fn decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_o_caminho_das_mensagens() {
        assert_eq!(
            messages_path("/lol-chat/v1/conversations/a%40br1.pvp.net/messages"),
            Some("a%40br1.pvp.net")
        );
        assert_eq!(
            messages_path("/lol-chat/v1/conversations/a/messages/1"),
            None
        );
        assert_eq!(messages_path("/lol-chat/v1/conversations//messages"), None);
        assert_eq!(messages_path("/lol-chat/v1/conversations/a"), None);
    }

    #[test]
    fn horario_no_formato_da_lcu() {
        assert_eq!(iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso(1_793_841_221_755), "2026-11-05T01:13:41.755Z");
        assert_eq!(iso(951_782_400_001), "2000-02-29T00:00:00.001Z");
    }

    #[test]
    fn codifica_e_decodifica() {
        assert_eq!(encode("a@b.c"), "a%40b.c");
        assert_eq!(decode("a%40b.c"), "a@b.c");
        assert_eq!(decode("100%"), "100%");
    }
}
