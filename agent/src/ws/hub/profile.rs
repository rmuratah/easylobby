//! Perfil do jogador logado: ranks (#171), histórico de partidas (#172),
//! placar de uma partida (#174) e maestria (#175).
//!
//! Tudo é lido ao conectar e relido depois de cada partida (veja
//! [`ends_a_game`]); os ranks também chegam pelo WAMP. Os resultados REST só
//! entram se a conexão ainda é a mesma de quando a leitura saiu. As páginas
//! do histórico se juntam sem repetir partida, então a ordem em que uma
//! releitura e um `loadMoreMatches` terminam não importa.

use super::{not_connected, Hub, Shared};
use crate::gameflow::GameflowPhase;
use crate::mastery::{self, ChampionMastery};
use crate::match_history::{self, MatchDetails, MatchHistory, MatchSummary};
use crate::ranked::{self, RankedStats};
use crate::rest::LcuClient;
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

/// Se a troca de fase `from` -> `to` marca o fim de uma partida, quando o
/// histórico, os ranks e a maestria mudam. Relê ao entrar no pós-jogo e de
/// novo ao sair dele, porque a partida às vezes demora a entrar no
/// histórico.
pub(super) fn ends_a_game(from: Option<GameflowPhase>, to: Option<GameflowPhase>) -> bool {
    use GameflowPhase::*;
    let after_game = matches!(
        from,
        Some(InProgress | WaitingForStats | PreEndOfGame | EndOfGame)
    );
    after_game && from != to && matches!(to, Some(EndOfGame | Lobby | Idle))
}

impl Hub {
    pub fn set_ranked(&self, ranked: Option<RankedStats>) {
        self.update(|shared| write_ranked(shared, ranked));
    }

    pub fn set_champion_masteries(&self, masteries: Vec<ChampionMastery>) {
        self.update(|shared| write_masteries(shared, masteries));
    }

    pub fn set_match_history(&self, history: Option<MatchHistory>) {
        self.update(|shared| write_history(shared, history));
    }

    pub fn set_match_details(&self, details: Option<MatchDetails>) {
        self.update(|shared| write_details(shared, details));
    }

    /// Muda o estado com `f` só se a conexão ainda é a `session`.
    fn update_in_session(
        &self,
        session: u64,
        f: impl FnOnce(&mut Shared) -> Option<ServerMessage>,
    ) -> bool {
        let mut applied = false;
        self.update(|shared| {
            if shared.session != session {
                return None;
            }
            applied = true;
            f(shared)
        });
        applied
    }

    fn client_and_session(&self) -> Option<(LcuClient, u64, Option<String>)> {
        let shared = self.lock();
        let client = shared.client.clone()?;
        let puuid = shared.state.summoner.as_ref().map(|s| s.puuid.clone());
        Some((client, shared.session, puuid))
    }

    /// Relê ranks, maestria e a primeira página do histórico. Chamado ao
    /// conectar (depois do invocador, para achar o jogador nas partidas) e
    /// no fim de cada partida.
    pub(super) async fn refresh_profile(&self) {
        let Some((client, session, puuid)) = self.client_and_session() else {
            return;
        };
        let (ranked, masteries, page) = tokio::join!(
            ranked::current(&client),
            mastery::top(&client),
            match_history::page(&client, 0, puuid.as_deref()),
        );
        // Um erro (conta sem histórico, LCU ainda subindo) não apaga o que já
        // estava na tela; a próxima releitura tenta de novo.
        if let Ok(Some(ranked)) = ranked {
            self.update_in_session(session, |shared| write_ranked(shared, Some(ranked)));
        }
        if let Ok(Some(masteries)) = masteries {
            self.update_in_session(session, |shared| write_masteries(shared, masteries));
        }
        if let Ok(Some((matches, received))) = page {
            self.update_in_session(session, |shared| {
                let history = merge_first(shared.state.match_history.clone(), matches, received);
                write_history(shared, Some(history))
            });
        }
    }

    /// Relê o perfil numa tarefa separada, sem segurar quem trata os eventos.
    pub(super) fn spawn_refresh_profile(&self) {
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let hub = self.clone();
            runtime.spawn(async move { hub.refresh_profile().await });
        }
    }

    /// Carrega a próxima página do histórico (comando `loadMoreMatches`,
    /// #172). O histórico com as partidas novas chega numa mensagem
    /// `matchHistory` antes da resposta.
    pub async fn load_more_matches(&self) -> CommandResult {
        let Some((client, session, puuid)) = self.client_and_session() else {
            return not_connected();
        };
        let begin = self
            .lock()
            .state
            .match_history
            .as_ref()
            .map_or(0, |h| h.matches.len());
        match match_history::page(&client, begin, puuid.as_deref()).await {
            Ok(Some((matches, received))) => {
                self.update_in_session(session, |shared| {
                    let history = match shared.state.match_history.clone() {
                        Some(mut history) => {
                            history.append(matches, received);
                            history
                        }
                        None => MatchHistory::first_page(matches, received),
                    };
                    write_history(shared, Some(history))
                });
                CommandResult::Ok
            }
            Ok(None) => CommandResult::error(ErrorCode::Lcu, "resposta inesperada da LCU"),
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Busca o placar da partida `game_id` (comando `openMatchDetails`,
    /// #174). O placar chega numa mensagem `matchDetails` antes da resposta.
    pub async fn open_match_details(&self, game_id: u64) -> CommandResult {
        let Some((client, session, _)) = self.client_and_session() else {
            return not_connected();
        };
        match match_history::details(&client, game_id).await {
            Ok(Some(details)) => {
                self.update_in_session(session, |shared| write_details(shared, Some(details)));
                CommandResult::Ok
            }
            Ok(None) => CommandResult::error(ErrorCode::Lcu, "resposta inesperada da LCU"),
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }
}

fn merge_first(
    current: Option<MatchHistory>,
    matches: Vec<MatchSummary>,
    received: usize,
) -> MatchHistory {
    match current {
        Some(mut history) => {
            history.merge_first_page(matches, received);
            history
        }
        None => MatchHistory::first_page(matches, received),
    }
}

fn write_ranked(shared: &mut Shared, ranked: Option<RankedStats>) -> Option<ServerMessage> {
    if shared.state.ranked == ranked {
        return None;
    }
    shared.state.ranked = ranked.clone();
    Some(ServerMessage::Ranked { ranked })
}

fn write_masteries(shared: &mut Shared, masteries: Vec<ChampionMastery>) -> Option<ServerMessage> {
    if shared.state.champion_masteries == masteries {
        return None;
    }
    shared.state.champion_masteries = masteries.clone();
    Some(ServerMessage::ChampionMasteries {
        champion_masteries: masteries,
    })
}

fn write_history(shared: &mut Shared, history: Option<MatchHistory>) -> Option<ServerMessage> {
    if shared.state.match_history == history {
        return None;
    }
    shared.state.match_history = history.clone();
    Some(ServerMessage::MatchHistory {
        match_history: history,
    })
}

fn write_details(shared: &mut Shared, details: Option<MatchDetails>) -> Option<ServerMessage> {
    if shared.state.match_details == details {
        return None;
    }
    shared.state.match_details = details.clone();
    Some(ServerMessage::MatchDetails {
        match_details: details,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::event;
    use super::*;
    use crate::match_history::tests::{game, page_json};
    use crate::ranked::RANKED_STATS_PATH;
    use crate::wamp::EventType;
    use serde_json::json;

    #[test]
    fn fim_de_partida() {
        use GameflowPhase::*;
        assert!(ends_a_game(Some(InProgress), Some(EndOfGame)));
        assert!(ends_a_game(Some(PreEndOfGame), Some(EndOfGame)));
        assert!(ends_a_game(Some(EndOfGame), Some(Lobby)));
        assert!(ends_a_game(Some(EndOfGame), Some(Idle)));
        assert!(!ends_a_game(Some(EndOfGame), Some(EndOfGame)));
        assert!(!ends_a_game(Some(Lobby), Some(Matchmaking)));
        assert!(!ends_a_game(Some(ChampSelect), Some(Lobby)));
        assert!(!ends_a_game(None, Some(Idle)));
    }

    #[tokio::test]
    async fn ranks_pelo_wamp() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let body = json!({"queues": [
            {"queueType": "RANKED_SOLO_5x5", "tier": "GOLD", "division": "II", "leaguePoints": 47}
        ]});
        hub.apply(event(RANKED_STATS_PATH, EventType::Update, body.clone()))
            .await;
        hub.apply(event(RANKED_STATS_PATH, EventType::Update, body))
            .await;
        let Ok(ServerMessage::Ranked {
            ranked: Some(ranked),
        }) = rx.try_recv()
        else {
            panic!("esperava os ranks");
        };
        assert_eq!(ranked.solo_duo.tier.as_deref(), Some("GOLD"));
        assert!(rx.try_recv().is_err(), "repetido não gera mensagem");
        // `Delete` não apaga os ranks (a LCU não manda na prática).
        hub.apply(event(RANKED_STATS_PATH, EventType::Delete, json!(null)))
            .await;
        assert!(hub.state().ranked.is_some());
    }

    #[test]
    fn juntar_releitura_sem_historico() {
        let (matches, received) =
            match_history::page_from_json(&page_json(vec![game(1, 10, true, 1)]), None).unwrap();
        let history = merge_first(None, matches, received);
        assert_eq!(history.matches.len(), 1);
        assert!(!history.has_more);
    }

    #[tokio::test]
    async fn comandos_sem_lol_respondem_not_connected() {
        let hub = Hub::new();
        for result in [
            hub.load_more_matches().await,
            hub.open_match_details(1).await,
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

    #[tokio::test]
    async fn queda_limpa_o_perfil() {
        let hub = Hub::new();
        hub.set_ranked(Some(RankedStats::default()));
        hub.set_match_history(Some(MatchHistory::default()));
        hub.set_champion_masteries(vec![ChampionMastery {
            champion_id: 1,
            level: 1,
            points: 1,
        }]);
        hub.apply(crate::connection::ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        let state = hub.state();
        assert_eq!(state.ranked, None);
        assert_eq!(state.match_history, None);
        assert_eq!(state.match_details, None);
        assert!(state.champion_masteries.is_empty());
    }
}
