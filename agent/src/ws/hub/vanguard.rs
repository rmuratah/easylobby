//! Aviso de Vanguard desconectado (#579). Veja [`crate::vanguard`].
//!
//! O agente lê o estado ao conectar e no login, e aplica os eventos do WAMP
//! no mesmo URI. Com o LoL fechado, o estado volta a `null`.

use super::Hub;
use crate::vanguard::{self, Vanguard};
use crate::ws::protocol::ServerMessage;

impl Hub {
    pub fn set_vanguard(&self, vanguard: Option<Vanguard>) {
        self.update(|shared| {
            if shared.state.vanguard == vanguard {
                return None;
            }
            if let Some(v) = vanguard.as_ref().filter(|v| !v.connected) {
                tracing::warn!(
                    "Vanguard desconectado (state {}, vanguardStatus {:?}): a partida não vai abrir",
                    v.state,
                    v.status
                );
            }
            shared.state.vanguard = vanguard.clone();
            Some(ServerMessage::Vanguard { vanguard })
        });
    }

    /// Relê o estado do Vanguard numa tarefa separada. Uma falha (LoL
    /// subindo) deixa o que já estava; o evento ou o login corrigem.
    pub(super) fn spawn_refresh_vanguard(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            let Ok(read) = vanguard::current(&client).await else {
                return;
            };
            if hub.lock().session != session {
                return;
            }
            hub.set_vanguard(read);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::wamp::EventType;
    use serde_json::json;

    #[tokio::test]
    async fn evento_do_vanguard_atualiza_o_estado() {
        let hub = Hub::new();
        hub.apply(event(
            vanguard::SESSION_PATH,
            EventType::Update,
            json!({"state": "NOT_CONNECTED", "vanguardStatus": 3}),
        ))
        .await;
        let v = hub.lock().state.vanguard.clone().unwrap();
        assert!(!v.connected);
        assert_eq!(v.state, "NOT_CONNECTED");

        hub.apply(event(
            vanguard::SESSION_PATH,
            EventType::Update,
            json!({"state": "CONNECTED", "vanguardStatus": 0}),
        ))
        .await;
        assert!(hub.lock().state.vanguard.as_ref().unwrap().connected);

        hub.apply(event(
            vanguard::SESSION_PATH,
            EventType::Delete,
            json!(null),
        ))
        .await;
        assert_eq!(hub.lock().state.vanguard, None);
    }
}
