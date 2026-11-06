//! Ready check: a partida encontrada, aceitar e recusar (#20).

use super::{command_result, not_connected, Hub};
use crate::ready_check::{self, ReadyCheck};
use crate::ws::protocol::{CommandResult, ServerMessage};

impl Hub {
    pub fn set_ready_check(&self, ready_check: Option<ReadyCheck>) {
        self.update(|shared| {
            if shared.state.ready_check == ready_check {
                return None;
            }
            shared.state.ready_check = ready_check.clone();
            Some(ServerMessage::ReadyCheck { ready_check })
        });
    }

    /// Aceita a partida encontrada (comando `acceptReadyCheck`, #20).
    pub async fn accept_ready_check(&self) -> CommandResult {
        match self.client() {
            Some(client) => command_result(ready_check::accept(&client).await),
            None => not_connected(),
        }
    }

    /// Recusa a partida encontrada (comando `declineReadyCheck`, #20).
    pub async fn decline_ready_check(&self) -> CommandResult {
        match self.client() {
            Some(client) => command_result(ready_check::decline(&client).await),
            None => not_connected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::connection::ConnectionEvent;
    use crate::ready_check::READY_CHECK_PATH;
    use crate::wamp::EventType;
    use crate::ws::protocol::ErrorCode;
    use serde_json::json;

    fn ready_check(state: &str, response: &str, timer: f64) -> ConnectionEvent {
        event(
            READY_CHECK_PATH,
            EventType::Update,
            json!({
                "declinerIds": [],
                "dodgeWarning": "None",
                "playerResponse": response,
                "state": state,
                "suppressUx": false,
                "timer": timer,
            }),
        )
    }

    #[tokio::test]
    async fn segue_o_ready_check() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        // Na fila a LCU já manda o ready check com `Invalid`: nada muda.
        hub.apply(ready_check("Invalid", "None", 0.0)).await;
        assert!(rx.try_recv().is_err());

        hub.apply(ready_check("InProgress", "None", 0.0)).await;
        hub.apply(ready_check("InProgress", "Accepted", 1.0)).await;
        hub.apply(ready_check("EveryoneReady", "Accepted", 2.0))
            .await;
        hub.apply(event(READY_CHECK_PATH, EventType::Delete, json!(null)))
            .await;

        let mut seen = Vec::new();
        while let Ok(ServerMessage::ReadyCheck { ready_check }) = rx.try_recv() {
            seen.push(ready_check.map(|c| (c.state.as_str(), c.player_response.as_str(), c.timer)));
        }
        assert_eq!(
            seen,
            [
                Some(("InProgress", "None", 0.0)),
                Some(("InProgress", "Accepted", 1.0)),
                Some(("EveryoneReady", "Accepted", 2.0)),
                None,
            ]
        );
        assert_eq!(hub.state().ready_check, None);
    }

    #[tokio::test]
    async fn queda_fecha_o_ready_check() {
        let hub = Hub::new();
        hub.apply(ready_check("InProgress", "None", 4.0)).await;
        assert!(hub.state().ready_check.is_some());
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(hub.state().ready_check, None);
    }

    #[tokio::test]
    async fn aceitar_e_recusar_sem_lol_respondem_not_connected() {
        let hub = Hub::new();
        for result in [
            hub.accept_ready_check().await,
            hub.decline_ready_check().await,
        ] {
            assert!(matches!(
                result,
                CommandResult::Error {
                    code: ErrorCode::NotConnected,
                    ..
                }
            ));
        }
    }
}
