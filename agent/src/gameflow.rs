//! Fase do gameflow (#16): em que ponto do caminho até a partida o jogador
//! está (lobby, fila, ready check, champ select, em jogo, pós-jogo).
//!
//! A LCU guarda a fase em `GET /lol-gameflow/v1/gameflow-phase` (uma string
//! JSON, como `"Lobby"`) e avisa cada mudança pelo WAMP no mesmo URI. O
//! [`Hub`](crate::ws::Hub) lê a fase pelo REST a cada conexão e depois segue
//! os eventos; a UI recebe a [`GameflowPhase`] pelo `/ws`.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) da fase do gameflow.
pub const GAMEFLOW_PHASE_PATH: &str = "/lol-gameflow/v1/gameflow-phase";

/// Fase do gameflow, com os mesmos nomes da LCU. Na ordem de uma partida:
///
/// | Fase | O que está acontecendo |
/// | --- | --- |
/// | `None` | Client aberto, fora de lobby |
/// | `Lobby` | Num lobby, montando o grupo |
/// | `Matchmaking` | Na fila |
/// | `CheckedIntoTournament` | Inscrito num torneio (Clash) |
/// | `ReadyCheck` | Partida encontrada, esperando aceitar |
/// | `ChampSelect` | Seleção de campeões |
/// | `GameStart` | A partida está abrindo |
/// | `FailedToLaunch` | A partida não abriu |
/// | `InProgress` | Em jogo |
/// | `Reconnect` | A partida está rolando e o jogador saiu dela |
/// | `WaitingForStats` | A partida acabou, esperando as estatísticas |
/// | `PreEndOfGame` | Telas antes do fim (honra, progresso) |
/// | `EndOfGame` | Tela de fim de partida |
/// | `TerminatedInError` | A partida terminou com erro |
///
/// Uma fase que a LCU inventar num patch futuro vira [`GameflowPhase::Unknown`]
/// em vez de erro, para o agente não parar de acompanhar o gameflow.
// O `Deserialize` é feito à mão (mais abaixo) para aceitar fases novas como
// `Unknown`; o `#[serde(other)]` faria o mesmo, mas o ts-rs não o entende.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
pub enum GameflowPhase {
    /// Client aberto, fora de lobby. A LCU chama de `None`.
    #[serde(rename = "None")]
    Idle,
    Lobby,
    Matchmaking,
    CheckedIntoTournament,
    ReadyCheck,
    ChampSelect,
    GameStart,
    FailedToLaunch,
    InProgress,
    Reconnect,
    WaitingForStats,
    PreEndOfGame,
    EndOfGame,
    TerminatedInError,
    /// Fase que o agente não conhece.
    Unknown,
}

impl GameflowPhase {
    /// Todas as fases, na ordem de uma partida, com [`GameflowPhase::Unknown`]
    /// no fim.
    pub const ALL: [GameflowPhase; 15] = [
        GameflowPhase::Idle,
        GameflowPhase::Lobby,
        GameflowPhase::Matchmaking,
        GameflowPhase::CheckedIntoTournament,
        GameflowPhase::ReadyCheck,
        GameflowPhase::ChampSelect,
        GameflowPhase::GameStart,
        GameflowPhase::FailedToLaunch,
        GameflowPhase::InProgress,
        GameflowPhase::Reconnect,
        GameflowPhase::WaitingForStats,
        GameflowPhase::PreEndOfGame,
        GameflowPhase::EndOfGame,
        GameflowPhase::TerminatedInError,
        GameflowPhase::Unknown,
    ];

    /// Fase com o nome da LCU; [`GameflowPhase::Unknown`] para um nome que o
    /// agente não conhece.
    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|phase| phase.as_str() == name)
            .unwrap_or(GameflowPhase::Unknown)
    }

    /// Nome da fase como a LCU escreve (`"None"`, `"Lobby"`...).
    pub fn as_str(self) -> &'static str {
        match self {
            GameflowPhase::Idle => "None",
            GameflowPhase::Lobby => "Lobby",
            GameflowPhase::Matchmaking => "Matchmaking",
            GameflowPhase::CheckedIntoTournament => "CheckedIntoTournament",
            GameflowPhase::ReadyCheck => "ReadyCheck",
            GameflowPhase::ChampSelect => "ChampSelect",
            GameflowPhase::GameStart => "GameStart",
            GameflowPhase::FailedToLaunch => "FailedToLaunch",
            GameflowPhase::InProgress => "InProgress",
            GameflowPhase::Reconnect => "Reconnect",
            GameflowPhase::WaitingForStats => "WaitingForStats",
            GameflowPhase::PreEndOfGame => "PreEndOfGame",
            GameflowPhase::EndOfGame => "EndOfGame",
            GameflowPhase::TerminatedInError => "TerminatedInError",
            GameflowPhase::Unknown => "Unknown",
        }
    }

    /// Lê a fase do `data` de um evento WAMP ou de uma resposta REST. `None`
    /// se o valor nem for uma string.
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        value.as_str().map(Self::from_name)
    }
}

impl<'de> Deserialize<'de> for GameflowPhase {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::from_name(&name))
    }
}

impl fmt::Display for GameflowPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Busca a fase atual do gameflow.
pub async fn current_phase(lcu: &LcuClient) -> Result<GameflowPhase, LcuError> {
    lcu.get(GAMEFLOW_PHASE_PATH).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mesmos_nomes_da_lcu() {
        for phase in GameflowPhase::ALL {
            assert_eq!(serde_json::to_value(phase).unwrap(), json!(phase.as_str()));
            assert_eq!(
                GameflowPhase::from_json(&json!(phase.as_str())),
                Some(phase)
            );
        }
        assert_eq!(
            GameflowPhase::from_json(&json!("None")),
            Some(GameflowPhase::Idle)
        );
        assert_eq!(GameflowPhase::ChampSelect.to_string(), "ChampSelect");
    }

    #[test]
    fn fase_nova_vira_unknown() {
        assert_eq!(
            GameflowPhase::from_json(&json!("FaseDeUmPatchNovo")),
            Some(GameflowPhase::Unknown)
        );
        assert_eq!(GameflowPhase::from_json(&json!(null)), None);
        assert_eq!(GameflowPhase::from_json(&json!({"phase": "Lobby"})), None);
        // Pelo serde também, como o REST lê.
        assert_eq!(
            serde_json::from_value::<GameflowPhase>(json!("FaseDeUmPatchNovo")).unwrap(),
            GameflowPhase::Unknown
        );
        assert!(serde_json::from_value::<GameflowPhase>(json!(42)).is_err());
    }
}
