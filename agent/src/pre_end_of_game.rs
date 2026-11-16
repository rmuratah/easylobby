//! Sequência de telas antes do fim de partida (`PreEndOfGame`, #180).
//!
//! Quando o jogo fecha, o LoL passa pela fase `PreEndOfGame`, uma fila de
//! telas que a UI oficial mostra em ordem (votação de honra, desafios,
//! missões, PDL). A tela atual fica em
//! `GET /lol-pre-end-of-game/v1/currentSequenceEvent`, por exemplo
//! `{"name": "honor-vote", "priority": 1}`, e `{"name": "", "priority": -1}`
//! quando não há nenhuma. A UI oficial avisa que cada tela acabou com
//! `POST /lol-pre-end-of-game/v1/complete/{name}`, e a fase só vai para
//! `EndOfGame` depois da última.
//!
//! Com a UI oficial fechada, ninguém faz esse aviso: no teste no LoL real
//! (16.22, ranqueada) a fase ficou parada em `honor-vote` até o jogador
//! voltar ao lobby. Com a UI aberta, a `honor-vote` acaba em uns 2 s e a
//! cédula de honra continua valendo em `EndOfGame`. O
//! [`Hub`](crate::ws::Hub) faz o mesmo aviso quando o agente fechou a UI.

use serde::Deserialize;

use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) da tela atual da sequência.
pub const CURRENT_SEQUENCE_EVENT_PATH: &str = "/lol-pre-end-of-game/v1/currentSequenceEvent";

/// Prefixo do aviso de tela concluída; o nome da tela vem depois.
pub const COMPLETE_PREFIX: &str = "/lol-pre-end-of-game/v1/complete/";

/// Tela atual da sequência, como a LCU manda.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SequenceEvent {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub priority: i64,
}

impl SequenceEvent {
    /// Lê o `data` de um evento ou a resposta do REST.
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }

    /// O nome da tela que está esperando, ou `None` sem tela.
    pub fn pending(&self) -> Option<&str> {
        let name = self.name.trim();
        (!name.is_empty()).then_some(name)
    }
}

/// Caminho que avisa que a tela `name` acabou.
pub fn complete_path(name: &str) -> String {
    let encoded: String = name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("{COMPLETE_PREFIX}{encoded}")
}

/// Tela atual da sequência (`GET .../currentSequenceEvent`).
pub async fn current(client: &LcuClient) -> Result<SequenceEvent, LcuError> {
    client.get(CURRENT_SEQUENCE_EVENT_PATH).await
}

/// Avisa que a tela `name` acabou (`POST .../complete/{name}`).
pub async fn complete(client: &LcuClient, name: &str) -> Result<(), LcuError> {
    client
        .send::<()>(reqwest::Method::POST, &complete_path(name), None)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_a_tela_atual() {
        let event =
            SequenceEvent::from_json(&json!({"name": "honor-vote", "priority": 1})).unwrap();
        assert_eq!(event.pending(), Some("honor-vote"));
        let empty = SequenceEvent::from_json(&json!({"name": "", "priority": -1})).unwrap();
        assert_eq!(empty.pending(), None);
        assert_eq!(SequenceEvent::from_json(&json!(null)), None);
    }

    #[test]
    fn caminho_do_aviso() {
        assert_eq!(
            complete_path("honor-vote"),
            "/lol-pre-end-of-game/v1/complete/honor-vote"
        );
        assert_eq!(
            complete_path("a b/c"),
            "/lol-pre-end-of-game/v1/complete/a%20b%2Fc"
        );
    }
}
