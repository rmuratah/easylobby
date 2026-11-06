//! Partida encontrada (#20): o ready check, com aceitar e recusar.
//!
//! Quando a fila acha uma partida, o gameflow vai para `ReadyCheck` e a LCU
//! abre um ready check em `GET /lol-matchmaking/v1/ready-check`. Durante ele,
//! a LCU manda um evento WAMP no mesmo URI mais ou menos a cada segundo, com
//! `timer` (segundos desde que a partida foi encontrada) e a resposta do
//! jogador. Quando todos respondem ou o tempo acaba, chega um evento com o
//! estado final (`EveryoneReady`, `StrangerNotReady`...) e depois um
//! `Delete`.
//!
//! Fora de um ready check, a LCU responde 404 (fora da fila) ou um objeto com
//! `state: "Invalid"` (na fila, ou depois do ready check). Os dois viram
//! "sem ready check" ([`ReadyCheck::from_json`] devolve `None`).
//!
//! O aceite é sempre um clique do jogador: aceite automático fica fora do MVP
//! (#39).

use std::fmt;

use reqwest::Method;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) do ready check.
pub const READY_CHECK_PATH: &str = "/lol-matchmaking/v1/ready-check";

/// Aceita a partida encontrada.
pub const ACCEPT_PATH: &str = "/lol-matchmaking/v1/ready-check/accept";

/// Recusa a partida encontrada.
pub const DECLINE_PATH: &str = "/lol-matchmaking/v1/ready-check/decline";

/// Quanto tempo o jogador tem para responder, em segundos. A LCU não manda
/// esse valor. Medido no LoL real (patch 16.22): o `timer` chega a 13 ainda
/// com `state: "InProgress"` e o ready check fecha logo depois.
pub const READY_CHECK_DURATION_SECS: f64 = 13.0;

/// Um ready check aberto: o que a tela de partida encontrada precisa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReadyCheck {
    /// Em andamento ou o resultado final.
    pub state: ReadyCheckState,
    /// O que o jogador respondeu até agora.
    pub player_response: ReadyCheckResponse,
    /// Segundos desde que a partida foi encontrada, como a LCU conta. Chega
    /// de segundo em segundo; a UI pode animar entre um valor e outro.
    pub timer: f64,
    /// Tempo total para responder, em segundos. Contagem regressiva:
    /// `duration - timer`.
    pub duration: f64,
}

impl ReadyCheck {
    /// Lê o ready check do `data` de um evento WAMP ou de uma resposta REST.
    /// `None` quando não há ready check aberto (`state: "Invalid"`) ou quando
    /// o valor não tem o formato esperado.
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        let raw = LcuReadyCheck::deserialize(value).ok()?;
        if raw.state == ReadyCheckState::Invalid {
            return None;
        }
        Some(ReadyCheck {
            state: raw.state,
            player_response: raw.player_response,
            timer: raw.timer,
            duration: READY_CHECK_DURATION_SECS,
        })
    }

    /// Segundos que ainda faltam para responder, nunca negativo.
    pub fn seconds_left(&self) -> f64 {
        (self.duration - self.timer).max(0.0)
    }
}

/// O ready check como a LCU manda. Os outros campos (`declinerIds`,
/// `dodgeWarning`, `suppressUx`) não são usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcuReadyCheck {
    state: ReadyCheckState,
    player_response: ReadyCheckResponse,
    #[serde(default)]
    timer: f64,
}

/// Estado do ready check, com os nomes da LCU.
// `Deserialize` à mão para aceitar valores novos como `Unknown`, como em
// `GameflowPhase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
pub enum ReadyCheckState {
    /// Esperando as respostas.
    InProgress,
    /// Todos aceitaram; a seleção de campeões vai abrir.
    EveryoneReady,
    /// Alguém de fora do grupo recusou ou não respondeu. O jogador volta para
    /// a fila sozinho, se aceitou.
    StrangerNotReady,
    /// Alguém do grupo (talvez o próprio jogador) recusou ou não respondeu.
    /// O grupo sai da fila.
    PartyNotReady,
    /// Erro do lado da Riot.
    Error,
    /// Sem ready check. O agente não manda este valor para a UI: vira
    /// `readyCheck: null`.
    Invalid,
    /// Estado que o agente não conhece.
    Unknown,
}

impl ReadyCheckState {
    pub const ALL: [ReadyCheckState; 7] = [
        ReadyCheckState::InProgress,
        ReadyCheckState::EveryoneReady,
        ReadyCheckState::StrangerNotReady,
        ReadyCheckState::PartyNotReady,
        ReadyCheckState::Error,
        ReadyCheckState::Invalid,
        ReadyCheckState::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ReadyCheckState::InProgress => "InProgress",
            ReadyCheckState::EveryoneReady => "EveryoneReady",
            ReadyCheckState::StrangerNotReady => "StrangerNotReady",
            ReadyCheckState::PartyNotReady => "PartyNotReady",
            ReadyCheckState::Error => "Error",
            ReadyCheckState::Invalid => "Invalid",
            ReadyCheckState::Unknown => "Unknown",
        }
    }

    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.as_str() == name)
            .unwrap_or(ReadyCheckState::Unknown)
    }
}

impl<'de> Deserialize<'de> for ReadyCheckState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::from_name(&name))
    }
}

impl fmt::Display for ReadyCheckState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Resposta do jogador ao ready check, com os nomes da LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
pub enum ReadyCheckResponse {
    /// Ainda não respondeu. A LCU chama de `None`.
    #[serde(rename = "None")]
    Pending,
    Accepted,
    Declined,
    /// Resposta que o agente não conhece.
    Unknown,
}

impl ReadyCheckResponse {
    pub const ALL: [ReadyCheckResponse; 4] = [
        ReadyCheckResponse::Pending,
        ReadyCheckResponse::Accepted,
        ReadyCheckResponse::Declined,
        ReadyCheckResponse::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ReadyCheckResponse::Pending => "None",
            ReadyCheckResponse::Accepted => "Accepted",
            ReadyCheckResponse::Declined => "Declined",
            ReadyCheckResponse::Unknown => "Unknown",
        }
    }

    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|r| r.as_str() == name)
            .unwrap_or(ReadyCheckResponse::Unknown)
    }
}

impl<'de> Deserialize<'de> for ReadyCheckResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::from_name(&name))
    }
}

impl fmt::Display for ReadyCheckResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Busca o ready check aberto. `Ok(None)` fora de um ready check (404 ou
/// `state: "Invalid"`).
pub async fn current(lcu: &LcuClient) -> Result<Option<ReadyCheck>, LcuError> {
    match lcu.get::<serde_json::Value>(READY_CHECK_PATH).await {
        Ok(value) => Ok(ReadyCheck::from_json(&value)),
        Err(error) if error.status() == Some(404) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Aceita a partida (`POST /lol-matchmaking/v1/ready-check/accept`).
pub async fn accept(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.send::<()>(Method::POST, ACCEPT_PATH, None).await
}

/// Recusa a partida (`POST /lol-matchmaking/v1/ready-check/decline`).
pub async fn decline(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.send::<()>(Method::POST, DECLINE_PATH, None).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lcu(state: &str, response: &str, timer: f64) -> serde_json::Value {
        json!({
            "declinerIds": [],
            "dodgeWarning": "None",
            "playerResponse": response,
            "state": state,
            "suppressUx": false,
            "timer": timer,
        })
    }

    #[test]
    fn le_o_ready_check_da_lcu() {
        let check = ReadyCheck::from_json(&lcu("InProgress", "None", 3.0)).unwrap();
        assert_eq!(check.state, ReadyCheckState::InProgress);
        assert_eq!(check.player_response, ReadyCheckResponse::Pending);
        assert_eq!(check.timer, 3.0);
        assert_eq!(check.seconds_left(), READY_CHECK_DURATION_SECS - 3.0);

        let check = ReadyCheck::from_json(&lcu("EveryoneReady", "Accepted", 5.0)).unwrap();
        assert_eq!(check.state, ReadyCheckState::EveryoneReady);
        assert_eq!(check.player_response, ReadyCheckResponse::Accepted);

        let late = ReadyCheck::from_json(&lcu("PartyNotReady", "Declined", 40.0)).unwrap();
        assert_eq!(late.seconds_left(), 0.0);
    }

    #[test]
    fn invalid_e_lixo_viram_sem_ready_check() {
        assert_eq!(ReadyCheck::from_json(&lcu("Invalid", "None", 0.0)), None);
        assert_eq!(ReadyCheck::from_json(&json!(null)), None);
        assert_eq!(ReadyCheck::from_json(&json!("InProgress")), None);
        assert_eq!(ReadyCheck::from_json(&json!({"timer": 1.0})), None);
    }

    #[test]
    fn valores_novos_viram_unknown() {
        let check = ReadyCheck::from_json(&lcu("EstadoNovo", "RespostaNova", 1.0)).unwrap();
        assert_eq!(check.state, ReadyCheckState::Unknown);
        assert_eq!(check.player_response, ReadyCheckResponse::Unknown);
    }

    #[test]
    fn formato_para_a_ui() {
        let check = ReadyCheck::from_json(&lcu("InProgress", "None", 2.0)).unwrap();
        assert_eq!(
            serde_json::to_value(&check).unwrap(),
            json!({
                "state": "InProgress",
                "playerResponse": "None",
                "timer": 2.0,
                "duration": READY_CHECK_DURATION_SECS,
            })
        );
        for state in ReadyCheckState::ALL {
            assert_eq!(serde_json::to_value(state).unwrap(), json!(state.as_str()));
        }
        for response in ReadyCheckResponse::ALL {
            assert_eq!(
                serde_json::to_value(response).unwrap(),
                json!(response.as_str())
            );
        }
    }
}
