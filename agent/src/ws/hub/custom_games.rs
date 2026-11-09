//! Personalizadas abertas (#264): a lista e entrar numa delas.

use super::{not_connected, Hub, Resource, Shared};
use crate::custom_game_list::{self, CustomGameListing, MAX_PASSWORD_LEN};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    pub fn set_custom_games(&self, games: Option<Vec<CustomGameListing>>) {
        self.update(|shared| write_custom_games(shared, games));
    }

    /// Pede a lista de personalizadas abertas (comando `refreshCustomGames`).
    /// A lista chega numa mensagem `customGames` antes da resposta.
    pub async fn refresh_custom_games(&self) -> CommandResult {
        let Some((client, session, _)) = self.client_and_session() else {
            return not_connected();
        };
        match custom_game_list::refresh(&client).await {
            Ok(games) => {
                self.update_in_session(session, |shared| write_custom_games(shared, Some(games)));
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Entra na personalizada do grupo `party_id` (comando `joinCustomGame`).
    /// Sem senha numa partida que a lista diz ter senha recusa sem falar com
    /// a LCU. O lobby chega numa mensagem `lobby` antes da resposta.
    pub async fn join_custom_game(&self, party_id: &str, password: &str) -> CommandResult {
        if !custom_game_list::valid_party_id(party_id) {
            return invalid("partyId inválido");
        }
        if password.chars().count() > MAX_PASSWORD_LEN {
            return invalid(format!("a senha passa de {MAX_PASSWORD_LEN} caracteres"));
        }
        let needs_password = self
            .lock()
            .state
            .custom_games
            .iter()
            .flatten()
            .any(|game| game.party_id == party_id && game.has_password);
        if needs_password && password.is_empty() {
            return invalid("a personalizada pede senha");
        }
        let Some((client, _)) = self.begin(Resource::Lobby) else {
            return not_connected();
        };
        match custom_game_list::join(&client, party_id, password).await {
            Ok(()) => {
                self.refresh_lobby().await;
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }
}

fn invalid(message: impl Into<String>) -> CommandResult {
    CommandResult::error(ErrorCode::InvalidCommand, message)
}

fn write_custom_games(
    shared: &mut Shared,
    games: Option<Vec<CustomGameListing>>,
) -> Option<ServerMessage> {
    if shared.state.custom_games == games {
        return None;
    }
    shared.state.custom_games = games.clone();
    Some(ServerMessage::CustomGames {
        custom_games: games,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::wamp::EventType;
    use serde_json::json;

    fn code(result: CommandResult) -> ErrorCode {
        match result {
            CommandResult::Error { code, .. } => code,
            CommandResult::Ok => panic!("esperava erro"),
        }
    }

    #[tokio::test]
    async fn sem_lol_respondem_not_connected() {
        let hub = Hub::new();
        assert_eq!(
            code(hub.refresh_custom_games().await),
            ErrorCode::NotConnected
        );
        assert_eq!(
            code(hub.join_custom_game("p1", "").await),
            ErrorCode::NotConnected
        );
    }

    #[tokio::test]
    async fn lista_chega_pelo_wamp_e_pede_senha() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.apply(event(
            custom_game_list::CUSTOM_GAMES_PATH,
            EventType::Update,
            json!([
                {"id": 0, "partyId": "p1", "lobbyName": "Aberta", "hasPassword": false},
                {"id": 0, "partyId": "p2", "lobbyName": "Fechada", "hasPassword": true}
            ]),
        ))
        .await;
        let games = hub.state().custom_games.unwrap();
        assert_eq!(games.len(), 2);
        assert!(matches!(
            rx.try_recv(),
            Ok(ServerMessage::CustomGames {
                custom_games: Some(_)
            })
        ));

        assert_eq!(
            code(hub.join_custom_game("p2", "").await),
            ErrorCode::InvalidCommand
        );
        assert_eq!(
            code(hub.join_custom_game("", "").await),
            ErrorCode::InvalidCommand
        );
        assert_eq!(
            code(hub.join_custom_game("../lobby", "").await),
            ErrorCode::InvalidCommand
        );
        assert_eq!(
            code(
                hub.join_custom_game("p1", &"x".repeat(MAX_PASSWORD_LEN + 1))
                    .await
            ),
            ErrorCode::InvalidCommand
        );
        // Com a senha, ou numa sem senha, passa das regras (e para no LoL fechado).
        assert_eq!(
            code(hub.join_custom_game("p2", "segredo").await),
            ErrorCode::NotConnected
        );
        assert_eq!(
            code(hub.join_custom_game("p1", "").await),
            ErrorCode::NotConnected
        );
    }
}
