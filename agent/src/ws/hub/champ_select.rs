//! Seleção de campeões: sessão, ações, trocas e feitiços de invocador.

use super::{not_connected, Hub, Resource, Shared, Stamp, REFRESH_ATTEMPTS};
use crate::bench;
use crate::champ_select::{self, ActionIntent, ChampSelect};
use crate::champions::{self, ChampSelectChampions, ChampionCatalog, PICKABLE_CHAMPIONS_PATH};
use crate::summoner_spells::{self, SummonerSpellCatalog};
use crate::swaps::{self, SwapIntent, SwapKind};
use crate::wamp::{EventType, LcuEvent};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    pub fn set_champ_select(&self, champ_select: Option<ChampSelect>) {
        self.update(|shared| write_champ_select(shared, champ_select));
        self.resolve_champ_select_chat_senders();
    }

    pub(super) fn set_champ_select_if(
        &self,
        stamp: Stamp,
        champ_select: Option<ChampSelect>,
    ) -> bool {
        let applied = self.update_if(Resource::ChampSelect, stamp, |shared| {
            write_champ_select(shared, champ_select)
        });
        if applied {
            self.resolve_champ_select_chat_senders();
        }
        applied
    }

    /// Publica os feitiços de invocador do jogo (#24).
    pub fn set_summoner_spell_catalog(&self, catalog: Option<SummonerSpellCatalog>) {
        self.update(|shared| {
            if shared.state.summoner_spell_catalog == catalog {
                return None;
            }
            shared.state.summoner_spell_catalog = catalog.clone();
            Some(ServerMessage::SummonerSpellCatalog {
                summoner_spell_catalog: catalog,
            })
        });
    }

    /// Publica os campeões do jogo (#28).
    pub fn set_champion_catalog(&self, catalog: Option<ChampionCatalog>) {
        self.update(|shared| {
            if shared.state.champion_catalog == catalog {
                return None;
            }
            shared.state.champion_catalog = catalog.clone();
            Some(ServerMessage::ChampionCatalog {
                champion_catalog: catalog,
            })
        });
    }

    /// Publica os campeões que dá para escolher e banir na seleção (#28).
    pub fn set_champ_select_champions(&self, champions: ChampSelectChampions) {
        self.update(|shared| write_champ_select_champions(shared, champions));
    }

    /// Como [`Hub::set_champ_select_champions`], mas só se a conexão ainda é
    /// a `session` da releitura.
    pub(super) fn restore_champ_select_champions(
        &self,
        session: u64,
        champions: ChampSelectChampions,
    ) {
        self.update(|shared| {
            if shared.session != session {
                return None;
            }
            write_champ_select_champions(shared, champions)
        });
    }

    /// Evento do WAMP numa das listas de campeões da seleção. O `Delete`
    /// (a seleção fechou) esvazia a lista.
    pub(super) fn apply_champ_select_champions_event(&self, event: &LcuEvent) {
        let ids = match event.event_type {
            EventType::Delete => None,
            _ => match champions::ids_from_json(&event.data) {
                Some(ids) => Some(ids),
                // JSON que não é lista: ignorado.
                None => return,
            },
        };
        self.update(|shared| {
            let mut champions = shared.state.champ_select_champions.clone();
            if event.uri == PICKABLE_CHAMPIONS_PATH {
                champions.pickable = ids;
            } else {
                champions.bannable = ids;
            }
            write_champ_select_champions(shared, champions)
        });
    }

    /// Troca os feitiços de invocador do jogador logado (comando
    /// `setSummonerSpells`, #24). Confere o pedido contra a seleção atual, e
    /// relê a sessão pelo REST e a publica antes de devolver.
    pub async fn set_summoner_spells(&self, spell1_id: u32, spell2_id: u32) -> CommandResult {
        let (client, checked) = {
            let shared = self.lock();
            let state = &shared.state;
            let champ_select = state.champ_select.as_ref();
            // A fila da seleção na lista, para as filas fora das fixas (#146).
            let game_queue =
                champ_select.and_then(|cs| state.queues.iter().find(|q| q.queue_id == cs.queue_id));
            let checked = summoner_spells::check_spells_in(
                champ_select,
                game_queue,
                state.summoner_spell_catalog.as_ref(),
                state.summoner.as_ref().map(|s| s.summoner_level),
                spell1_id,
                spell2_id,
            );
            (shared.client.clone(), checked)
        };
        let Some(client) = client else {
            return not_connected();
        };
        if let Err(invalid) = checked {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        if let Err(error) = summoner_spells::set_spells(&client, spell1_id, spell2_id).await {
            return CommandResult::error(ErrorCode::Lcu, error.to_string());
        }
        self.refresh_champ_select().await;
        CommandResult::Ok
    }

    /// Marca um campeão numa ação da seleção sem confirmar (comando
    /// `hoverChampion`, #23). Confere o pedido contra a seleção atual, e
    /// relê a sessão pelo REST e a publica antes de devolver.
    pub async fn hover_champion(&self, action_id: u32, champion_id: u32) -> CommandResult {
        self.champ_select_action(action_id, champion_id, ActionIntent::Hover)
            .await
    }

    /// Confirma o ban ou o pick (comando `confirmAction`, #23). Como
    /// [`Hub::hover_champion`].
    pub async fn confirm_action(&self, action_id: u32, champion_id: u32) -> CommandResult {
        self.champ_select_action(action_id, champion_id, ActionIntent::Confirm)
            .await
    }

    pub(super) async fn champ_select_action(
        &self,
        action_id: u32,
        champion_id: u32,
        intent: ActionIntent,
    ) -> CommandResult {
        let (client, champ_select) = {
            let shared = self.lock();
            (shared.client.clone(), shared.state.champ_select.clone())
        };
        let Some(client) = client else {
            return not_connected();
        };
        if let Err(invalid) =
            champ_select::check_action(champ_select.as_ref(), action_id, champion_id, intent)
        {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let sent = match intent {
            ActionIntent::Hover => champ_select::hover(&client, action_id, champion_id).await,
            ActionIntent::Confirm => champ_select::confirm(&client, action_id, champion_id).await,
        };
        if let Err(error) = sent {
            return CommandResult::error(ErrorCode::Lcu, error.to_string());
        }
        self.refresh_champ_select().await;
        CommandResult::Ok
    }

    /// Pede, aceita, recusa ou cancela uma troca na seleção (comandos
    /// `requestSwap`, `acceptSwap`, `declineSwap` e `cancelSwap`, #25).
    /// Confere o pedido contra a seleção atual, e relê a sessão pelo REST e
    /// a publica antes de devolver.
    pub async fn champ_select_swap(
        &self,
        kind: SwapKind,
        swap_id: u64,
        intent: SwapIntent,
    ) -> CommandResult {
        let (client, champ_select) = {
            let shared = self.lock();
            (shared.client.clone(), shared.state.champ_select.clone())
        };
        let Some(client) = client else {
            return not_connected();
        };
        if let Err(invalid) = swaps::check_swap(champ_select.as_ref(), kind, swap_id, intent) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        if let Err(error) = swaps::send(&client, kind, swap_id, intent).await {
            return CommandResult::error(ErrorCode::Lcu, error.to_string());
        }
        self.refresh_champ_select().await;
        CommandResult::Ok
    }

    /// Rola de novo o campeão do jogador no ARAM (comando `rerollChampion`,
    /// #201). Confere o pedido contra a seleção atual, e relê a sessão pelo
    /// REST e a publica antes de devolver.
    pub async fn reroll_champion(&self) -> CommandResult {
        let (client, champ_select) = {
            let shared = self.lock();
            (shared.client.clone(), shared.state.champ_select.clone())
        };
        let Some(client) = client else {
            return not_connected();
        };
        if let Err(invalid) = bench::check_reroll(champ_select.as_ref()) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        if let Err(error) = bench::reroll(&client).await {
            return CommandResult::error(ErrorCode::Lcu, error.to_string());
        }
        self.refresh_champ_select().await;
        CommandResult::Ok
    }

    /// Troca o campeão do jogador por um do banco do ARAM (comando
    /// `swapBenchChampion`, #201). Como [`Hub::reroll_champion`].
    pub async fn swap_bench_champion(&self, champion_id: u32) -> CommandResult {
        let (client, champ_select) = {
            let shared = self.lock();
            (shared.client.clone(), shared.state.champ_select.clone())
        };
        let Some(client) = client else {
            return not_connected();
        };
        if let Err(invalid) = bench::check_bench_swap(champ_select.as_ref(), champion_id) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        if let Err(error) = bench::swap_with_bench(&client, champion_id).await {
            return CommandResult::error(ErrorCode::Lcu, error.to_string());
        }
        self.refresh_champ_select().await;
        CommandResult::Ok
    }

    /// Relê a sessão da seleção depois de um comando; como
    /// [`Hub::refresh_lobby`] (#87). Um `Delete` ou uma queda durante a
    /// espera descartam a resposta, e a seleção encerrada não volta. Sem
    /// sessão (404) ou com erro, o evento do WAMP traz o estado logo depois.
    pub(super) async fn refresh_champ_select(&self) {
        for _ in 0..REFRESH_ATTEMPTS {
            let Some((client, stamp)) = self.begin(Resource::ChampSelect) else {
                return;
            };
            let Ok(Some(session)) = champ_select::current_session(&client).await else {
                return;
            };
            if self.set_champ_select_if(stamp, Some(session)) {
                return;
            }
        }
    }
}

fn write_champ_select_champions(
    shared: &mut Shared,
    champions: ChampSelectChampions,
) -> Option<ServerMessage> {
    if shared.state.champ_select_champions == champions {
        return None;
    }
    shared.state.champ_select_champions = champions.clone();
    Some(ServerMessage::ChampSelectChampions {
        champ_select_champions: champions,
    })
}

/// Troca a seleção de campeões, contando a escrita mesmo quando nada muda
/// (#87).
pub(super) fn write_champ_select(
    shared: &mut Shared,
    champ_select: Option<ChampSelect>,
) -> Option<ServerMessage> {
    shared.champ_select_rev += 1;
    if shared.state.champ_select == champ_select {
        return None;
    }
    shared.state.champ_select = champ_select.clone();
    Some(ServerMessage::ChampSelect { champ_select })
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::champ_select::CHAMP_SELECT_SESSION_PATH;
    use crate::connection::ConnectionEvent;
    use crate::wamp::EventType;
    use serde_json::json;

    #[tokio::test]
    async fn segue_a_selecao_de_campeoes() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Create,
            champ_select("PLANNING", 0),
        ))
        .await;
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Update,
            champ_select("BAN_PICK", 86),
        ))
        .await;
        // JSON que não é sessão: ignorado.
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Update,
            json!({}),
        ))
        .await;
        // O `Delete` da LCU traz uma sessão vazia em `data`.
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Delete,
            json!({"actions": [], "myTeam": [], "timer": {"phase": ""}}),
        ))
        .await;

        let seen: Vec<Option<(&'static str, Option<u32>)>> =
            std::iter::from_fn(|| rx.try_recv().ok())
                .map(|m| match m {
                    ServerMessage::ChampSelect { champ_select } => champ_select.map(|cs| {
                        (
                            cs.timer.phase.as_str(),
                            cs.pending_action.and_then(|a| a.champion_id),
                        )
                    }),
                    other => panic!("esperava champSelect: {other:?}"),
                })
                .collect();
        assert_eq!(
            seen,
            [Some(("PLANNING", None)), Some(("BAN_PICK", Some(86))), None]
        );
        assert_eq!(hub.state().champ_select, None);
    }

    #[tokio::test]
    async fn queda_fecha_a_selecao_de_campeoes() {
        let hub = Hub::new();
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Update,
            champ_select("BAN_PICK", 0),
        ))
        .await;
        assert!(hub.state().champ_select.is_some());
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(hub.state().champ_select, None);
    }

    #[tokio::test]
    async fn segue_os_campeoes_que_da_para_escolher_e_banir() {
        use crate::champions::BANNABLE_CHAMPIONS_PATH;
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.apply(event(
            PICKABLE_CHAMPIONS_PATH,
            EventType::Create,
            json!([266, 1]),
        ))
        .await;
        hub.apply(event(
            BANNABLE_CHAMPIONS_PATH,
            EventType::Create,
            json!([1, 2, 266]),
        ))
        .await;
        // Repetido não publica; JSON que não é lista é ignorado.
        hub.apply(event(
            PICKABLE_CHAMPIONS_PATH,
            EventType::Update,
            json!([1, 266]),
        ))
        .await;
        hub.apply(event(PICKABLE_CHAMPIONS_PATH, EventType::Update, json!({})))
            .await;
        hub.apply(event(PICKABLE_CHAMPIONS_PATH, EventType::Delete, json!([])))
            .await;
        let seen: Vec<ChampSelectChampions> = std::iter::from_fn(|| rx.try_recv().ok())
            .map(|m| match m {
                ServerMessage::ChampSelectChampions {
                    champ_select_champions,
                } => champ_select_champions,
                other => panic!("esperava champSelectChampions: {other:?}"),
            })
            .collect();
        assert_eq!(
            seen,
            [
                ChampSelectChampions {
                    pickable: Some(vec![1, 266]),
                    bannable: None,
                },
                ChampSelectChampions {
                    pickable: Some(vec![1, 266]),
                    bannable: Some(vec![1, 2, 266]),
                },
                ChampSelectChampions {
                    pickable: None,
                    bannable: Some(vec![1, 2, 266]),
                },
            ]
        );
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(
            hub.state().champ_select_champions,
            ChampSelectChampions::default()
        );
    }
}
