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
//!
//! Conectado, o Vanguard ainda cai sozinho com o código 7003 se o vigia dele
//! não se ligar à LCU (#661); veja [`WATCHER_SIGNAL_PATH`].

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Sessão do Vanguard na LCU (e URI dos eventos dela).
pub const SESSION_PATH: &str = "/lol-vanguard/v1/session";

/// Ajuste local "fechar o client durante a partida" na LCU, e URI dos
/// eventos dele: o sinal do vigia do Vanguard (#661).
///
/// Cerca de 1 s depois de o Vanguard conectar, um programa de fora da LCU
/// (aqui, o "vigia"; pelos horários, é do próprio Vanguard) se liga a ela e
/// passa a regravar esse ajuste como desligado a cada 30 s. Cada gravação
/// vira um `Update` neste URI, e é o único sinal dele que a LCU dá. O LoL
/// também grava o ajuste uma vez ao subir, antes de o Vanguard conectar.
///
/// Em cerca de metade das aberturas do LoL (6 de 11 nos logs) a primeira
/// tentativa do vigia falha, e a segunda vem uns 91 s depois de o Vanguard
/// conectar. Nos logs, ele só se ligou com a UI oficial aberta, e sem ele o
/// Vanguard desconectou com o código 7003 de 146 a 150 s depois de conectar.
/// Com o vigia já ligado, fechar a UI não derrubou mais nada.
pub const WATCHER_SIGNAL_PATH: &str = "/lol-settings/v1/local/lol-user-experience";

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
