//! Convites recebidos e convites para o lobby (#32).

use super::{not_connected, Hub, Resource, Shared, Stamp};
use crate::invitations::{self, InvitationError, Invitee, ReceivedInvitation};
use crate::players::{self, RecentPlayer, SuggestedPlayer};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    pub fn set_received_invitations(&self, invitations: Vec<ReceivedInvitation>) {
        self.update(|shared| write_invitations(shared, invitations));
    }

    pub(super) fn set_received_invitations_if(
        &self,
        stamp: Stamp,
        invitations: Vec<ReceivedInvitation>,
    ) {
        self.update_if(Resource::Invitations, stamp, |shared| {
            write_invitations(shared, invitations)
        });
    }

    /// Relê pelo REST os convites recebidos e publica, se nada mais novo
    /// chegou durante a espera. Se a releitura falhar, o evento do WAMP
    /// corrige.
    pub(super) async fn refresh_received_invitations(&self) {
        if let Some((client, stamp)) = self.begin(Resource::Invitations) {
            if let Ok(list) = invitations::received_invitations(&client).await {
                self.set_received_invitations_if(stamp, list);
            }
        }
    }

    /// Aceita um convite recebido (comando `acceptInvitation`, #32). Relê os
    /// convites e o lobby pelo REST e publica antes de devolver.
    pub async fn accept_invitation(&self, invitation_id: &str) -> CommandResult {
        if let Err(invalid) = invitations::check_invitation_id(invitation_id) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        match invitations::accept(&client, invitation_id).await {
            Ok(()) => {
                // A LCU pode demorar um pouco para montar o lobby; se a
                // releitura ainda não o trouxer, o WAMP traz.
                tokio::join!(self.refresh_received_invitations(), self.refresh_lobby());
                CommandResult::Ok
            }
            Err(error) => invitation_error(error),
        }
    }

    /// Recusa um convite recebido (comando `declineInvitation`, #32). Relê os
    /// convites pelo REST e publica antes de devolver.
    pub async fn decline_invitation(&self, invitation_id: &str) -> CommandResult {
        if let Err(invalid) = invitations::check_invitation_id(invitation_id) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        match invitations::decline(&client, invitation_id).await {
            Ok(()) => {
                self.refresh_received_invitations().await;
                CommandResult::Ok
            }
            Err(error) => invitation_error(error),
        }
    }

    /// Convida jogadores para o lobby atual (comando `inviteToLobby`, #32).
    /// Relê o lobby, com os convites enviados, e publica antes de devolver.
    pub async fn invite_to_lobby(&self, invitees: &[Invitee]) -> CommandResult {
        if let Err(invalid) = invitations::check_invitees(invitees) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        match invitations::invite(&client, invitees).await {
            Ok(()) => {
                self.refresh_lobby().await;
                CommandResult::Ok
            }
            Err(error) => invitation_error(error),
        }
    }

    /// Comando `inviteByRiotId` (#206): acha o jogador e convida.
    pub async fn invite_by_riot_id(&self, game_name: &str, tag_line: &str) -> CommandResult {
        let Some((name, tag)) = crate::players::parse_riot_id(game_name, tag_line) else {
            return CommandResult::error(
                ErrorCode::InvalidCommand,
                "digite o nome de jogo e a tagline",
            );
        };
        let Some(client) = self.client() else {
            return not_connected();
        };
        match crate::players::lookup(&client, &name, &tag).await {
            Ok(Some(invitee)) => self.invite_to_lobby(&[invitee]).await,
            Ok(None) => CommandResult::error(
                ErrorCode::NotFound,
                format!("nenhum jogador com o Riot ID {name}#{tag}"),
            ),
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Comando `loadRecentPlayers` (#206): relê os jogadores das partidas
    /// recentes.
    pub async fn load_recent_players(&self) -> CommandResult {
        let Some((client, session, puuid)) = ({
            let shared = self.lock();
            shared.client.clone().map(|client| {
                let puuid = shared.state.summoner.as_ref().map(|s| s.puuid.clone());
                (client, shared.session, puuid)
            })
        }) else {
            return not_connected();
        };
        let Some(puuid) = puuid else {
            return CommandResult::error(ErrorCode::Lcu, "o invocador ainda não carregou");
        };
        match crate::players::recent(&client, &puuid).await {
            Ok(players) => {
                self.update(|shared| {
                    if shared.session != session || shared.state.recent_players == players {
                        return None;
                    }
                    shared.state.recent_players = players.clone();
                    Some(ServerMessage::RecentPlayers {
                        recent_players: players,
                    })
                });
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Comando `loadSuggestedPlayers` (#377): relê os jogadores que o client
    /// oficial sugere convidar.
    pub async fn load_suggested_players(&self) -> CommandResult {
        let Some((client, session, me)) = ({
            let shared = self.lock();
            shared.client.clone().map(|client| {
                let me = shared.state.summoner.as_ref().map_or(0, |s| s.summoner_id);
                (client, shared.session, me)
            })
        }) else {
            return not_connected();
        };
        match players::suggested(&client, me).await {
            Ok(list) => {
                self.update(|shared| {
                    if shared.session != session {
                        return None;
                    }
                    write_suggested_players(shared, list)
                });
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Comando `dismissSuggestedPlayer` (#377): tira o jogador da lista de
    /// sugeridos na LCU e publica a lista sem ele antes de devolver.
    pub async fn dismiss_suggested_player(&self, summoner_id: u64) -> CommandResult {
        let (client, listed) = {
            let shared = self.lock();
            let listed = shared
                .state
                .suggested_players
                .iter()
                .any(|p| p.summoner_id == summoner_id);
            (shared.client.clone(), listed)
        };
        let Some(client) = client else {
            return not_connected();
        };
        if !listed {
            return CommandResult::error(
                ErrorCode::InvalidCommand,
                "esse jogador não está nos sugeridos",
            );
        }
        if let Err(error) = players::dismiss_suggested(&client, summoner_id).await {
            return CommandResult::error(ErrorCode::Lcu, error.to_string());
        }
        self.update(|shared| {
            let mut list = shared.state.suggested_players.clone();
            list.retain(|p| p.summoner_id != summoner_id);
            write_suggested_players(shared, list)
        });
        CommandResult::Ok
    }

    pub fn set_suggested_players(&self, suggested_players: Vec<SuggestedPlayer>) {
        self.update(|shared| write_suggested_players(shared, suggested_players));
    }

    pub fn set_recent_players(&self, recent_players: Vec<RecentPlayer>) {
        self.update(|shared| {
            if shared.state.recent_players == recent_players {
                return None;
            }
            shared.state.recent_players = recent_players.clone();
            Some(ServerMessage::RecentPlayers { recent_players })
        });
    }
}

fn write_suggested_players(
    shared: &mut Shared,
    suggested_players: Vec<SuggestedPlayer>,
) -> Option<ServerMessage> {
    if shared.state.suggested_players == suggested_players {
        return None;
    }
    shared.state.suggested_players = suggested_players.clone();
    Some(ServerMessage::SuggestedPlayers { suggested_players })
}

/// Troca os convites recebidos, contando a escrita mesmo quando nada muda
/// (#73).
pub(super) fn write_invitations(
    shared: &mut Shared,
    invitations: Vec<ReceivedInvitation>,
) -> Option<ServerMessage> {
    shared.invitations_rev += 1;
    if shared.state.received_invitations == invitations {
        return None;
    }
    shared.state.received_invitations = invitations.clone();
    Some(ServerMessage::ReceivedInvitations { invitations })
}

pub(super) fn invitation_error(error: InvitationError) -> CommandResult {
    match error {
        InvitationError::Invalid(invalid) => {
            CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string())
        }
        InvitationError::Lcu(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::connection::ConnectionEvent;
    use crate::invitations::RECEIVED_INVITATIONS_PATH;
    use crate::wamp::EventType;
    use serde_json::json;

    fn received(ids: &[&str]) -> serde_json::Value {
        serde_json::Value::Array(
            ids.iter()
                .map(|id| {
                    json!({"invitationId": id, "state": "Pending", "fromSummonerId": 1003,
                           "canAcceptInvitation": true, "gameConfig": {"queueId": 400}})
                })
                .collect(),
        )
    }

    #[tokio::test]
    async fn sugeridos_pedem_o_lol_e_so_dispensam_quem_esta_na_lista() {
        let hub = Hub::new();
        let code = |result: CommandResult| match result {
            CommandResult::Error { code, .. } => Some(code),
            CommandResult::Ok => None,
        };
        assert_eq!(
            code(hub.load_suggested_players().await),
            Some(ErrorCode::NotConnected)
        );
        assert_eq!(
            code(hub.dismiss_suggested_player(10).await),
            Some(ErrorCode::NotConnected)
        );
        let (_, mut rx) = hub.subscribe();
        let list = crate::players::suggested_from_json(
            &serde_json::json!([{"summonerId": 10, "reason": "FriendOfFriend"}]),
            0,
        );
        hub.set_suggested_players(list.clone());
        assert_eq!(hub.state().suggested_players, list);
        assert_eq!(
            rx.recv().await.unwrap(),
            ServerMessage::SuggestedPlayers {
                suggested_players: list
            }
        );
    }

    #[tokio::test]
    async fn segue_os_convites_recebidos() {
        let hub = Hub::new();
        let (state, mut rx) = hub.subscribe();
        assert!(state.received_invitations.is_empty());
        hub.apply(event(
            RECEIVED_INVITATIONS_PATH,
            EventType::Update,
            received(&["a"]),
        ))
        .await;
        // Repetido não gera mensagem.
        hub.apply(event(
            RECEIVED_INVITATIONS_PATH,
            EventType::Update,
            received(&["a"]),
        ))
        .await;
        hub.apply(event(
            RECEIVED_INVITATIONS_PATH,
            EventType::Update,
            received(&["a", "b"]),
        ))
        .await;
        // JSON que não é lista: ignorado.
        hub.apply(event(
            RECEIVED_INVITATIONS_PATH,
            EventType::Update,
            json!({}),
        ))
        .await;
        hub.apply(event(
            RECEIVED_INVITATIONS_PATH,
            EventType::Update,
            received(&[]),
        ))
        .await;
        hub.apply(event(
            RECEIVED_INVITATIONS_PATH,
            EventType::Update,
            received(&["c"]),
        ))
        .await;
        hub.apply(event(
            RECEIVED_INVITATIONS_PATH,
            EventType::Delete,
            json!(null),
        ))
        .await;

        let seen: Vec<Vec<String>> = std::iter::from_fn(|| rx.try_recv().ok())
            .map(|m| match m {
                ServerMessage::ReceivedInvitations { invitations } => {
                    invitations.into_iter().map(|i| i.invitation_id).collect()
                }
                other => panic!("esperava receivedInvitations: {other:?}"),
            })
            .collect();
        assert_eq!(
            seen,
            [
                vec!["a".to_owned()],
                vec!["a".to_owned(), "b".to_owned()],
                vec![],
                vec!["c".to_owned()],
                vec![],
            ]
        );
    }

    #[tokio::test]
    async fn queda_limpa_os_convites() {
        let hub = Hub::new();
        hub.apply(event(
            RECEIVED_INVITATIONS_PATH,
            EventType::Update,
            received(&["a"]),
        ))
        .await;
        assert_eq!(hub.state().received_invitations.len(), 1);
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert!(hub.state().received_invitations.is_empty());
    }

    #[tokio::test]
    async fn comandos_de_convite() {
        let hub = Hub::new();
        let invitee = || Invitee {
            summoner_id: 1003,
            puuid: "p".into(),
        };
        for result in [
            hub.accept_invitation("a").await,
            hub.decline_invitation("a").await,
            hub.invite_to_lobby(&[invitee()]).await,
        ] {
            assert!(
                matches!(
                    result,
                    CommandResult::Error {
                        code: ErrorCode::NotConnected,
                        ..
                    }
                ),
                "{result:?}"
            );
        }
        // Valores inválidos nem precisam do LoL.
        for result in [
            hub.accept_invitation("../lobby").await,
            hub.decline_invitation("").await,
            hub.invite_to_lobby(&[]).await,
        ] {
            assert!(
                matches!(
                    result,
                    CommandResult::Error {
                        code: ErrorCode::InvalidCommand,
                        ..
                    }
                ),
                "{result:?}"
            );
        }
    }

    #[tokio::test]
    async fn convidar_por_nome_e_recentes_sem_lol() {
        let hub = Hub::new();
        assert!(matches!(
            hub.invite_by_riot_id("nome", " # ").await,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ));
        assert!(matches!(
            hub.invite_by_riot_id("nome", "BR1").await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
        assert!(matches!(
            hub.load_recent_players().await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }
}
