//! Endpoints quebrados de propósito, para testar o modo de compatibilidade
//! do agente (#36) sem esperar o LoL mudar.
//!
//! - [`Broken::missing`]: o caminho (e os de baixo dele) responde como a LCU
//!   responde a um caminho que ela não conhece: 404 com `errorCode`
//!   `RESOURCE_NOT_FOUND` e `message` "Invalid URI format", em qualquer
//!   método. É o que acontece quando a Riot tira ou renomeia um endpoint.
//! - [`Broken::bad_format`]: o `GET` do caminho responde 200 com um texto
//!   no lugar do JSON de sempre, como se o formato tivesse mudado.

use axum::http::Method;
use serde_json::{json, Value};

use crate::state::Response;

/// Os caminhos quebrados. Vazio por padrão.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Broken {
    pub missing: Vec<String>,
    pub bad_format: Vec<String>,
}

/// Corpo do `GET` de um caminho em [`Broken::bad_format`].
pub const BAD_FORMAT_BODY: &str = "formato-mudou-no-mock";

impl Broken {
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.bad_format.is_empty()
    }

    /// A resposta quebrada para `method` e `path` (sem a query), ou `None`
    /// se o caminho não está quebrado.
    pub fn respond(&self, method: &Method, path: &str) -> Option<Response> {
        if self.missing.iter().any(|p| covers(p, path)) {
            return Some(Response {
                status: 404,
                body: json!({
                    "errorCode": "RESOURCE_NOT_FOUND",
                    "httpStatus": 404,
                    "implementationDetails": {},
                    "message": "Invalid URI format",
                }),
            });
        }
        if *method == Method::GET && self.bad_format.iter().any(|p| p == path) {
            return Some(Response {
                status: 200,
                body: Value::String(BAD_FORMAT_BODY.into()),
            });
        }
        None
    }
}

/// `broken` cobre o próprio caminho e os que ficam embaixo dele.
fn covers(broken: &str, path: &str) -> bool {
    let broken = broken.trim_end_matches('/');
    path == broken
        || path
            .strip_prefix(broken)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caminho_quebrado_responde_como_a_lcu() {
        let broken = Broken {
            missing: vec!["/lol-lobby/v2/lobby".into()],
            bad_format: vec!["/lol-chat/v1/friends".into()],
        };
        let missing = broken.respond(&Method::GET, "/lol-lobby/v2/lobby").unwrap();
        assert_eq!(missing.status, 404);
        assert_eq!(missing.body["errorCode"], "RESOURCE_NOT_FOUND");
        assert!(broken
            .respond(&Method::POST, "/lol-lobby/v2/lobby/matchmaking/search")
            .is_some());
        assert!(broken
            .respond(&Method::GET, "/lol-lobby/v2/lobbyx")
            .is_none());
        assert!(broken
            .respond(&Method::GET, "/lol-lobby/v2/party")
            .is_none());

        let bad = broken
            .respond(&Method::GET, "/lol-chat/v1/friends")
            .unwrap();
        assert_eq!(
            (bad.status, bad.body.as_str()),
            (200, Some(BAD_FORMAT_BODY))
        );
        assert!(broken
            .respond(&Method::POST, "/lol-chat/v1/friends")
            .is_none());
    }
}
