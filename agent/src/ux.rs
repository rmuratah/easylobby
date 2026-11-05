//! Fechar e reabrir a UI oficial do LoL (#12).
//!
//! O LoL tem duas partes: o `LeagueClient`, que mantém a LCU API de pé, e o
//! `LeagueClientUx`, a interface em Chromium que gasta a memória. O
//! [`kill_ux`] fecha só a interface e o client continua rodando, então o
//! easylobby segue falando com a LCU. O [`launch_ux`] reabre a interface (o
//! botão de emergência).
//!
//! A [`LcuConnection`](crate::connection::LcuConnection) chama o [`kill_ux`]
//! sozinha a cada conexão, inclusive quando o LoL reabre, a menos que
//! `ReconnectConfig::kill_ux` seja `false`.

use std::sync::Arc;

use reqwest::Method;

use crate::rest::{LcuClient, LcuError};

/// Fecha a UI oficial e mantém o client rodando.
pub const KILL_UX: &str = "/riotclient/kill-ux";

/// Reabre a UI oficial.
pub const LAUNCH_UX: &str = "/riotclient/launch-ux";

/// Fecha a UI oficial (`POST /riotclient/kill-ux`). A LCU continua
/// respondendo. Chamar com a UI já fechada não faz mal.
pub async fn kill_ux(client: &LcuClient) -> Result<(), LcuError> {
    client.send::<()>(Method::POST, KILL_UX, None).await
}

/// Reabre a UI oficial (`POST /riotclient/launch-ux`).
pub async fn launch_ux(client: &LcuClient) -> Result<(), LcuError> {
    client.send::<()>(Method::POST, LAUNCH_UX, None).await
}

/// O que aconteceu com a UI oficial quando a conexão abriu.
#[derive(Debug, Clone)]
pub enum KillUx {
    /// O kill-ux automático está desligado: a UI oficial ficou como estava.
    Disabled,
    /// A LCU aceitou o `kill-ux`.
    Killed,
    /// O `kill-ux` falhou. A conexão continua valendo; a UI oficial pode ter
    /// ficado aberta.
    Failed(Arc<LcuError>),
}

impl KillUx {
    /// Se a UI oficial foi fechada.
    pub fn is_killed(&self) -> bool {
        matches!(self, KillUx::Killed)
    }
}
