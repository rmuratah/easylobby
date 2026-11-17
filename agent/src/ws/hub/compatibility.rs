//! Modo de compatibilidade (#36): a checagem dos endpoints ao conectar e as
//! funções que ela desliga. Veja [`crate::health`].
//!
//! A checagem roda numa tarefa separada logo depois do login (ou da
//! conexão, se o jogador já estava logado). Um endpoint que falha é checado
//! de novo depois de [`RECHECK_DELAY`], porque a LCU registra as rotas aos
//! poucos enquanto sobe; só o que falha duas vezes desliga a função.

use std::time::Duration;

use super::Hub;
use crate::health::{self, Feature, Probe, UnavailableFeature, PROBES};
use crate::ws::protocol::ServerMessage;

/// Espera antes de checar de novo os endpoints que falharam.
const RECHECK_DELAY: Duration = Duration::from_secs(3);

impl Hub {
    pub fn set_unavailable_features(&self, unavailable: Vec<UnavailableFeature>) {
        self.update(|shared| {
            if shared.state.unavailable_features == unavailable {
                return None;
            }
            shared.state.unavailable_features = unavailable.clone();
            Some(ServerMessage::UnavailableFeatures {
                unavailable_features: unavailable,
            })
        });
    }

    /// Liga ou desliga a checagem dos endpoints (ligada por padrão). Com ela
    /// desligada nenhuma função é desligada.
    pub fn set_check_health(&self, enabled: bool) {
        self.lock().check_health = enabled;
    }

    /// Se a checagem desligou `feature`.
    pub fn feature_unavailable(&self, feature: Feature) -> Option<UnavailableFeature> {
        self.lock()
            .state
            .unavailable_features
            .iter()
            .find(|u| u.feature == feature)
            .cloned()
    }

    /// Checa os endpoints de [`PROBES`] numa tarefa separada e publica as
    /// funções desligadas. O resultado de uma conexão que já caiu é
    /// descartado.
    pub(super) fn spawn_check_health(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (client, session) = {
            let shared = self.lock();
            match (&shared.client, shared.check_health) {
                (Some(client), true) => (client.clone(), shared.session),
                _ => return,
            }
        };
        let hub = self.clone();
        runtime.spawn(async move {
            let mut unavailable = health::check(&client, PROBES).await;
            if !unavailable.is_empty() {
                tokio::time::sleep(RECHECK_DELAY).await;
                let failed: Vec<Probe> = PROBES
                    .iter()
                    .filter(|p| unavailable.iter().any(|u| u.feature == p.feature))
                    .copied()
                    .collect();
                unavailable = health::check(&client, &failed).await;
            }
            if hub.lock().session != session {
                return;
            }
            for u in &unavailable {
                tracing::warn!(
                    "modo de compatibilidade: {:?} desligada; {} falhou: {}",
                    u.feature,
                    u.path,
                    u.detail
                );
            }
            hub.set_unavailable_features(unavailable);
        });
    }
}
