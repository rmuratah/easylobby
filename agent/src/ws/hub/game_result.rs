//! Resultado da partida no `/ws` (#179).
//!
//! Veja [`crate::end_of_game`]. O bloco de estatísticas vem pelo WAMP e, para
//! não perder um que chegou antes da conexão ou um evento perdido, é relido
//! pelo REST ao conectar no pós-jogo e a cada fase do pós-jogo, junto com a
//! fila da sessão do gameflow.
//!
//! A mudança de PDL compara os ranks guardados quando a partida começou com
//! os ranks atuais, e é refeita quando os ranks novos chegam (alguns
//! segundos depois do bloco, na gravação). Quem conecta já no pós-jogo não
//! tem os ranks de antes e fica sem a mudança de PDL.

use super::{honor::post_game, live_game::in_game, Hub, Shared};
use crate::end_of_game::{self, GameResult};
use crate::gameflow::GameflowPhase;
use crate::ranked::RankedStats;
use crate::ws::protocol::ServerMessage;

/// O que o agente já sabe do resultado da partida.
#[derive(Debug, Default)]
pub(super) struct GameResultSync {
    /// O bloco lido, sem a fila e a mudança de PDL.
    block: Option<GameResult>,
    /// `(queueId, nome)` da sessão do gameflow.
    queue: Option<(u32, String)>,
    /// Ranks de quando a partida começou.
    ranked_before: Option<RankedStats>,
}

impl Hub {
    /// Troca o bloco de estatísticas (`None` apaga o resultado).
    pub fn set_game_result_block(&self, block: Option<GameResult>) {
        self.update(|shared| {
            shared.game_result.block = block;
            write_game_result(shared)
        });
    }

    /// A fase mudou de `before` para `phase`: guarda os ranks quando a
    /// partida começa, relê o resultado no pós-jogo e o apaga fora dele.
    pub(super) fn follow_game_result(
        &self,
        before: Option<GameflowPhase>,
        phase: Option<GameflowPhase>,
    ) {
        if before == phase {
            return;
        }
        if in_game(phase) && !in_game(before) {
            self.update(|shared| {
                shared.game_result = GameResultSync {
                    ranked_before: shared.state.ranked.clone(),
                    ..GameResultSync::default()
                };
                write_game_result(shared)
            });
        } else if post_game(phase) {
            self.spawn_refresh_game_result();
        } else if !in_game(phase) {
            self.update(|shared| {
                shared.game_result = GameResultSync::default();
                write_game_result(shared)
            });
        }
    }

    /// Esquece o resultado e os ranks de antes (o LoL fechou).
    pub(super) fn reset_game_result(&self) {
        self.update(|shared| {
            shared.game_result = GameResultSync::default();
            write_game_result(shared)
        });
    }

    /// Refaz o resultado com os ranks atuais, que podem ter chegado depois
    /// do bloco.
    pub(super) fn refresh_ranked_change(&self) {
        self.update(write_game_result);
    }

    /// Relê o bloco e a fila pelo REST, se ainda estiver no pós-jogo da
    /// mesma conexão quando as respostas chegarem.
    pub(super) fn spawn_refresh_game_result(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            let (block, queue) = tokio::join!(
                end_of_game::current(&client),
                end_of_game::current_queue(&client),
            );
            hub.update(|shared| {
                if shared.session != session || !post_game(shared.state.gameflow_phase) {
                    return None;
                }
                if let Ok(Some(queue)) = queue {
                    shared.game_result.queue = Some(queue);
                }
                // Um 404 ou um erro na releitura não apaga o que veio pelo
                // evento: quem apaga é o `Delete` ou a saída do pós-jogo.
                if let Ok(Some(block)) = block {
                    shared.game_result.block = Some(block);
                }
                write_game_result(shared)
            });
        });
    }
}

/// O resultado montado com o bloco, a fila e os ranks.
fn compose(shared: &Shared) -> Option<GameResult> {
    let sync = &shared.game_result;
    let mut result = sync.block.clone()?;
    if let Some((id, name)) = &sync.queue {
        result.queue_id = *id;
        result.queue_name = name.clone();
    }
    if result.result != crate::match_history::MatchResult::Remake {
        if let (Some(queue), Some(before), Some(after)) = (
            result.ranked_queue(),
            sync.ranked_before.as_ref(),
            shared.state.ranked.as_ref(),
        ) {
            result.ranked_change = end_of_game::ranked_change(queue, before, after);
        }
    }
    Some(result)
}

pub(super) fn write_game_result(shared: &mut Shared) -> Option<ServerMessage> {
    let result = compose(shared);
    if shared.state.game_result == result {
        return None;
    }
    shared.state.game_result = result.clone();
    Some(ServerMessage::GameResult {
        game_result: result,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::end_of_game::EOG_STATS_PATH;
    use crate::gameflow::GAMEFLOW_PHASE_PATH;
    use crate::match_history::MatchResult;
    use crate::ranked::{RankedEntry, RANKED_STATS_PATH};
    use crate::wamp::EventType;
    use serde_json::{json, Value};

    fn bloco() -> Value {
        json!({
            "gameId": 1124,
            "gameLength": 1551,
            "gameMode": "CLASSIC",
            "queueType": "RANKED_SOLO_5x5",
            "ranked": true,
            "teams": [
                {"teamId": 200, "isPlayerTeam": true, "isWinningTeam": true,
                 "players": [{"puuid": "fake-puuid-0001", "isLocalPlayer": true,
                              "championId": 432, "riotIdGameName": "fake-name-0001",
                              "stats": {"CHAMPIONS_KILLED": 1, "NUM_DEATHS": 2, "ASSISTS": 20}}]},
                {"teamId": 100, "isPlayerTeam": false, "isWinningTeam": false,
                 "players": [{"puuid": "fake-puuid-0091", "championId": 41}]}
            ]
        })
    }

    fn ranks(lp: u32, wins: u32) -> Value {
        json!({"queues": [{"queueType": "RANKED_SOLO_5x5", "tier": "MASTER",
                           "division": "NA", "leaguePoints": lp, "wins": wins,
                           "losses": 28}]})
    }

    async fn fase(hub: &Hub, phase: &str) {
        hub.apply(event(GAMEFLOW_PHASE_PATH, EventType::Update, json!(phase)))
            .await;
    }

    #[tokio::test]
    async fn segue_o_bloco_pelo_wamp() {
        let hub = Hub::new();
        hub.apply(event(EOG_STATS_PATH, EventType::Create, bloco()))
            .await;
        let result = hub.state().game_result.unwrap();
        assert_eq!(result.game_id, 1124);
        assert_eq!(result.queue_id, 420);
        assert_eq!(result.result, MatchResult::Win);
        assert_eq!(result.teams[0].players[0].assists, 20);

        hub.apply(event(EOG_STATS_PATH, EventType::Delete, Value::Null))
            .await;
        assert_eq!(hub.state().game_result, None);
    }

    #[tokio::test]
    async fn pdl_da_partida() {
        let hub = Hub::new();
        hub.apply(event(RANKED_STATS_PATH, EventType::Update, ranks(804, 54)))
            .await;
        fase(&hub, "InProgress").await;
        fase(&hub, "WaitingForStats").await;
        hub.apply(event(EOG_STATS_PATH, EventType::Create, bloco()))
            .await;
        // Os ranks novos ainda não chegaram.
        assert_eq!(hub.state().game_result.unwrap().ranked_change, None);

        hub.apply(event(RANKED_STATS_PATH, EventType::Update, ranks(868, 55)))
            .await;
        let change = hub.state().game_result.unwrap().ranked_change.unwrap();
        assert_eq!(change.league_points_delta, Some(64));
        assert_eq!(
            change.before,
            RankedEntry {
                tier: Some("MASTER".into()),
                division: None,
                league_points: 804,
                wins: 54,
                losses: 28,
                provisional: false,
            }
        );
    }

    #[tokio::test]
    async fn remake_nao_mostra_pdl() {
        let hub = Hub::new();
        hub.apply(event(RANKED_STATS_PATH, EventType::Update, ranks(804, 54)))
            .await;
        fase(&hub, "InProgress").await;
        let mut block = bloco();
        block["gameEndedInEarlySurrender"] = json!(true);
        hub.apply(event(EOG_STATS_PATH, EventType::Create, block))
            .await;
        hub.apply(event(RANKED_STATS_PATH, EventType::Update, ranks(800, 54)))
            .await;
        let result = hub.state().game_result.unwrap();
        assert_eq!(result.result, MatchResult::Remake);
        assert_eq!(result.ranked_change, None);
    }

    #[tokio::test]
    async fn sai_do_pos_jogo_e_apaga() {
        let hub = Hub::new();
        fase(&hub, "EndOfGame").await;
        hub.apply(event(EOG_STATS_PATH, EventType::Create, bloco()))
            .await;
        assert!(hub.state().game_result.is_some());
        // "Jogar de novo": de volta ao lobby, mesmo sem o `Delete`.
        fase(&hub, "Lobby").await;
        assert_eq!(hub.state().game_result, None);

        // Nova partida começando também apaga um resultado velho.
        hub.apply(event(EOG_STATS_PATH, EventType::Create, bloco()))
            .await;
        fase(&hub, "InProgress").await;
        assert_eq!(hub.state().game_result, None);
    }

    #[tokio::test]
    async fn bloco_sem_partida_apaga() {
        let hub = Hub::new();
        hub.apply(event(EOG_STATS_PATH, EventType::Create, bloco()))
            .await;
        hub.apply(event(
            EOG_STATS_PATH,
            EventType::Update,
            json!({"gameId": 0, "teams": []}),
        ))
        .await;
        assert_eq!(hub.state().game_result, None);
    }

    #[tokio::test]
    async fn fechar_o_lol_apaga() {
        let hub = Hub::new();
        hub.apply(event(EOG_STATS_PATH, EventType::Create, bloco()))
            .await;
        hub.apply(crate::connection::ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(hub.state().game_result, None);
    }
}
