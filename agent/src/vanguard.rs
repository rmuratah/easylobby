//! Estado do Vanguard, o anti-cheat da Riot (#579). Só leitura.
//!
//! Sem o Vanguard conectado a partida não abre: o servidor encerra a seleção
//! em `GAME_STARTING` com `PLAYER_LACKS_VANGUARD_SESSION` e conta esquiva
//! para o jogador. O client oficial avisa e desliga o botão Jogar; com a UI
//! oficial fechada, quem avisa é o easylobby.
//!
//! `GET /lol-vanguard/v1/session` responde `{"state":"CONNECTED",
//! "vanguardStatus":0}` com o Vanguard funcionando (conferido no 16.22), e o
//! WAMP avisa no mesmo URI quando o estado muda.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Sessão do Vanguard na LCU (e URI dos eventos dela).
pub const SESSION_PATH: &str = "/lol-vanguard/v1/session";

/// O estado com o Vanguard funcionando.
const CONNECTED: &str = "CONNECTED";

/// Estado do Vanguard que a LCU informou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Vanguard {
    /// Se o Vanguard está conectado. Desconectado, a partida não abre.
    pub connected: bool,
    /// O `state` da LCU como veio (`CONNECTED` com tudo certo), para o
    /// aviso e para quem reportar.
    pub state: String,
    /// O `vanguardStatus` da LCU (0 com tudo certo), ou `null`.
    pub status: Option<i32>,
}

impl Vanguard {
    /// Lê a resposta de [`SESSION_PATH`]. `None` se não tiver `state`.
    pub fn from_json(value: &Value) -> Option<Self> {
        let state = value.get("state")?.as_str()?.to_owned();
        Some(Self {
            connected: state.eq_ignore_ascii_case(CONNECTED),
            state,
            status: value
                .get("vanguardStatus")
                .and_then(Value::as_i64)
                .and_then(|n| i32::try_from(n).ok()),
        })
    }
}

/// Lê o estado do Vanguard. `Ok(None)` se a LCU não tiver o endpoint (404)
/// ou responder num formato desconhecido: aí o easylobby não avisa nada.
pub async fn current(lcu: &LcuClient) -> Result<Option<Vanguard>, LcuError> {
    match lcu.get::<Value>(SESSION_PATH).await {
        Ok(body) => Ok(Vanguard::from_json(&body)),
        Err(error) if error.status() == Some(404) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conectado() {
        let v = Vanguard::from_json(&json!({"state": "CONNECTED", "vanguardStatus": 0})).unwrap();
        assert!(v.connected);
        assert_eq!(v.state, "CONNECTED");
        assert_eq!(v.status, Some(0));
    }

    #[test]
    fn outro_estado_e_desconectado() {
        let v =
            Vanguard::from_json(&json!({"state": "NOT_CONNECTED", "vanguardStatus": 7})).unwrap();
        assert!(!v.connected);
        assert_eq!(v.status, Some(7));
    }

    #[test]
    fn sem_state_nao_vale() {
        assert_eq!(Vanguard::from_json(&json!({"vanguardStatus": 0})), None);
        assert_eq!(Vanguard::from_json(&json!(null)), None);
    }
}
