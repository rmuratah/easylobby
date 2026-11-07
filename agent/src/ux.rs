//! Fechar e reabrir a UI oficial do LoL (#12).
//!
//! O LoL tem duas partes: o `LeagueClient`, que mantém a LCU API de pé, e o
//! `LeagueClientUx`, a interface em Chromium que gasta a memória. O
//! [`kill_ux`] fecha só a interface e o client continua rodando, então o
//! easylobby segue falando com a LCU. O [`launch_ux`] reabre a interface (o
//! botão de emergência).
//!
//! A UI oficial é fechada a cada conexão, inclusive quando o LoL reabre, a
//! menos que `ReconnectConfig::kill_ux` seja `false`. Por padrão o
//! fechamento espera `ReconnectConfig::kill_ux_delay`, porque o Vanguard fecha
//! o LoL (VAN7003) se não achar a UI enquanto inicializa.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Method;

use crate::rest::{LcuClient, LcuError};

/// Fecha a UI oficial e mantém o client rodando.
pub const KILL_UX: &str = "/riotclient/kill-ux";

/// Reabre a UI oficial.
pub const LAUNCH_UX: &str = "/riotclient/launch-ux";

/// Encerra o LoL inteiro (o `LeagueClient` e, se estiver aberta, a UI
/// oficial), do jeito que o próprio client faz ao clicar em fechar.
pub const QUIT_CLIENT: &str = "/process-control/v1/process/quit";

/// Evento em que o LoL pede para mostrar (ou esconder) a UI oficial. Com a
/// UI fechada, um pedido para mostrar a abre de novo (#188).
pub const UX_STATE_REQUEST_PATH: &str = "/riotclient/ux-state/request";

/// Fecha a UI oficial (`POST /riotclient/kill-ux`). A LCU continua
/// respondendo. Chamar com a UI já fechada não faz mal.
pub async fn kill_ux(client: &LcuClient) -> Result<(), LcuError> {
    client.send::<()>(Method::POST, KILL_UX, None).await
}

/// Reabre a UI oficial (`POST /riotclient/launch-ux`).
pub async fn launch_ux(client: &LcuClient) -> Result<(), LcuError> {
    client.send::<()>(Method::POST, LAUNCH_UX, None).await
}

/// Encerra o LoL (`POST /process-control/v1/process/quit`). A LCU responde
/// antes de o processo fechar, e a conexão cai logo depois.
pub async fn quit_client(client: &LcuClient) -> Result<(), LcuError> {
    client.send::<()>(Method::POST, QUIT_CLIENT, None).await
}

/// O que aconteceu com a UI oficial quando a conexão abriu.
#[derive(Debug, Clone)]
pub enum KillUx {
    /// O kill-ux automático está desligado: a UI oficial ficou como estava.
    Disabled,
    /// A LCU aceitou o `kill-ux`.
    Killed,
    /// A UI oficial ainda está aberta e deve ser fechada depois de `delay`,
    /// por quem dirige a conexão (o [`Hub`](crate::ws::Hub)). Fechar logo que
    /// o LoL abre dá VAN7003.
    Scheduled { delay: Duration },
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
