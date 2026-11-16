//! Cédula de honra no `/ws`: votar num aliado ou pular (#182).
//!
//! Veja [`crate::honor`]. A cédula vem pelo WAMP e, para não perder uma que
//! abriu antes da conexão ou um evento perdido, é relida pelo REST ao
//! conectar no pós-jogo e ao entrar em `PreEndOfGame` e `EndOfGame`.

use super::{command_result, not_connected, Hub, Shared};
use crate::gameflow::GameflowPhase;
use crate::honor::{self, HonorBallot};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

/// Fases em que a cédula pode estar aberta.
pub(super) fn post_game(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        Some(
            GameflowPhase::WaitingForStats | GameflowPhase::PreEndOfGame | GameflowPhase::EndOfGame
        )
    )
}

impl Hub {
    pub fn set_honor_ballot(&self, ballot: Option<HonorBallot>) {
        self.update(|shared| write_ballot(shared, ballot));
    }

    /// Relê a cédula pelo REST, se ainda estiver no pós-jogo da mesma
    /// conexão quando a resposta chegar.
    pub(super) fn spawn_refresh_honor_ballot(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            let Ok(ballot) = honor::current_ballot(&client).await else {
                return;
            };
            let current = {
                let shared = hub.lock();
                shared.session == session && post_game(shared.state.gameflow_phase)
            };
            // Uma cédula vazia na releitura não apaga a que veio pelo
            // evento: quem fecha é o `Delete` ou a volta ao lobby.
            if current && ballot.is_some() {
                hub.set_honor_ballot(ballot);
            }
        });
    }

    /// Honra o aliado `puuid` (comando `honorPlayer`).
    pub async fn honor_player(&self, puuid: &str) -> CommandResult {
        let Some(client) = self.client() else {
            return not_connected();
        };
        let Some(ballot) = self.lock().state.honor_ballot.clone() else {
            return no_ballot();
        };
        let Some(candidate) = ballot.candidate(puuid) else {
            return CommandResult::error(
                ErrorCode::InvalidCommand,
                "esse jogador não está na cédula de honra",
            );
        };
        command_result(honor::vote(&client, ballot.game_id, candidate).await)
    }

    /// Pula a votação de honra (comando `skipHonor`).
    pub async fn skip_honor(&self) -> CommandResult {
        let Some(client) = self.client() else {
            return not_connected();
        };
        let Some(game_id) = self.lock().state.honor_ballot.as_ref().map(|b| b.game_id) else {
            return no_ballot();
        };
        command_result(honor::skip(&client, game_id).await)
    }
}

fn no_ballot() -> CommandResult {
    CommandResult::error(ErrorCode::InvalidCommand, "não há cédula de honra aberta")
}

fn write_ballot(shared: &mut Shared, ballot: Option<HonorBallot>) -> Option<ServerMessage> {
    if shared.state.honor_ballot == ballot {
        return None;
    }
    shared.state.honor_ballot = ballot.clone();
    Some(ServerMessage::HonorBallot {
        honor_ballot: ballot,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::honor::BALLOT_PATH;
    use crate::wamp::EventType;
    use serde_json::json;

    fn cedula() -> serde_json::Value {
        json!({
            "eligibleAllies": [
                {"botPlayer": false, "championId": 897, "championName": "K'Sante",
                 "puuid": "fake-puuid-0092", "role": "TOP", "summonerId": 1134,
                 "summonerName": "fake-name-0129"}
            ],
            "eligibleOpponents": [],
            "gameId": 1124,
            "honoredPlayers": [],
            "votePool": {"votes": 1}
        })
    }

    #[tokio::test]
    async fn segue_a_cedula_pelo_wamp() {
        let hub = Hub::new();
        hub.apply(event(BALLOT_PATH, EventType::Create, cedula()))
            .await;
        let ballot = hub.state().honor_ballot.unwrap();
        assert_eq!(ballot.game_id, 1124);
        assert_eq!(ballot.eligible_allies[0].champion_name, "K'Sante");

        hub.apply(event(
            BALLOT_PATH,
            EventType::Delete,
            serde_json::Value::Null,
        ))
        .await;
        assert_eq!(hub.state().honor_ballot, None);
    }

    #[tokio::test]
    async fn cedula_vazia_e_volta_ao_lobby_fecham() {
        let hub = Hub::new();
        hub.apply(event(BALLOT_PATH, EventType::Create, cedula()))
            .await;
        hub.apply(event(
            BALLOT_PATH,
            EventType::Update,
            json!({"eligibleAllies": [], "gameId": 0}),
        ))
        .await;
        assert_eq!(hub.state().honor_ballot, None);

        hub.apply(event(BALLOT_PATH, EventType::Create, cedula()))
            .await;
        hub.apply(event(
            crate::gameflow::GAMEFLOW_PHASE_PATH,
            EventType::Update,
            json!("Lobby"),
        ))
        .await;
        assert_eq!(hub.state().honor_ballot, None);
    }

    #[tokio::test]
    async fn sem_conexao_ou_sem_cedula() {
        let hub = Hub::new();
        assert!(matches!(
            hub.skip_honor().await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
        assert!(matches!(
            hub.honor_player("fake-puuid-0092").await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }

    #[test]
    fn fases_do_pos_jogo() {
        assert!(post_game(Some(GameflowPhase::EndOfGame)));
        assert!(post_game(Some(GameflowPhase::PreEndOfGame)));
        assert!(!post_game(Some(GameflowPhase::Lobby)));
        assert!(!post_game(None));
    }
}
