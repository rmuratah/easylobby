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
use crate::ws::protocol::ServerMessage;

/// Espera antes das primeiras leituras. Depois delas, uma a cada
/// [`STEADY_DELAY`] enquanto a conexão durar: um LoL esperando os servidores
/// da Riot pode levar minutos para voltar, e o app não pode ficar vazio até
/// lá.
const DELAYS: [Duration; 4] = [
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(15),
];

/// Espera entre as leituras depois das primeiras.
const STEADY_DELAY: Duration = Duration::from_secs(30);

/// Espera antes da leitura `attempt` (a partir de 0).
fn delay(attempt: usize) -> Duration {
    DELAYS.get(attempt).copied().unwrap_or(STEADY_DELAY)
}

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
    /// respondeu ou a conexão mudou (o LoL fechou ou reiniciou).
    pub(super) fn spawn_retry_connect_reads(&self, session: u64, missing: Missing) {
        self.set_lcu_slow(missing.any());
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
            for attempt in 0.. {
                tokio::time::sleep(delay(attempt)).await;
                if hub.lock().session != session {
                    return;
                }
                missing = hub.retry_reads(missing).await;
                if !missing.any() {
                    tracing::info!("o LoL respondeu a releitura da conexão");
                    hub.set_lcu_slow_in(session, false);
                    return;
                }
                // Uma linha no log quando passa para o ritmo lento, não uma
                // a cada tentativa.
                if attempt + 1 == DELAYS.len() {
                    tracing::warn!(
                        "o LoL continua sem responder ({}); tento de novo a cada {} s",
                        missing.names().join(", "),
                        STEADY_DELAY.as_secs()
                    );
                }
            }
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

    /// Publica se o LoL está demorando para responder (#580).
    pub(super) fn set_lcu_slow(&self, slow: bool) {
        self.update(|shared| write_lcu_slow(shared, slow));
    }

    /// Como [`Hub::set_lcu_slow`], só se a conexão ainda é `session`.
    fn set_lcu_slow_in(&self, session: u64, slow: bool) {
        self.update(|shared| {
            if shared.session != session {
                return None;
            }
            write_lcu_slow(shared, slow)
        });
    }

    /// Aplica a fase relida e o que muda junto com ela, como o evento.
    fn restore_phase(&self, stamp: super::Stamp, phase: GameflowPhase) {
        let before = self.lock().state.gameflow_phase;
        if self.set_gameflow_phase_if(stamp, Some(phase)) {
            self.after_phase(before, phase);
        }
    }
}

fn write_lcu_slow(shared: &mut super::Shared, slow: bool) -> Option<ServerMessage> {
    if shared.state.lcu_slow == slow {
        return None;
    }
    shared.state.lcu_slow = slow;
    Some(ServerMessage::LcuSlow { slow })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn espera_cresce_e_depois_fica_constante() {
        assert_eq!(delay(0), Duration::from_secs(2));
        assert_eq!(delay(3), Duration::from_secs(15));
        assert_eq!(delay(4), STEADY_DELAY);
        assert_eq!(delay(1_000), STEADY_DELAY);
    }
}
