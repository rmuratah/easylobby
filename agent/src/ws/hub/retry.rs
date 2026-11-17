//! Releitura do que ficou sem resposta ao conectar (#580).
//!
//! A releitura do `Connected` lê o invocador, a fase, o lobby, a busca, os
//! convites, as páginas de runas e as filas uma vez só. Com o LoL ainda
//! subindo, ou esperando os servidores da Riot, a LCU às vezes não responde
//! a tempo. Sem nova leitura, o estado ficava vazio até um evento da LCU
//! corrigir, e num lobby parado o evento nunca vinha: a UI ficava sem fase,
//! sem lobby e sem filas, e o jogador não conseguia iniciar a fila.
//!
//! Só entra aqui a leitura que ficou sem resposta (tempo esgotado ou conexão
//! caída). Um status de erro da LCU, como o `404` antes do login, é resposta
//! e fica como estava.

use std::time::Duration;

use super::{Hub, Resource};
use crate::gameflow::{self, GameflowPhase};
use crate::rest::LcuError;

/// Espera antes de cada nova leitura. Somadas, pouco mais de um minuto e
/// meio; depois disso os eventos da LCU continuam corrigindo o estado.
const DELAYS: [Duration; 6] = [
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(15),
    Duration::from_secs(30),
    Duration::from_secs(30),
];

/// Se a leitura ficou sem resposta da LCU e vale tentar de novo.
pub(super) fn no_answer<T>(result: &Result<T, LcuError>) -> bool {
    matches!(result, Err(error) if error.is_transport())
}

/// Leituras do `Connected` que ficaram sem resposta.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Missing {
    pub summoner: bool,
    pub phase: bool,
    pub lobby: bool,
    pub search: bool,
    pub invitations: bool,
    pub rune_pages: bool,
    pub queues: bool,
}

impl Missing {
    fn any(&self) -> bool {
        *self != Missing::default()
    }

    /// Nomes para o log.
    fn names(&self) -> Vec<&'static str> {
        [
            (self.summoner, "invocador"),
            (self.phase, "fase"),
            (self.lobby, "lobby"),
            (self.search, "busca"),
            (self.invitations, "convites"),
            (self.rune_pages, "runas"),
            (self.queues, "filas"),
        ]
        .into_iter()
        .filter_map(|(missing, name)| missing.then_some(name))
        .collect()
    }
}

impl Hub {
    /// Relê em segundo plano, com espera crescente, o que ficou sem resposta
    /// na releitura do `Connected` da sessão `session`. Para quando tudo
    /// respondeu, a conexão mudou ou as tentativas acabaram.
    pub(super) fn spawn_retry_connect_reads(&self, session: u64, missing: Missing) {
        if !missing.any() {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        tracing::warn!(
            "o LoL não respondeu a tempo ao conectar ({}); vou ler de novo",
            missing.names().join(", ")
        );
        let hub = self.clone();
        runtime.spawn(async move {
            let mut missing = missing;
            for delay in DELAYS {
                tokio::time::sleep(delay).await;
                if hub.lock().session != session {
                    return;
                }
                missing = hub.retry_reads(missing).await;
                if !missing.any() {
                    tracing::info!("o LoL respondeu a releitura da conexão");
                    return;
                }
            }
            tracing::warn!(
                "o LoL continua sem responder ({}); sigo pelos eventos",
                missing.names().join(", ")
            );
        });
    }

    /// Uma rodada de leituras. Devolve o que continua sem resposta.
    ///
    /// Cada leitura entra pelo carimbo dela (#245): se um evento ou um
    /// comando mudou o recurso enquanto a LCU respondia, o estado publicado
    /// é mais novo e a leitura é descartada, e o recurso conta como lido.
    async fn retry_reads(&self, missing: Missing) -> Missing {
        let mut still = Missing::default();

        if missing.summoner {
            if let Some((client, stamp)) = self.begin(Resource::Summoner) {
                match crate::summoner::current_summoner(&client).await {
                    Ok(summoner) => {
                        let logged_in = self.lock().state.summoner.is_none();
                        if self.set_summoner_if(stamp, Some(summoner)) && logged_in {
                            self.after_login();
                        }
                    }
                    Err(error) => still.summoner = error.is_transport(),
                }
            }
        }

        if missing.phase {
            if let Some((client, stamp)) = self.begin(Resource::GameflowPhase) {
                match gameflow::current_phase(&client).await {
                    Ok(phase) => self.restore_phase(stamp, phase),
                    Err(error) => still.phase = error.is_transport(),
                }
            }
        }

        if missing.queues {
            if let Some((client, stamp)) = self.begin(Resource::Queues) {
                match crate::queues::available(&client).await {
                    Ok(queues) => {
                        self.set_queues_if(stamp, queues.unwrap_or_default());
                    }
                    Err(error) => still.queues = error.is_transport(),
                }
            }
        }

        if missing.lobby {
            if let Some((client, stamp)) = self.begin(Resource::Lobby) {
                match crate::lobby::current_lobby(&client).await {
                    Ok(lobby) => {
                        if self.set_lobby_if(stamp, lobby) {
                            // Como no evento do lobby: a sala do grupo e os
                            // bots de uma personalizada.
                            self.spawn_refresh_available_bots();
                            self.ensure_lobby_chat().await;
                        }
                    }
                    Err(error) => still.lobby = error.is_transport(),
                }
            }
        }

        if missing.search {
            if let Some((client, stamp)) = self.begin(Resource::Matchmaking) {
                match crate::matchmaking::current_search(&client).await {
                    Ok(search) => {
                        self.set_matchmaking_if(stamp, search);
                    }
                    Err(error) => still.search = error.is_transport(),
                }
            }
        }

        if missing.invitations {
            if let Some((client, stamp)) = self.begin(Resource::Invitations) {
                match crate::invitations::received_invitations(&client).await {
                    Ok(invitations) => {
                        self.set_received_invitations_if(stamp, invitations);
                    }
                    Err(error) => still.invitations = error.is_transport(),
                }
            }
        }

        if missing.rune_pages {
            if let Some((client, stamp)) = self.begin(Resource::RunePages) {
                match crate::runes::current_pages(&client).await {
                    Ok(pages) => {
                        self.set_rune_pages_if(stamp, Some(pages));
                    }
                    Err(error) => still.rune_pages = error.is_transport(),
                }
            }
        }

        still
    }

    /// Aplica a fase relida e o que muda junto com ela, como o evento.
    fn restore_phase(&self, stamp: super::Stamp, phase: GameflowPhase) {
        let before = self.lock().state.gameflow_phase;
        if self.set_gameflow_phase_if(stamp, Some(phase)) {
            self.after_phase(before, phase);
        }
    }
}
