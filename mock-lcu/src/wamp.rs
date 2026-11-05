//! WebSocket WAMP 1 da LCU, só o que o easylobby usa.
//!
//! O cliente manda `[5, "OnJsonApiEvent"]` para receber todos os eventos ou
//! `[5, "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase"]` para um recurso
//! (a URI sem a barra inicial, com `/` trocada por `_`). `[6, tópico]` cancela.
//! Cada evento chega como `[8, tópico, {"data", "eventType", "uri"}]`, uma vez
//! por inscrição que casa com ele.

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

/// Tópico com todos os eventos da API.
pub const ALL_EVENTS: &str = "OnJsonApiEvent";

pub const SUBSCRIBE: u64 = 5;
pub const UNSUBSCRIBE: u64 = 6;
pub const EVENT: u64 = 8;

/// Mensagem recebida do cliente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMessage {
    Subscribe(String),
    Unsubscribe(String),
}

/// Interpreta uma mensagem de texto do cliente. Devolve `None` para o que o
/// mock não trata (chamadas RPC, lixo).
pub fn parse(text: &str) -> Option<ClientMessage> {
    let Value::Array(frame) = serde_json::from_str::<Value>(text).ok()? else {
        return None;
    };
    let topic = frame.get(1)?.as_str()?.to_owned();
    match frame.first()?.as_u64()? {
        SUBSCRIBE => Some(ClientMessage::Subscribe(topic)),
        UNSUBSCRIBE => Some(ClientMessage::Unsubscribe(topic)),
        _ => None,
    }
}

/// Tópico de um recurso: `/lol-gameflow/v1/session` ->
/// `OnJsonApiEvent_lol-gameflow_v1_session`.
pub fn topic_for(uri: &str) -> String {
    format!(
        "{ALL_EVENTS}_{}",
        uri.trim_start_matches('/').replace('/', "_")
    )
}

/// Se uma inscrição em `topic` recebe o evento de `uri`. Um tópico de recurso
/// recebe também os sub-recursos (`..._lol-chat_v1_conversations` recebe as
/// mensagens de cada conversa).
pub fn matches(topic: &str, uri: &str) -> bool {
    if topic == ALL_EVENTS {
        return true;
    }
    let resource = topic_for(uri);
    resource == topic
        || resource
            .strip_prefix(topic)
            .is_some_and(|rest| rest.starts_with('_'))
}

/// Mensagem `[8, tópico, payload]` de um evento.
pub fn event_message(topic: &str, event: &EventFixture) -> String {
    json!([
        EVENT,
        topic,
        { "data": event.data, "eventType": event.event_type, "uri": event.uri }
    ])
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_inscricoes() {
        assert_eq!(
            parse(r#"[5, "OnJsonApiEvent"]"#),
            Some(ClientMessage::Subscribe(ALL_EVENTS.into()))
        );
        assert_eq!(
            parse(r#"[6, "OnJsonApiEvent"]"#),
            Some(ClientMessage::Unsubscribe(ALL_EVENTS.into()))
        );
        assert_eq!(parse(r#"[2, "id", "uri"]"#), None);
        assert_eq!(parse("lixo"), None);
    }

    #[test]
    fn casa_topicos() {
        let uri = "/lol-chat/v1/conversations/fake-pid-0002/messages";
        assert!(matches(ALL_EVENTS, uri));
        assert!(matches("OnJsonApiEvent_lol-chat_v1_conversations", uri));
        assert!(matches(&topic_for(uri), uri));
        assert!(!matches("OnJsonApiEvent_lol-chat_v1_conv", uri));
        assert!(!matches("OnJsonApiEvent_lol-lobby_v2_lobby", uri));
    }

    #[test]
    fn monta_evento() {
        let event = EventFixture {
            t_ms: 0,
            uri: "/lol-gameflow/v1/gameflow-phase".into(),
            event_type: "Update".into(),
            data: json!("Lobby"),
        };
        let msg: Value = serde_json::from_str(&event_message(ALL_EVENTS, &event)).unwrap();
        assert_eq!(
            msg,
            json!([8, "OnJsonApiEvent", { "data": "Lobby", "eventType": "Update", "uri": "/lol-gameflow/v1/gameflow-phase" }])
        );
    }
}
