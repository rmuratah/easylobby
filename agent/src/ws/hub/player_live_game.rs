//! Aba "Em jogo" do perfil de outro jogador (#436).
//!
//! Depois que um perfil aberto com `openPlayerProfile` chega, uma tarefa
//! pergunta à API da Riot ([`crate::riot_api`]) se o jogador está em
//! partida. Se está, publica a partida com o que a API manda (equipes,
//! campeões, feitiços, runas e Riot ID) e depois, uma tarefa por jogador,
//! completa o elo e as partidas recentes pela LCU, como na partida ao vivo
//! do próprio jogador.
//!
//! Sem configuração da API, fora de partida ou com a API recusando, a
//! partida fica `null` e a UI não mostra a aba. Um resultado de um perfil
//! que já não é o aberto é descartado.

use serde_json::Value;

use super::{Hub, Shared};
use crate::live_game::{self, LivePlayerStats, PlayerLiveGame, RankedQueue};
use crate::player_profile;
use crate::riot_api::{self, RiotApi, RiotApiConfig};
use crate::ws::protocol::ServerMessage;

/// Publica a partida do perfil, se mudou.
fn write_player_live_game(
    shared: &mut Shared,
    game: Option<PlayerLiveGame>,
) -> Option<ServerMessage> {
    if shared.state.player_live_game == game {
        return None;
    }
    shared.state.player_live_game = game.clone();
    Some(ServerMessage::PlayerLiveGame {
        player_live_game: game,
    })
}

/// Se `puuid` ainda é o perfil aberto.
fn profile_open(shared: &Shared, puuid: &str) -> bool {
    shared.player_profile_request.as_deref() == Some(puuid)
}

impl Hub {
    pub fn set_player_live_game(&self, game: Option<PlayerLiveGame>) {
        self.update(|shared| write_player_live_game(shared, game));
    }

    /// Troca a configuração da API da Riot (testes, ou o app depois de
    /// mudar a chave). `None` desliga a aba.
    pub fn set_riot_api(&self, api: Option<RiotApi>) {
        self.lock().riot_api = Some(api);
    }

    /// O cliente da API, lido da configuração na primeira vez.
    fn riot_api(&self) -> Option<RiotApi> {
        let mut shared = self.lock();
        shared
            .riot_api
            .get_or_insert_with(|| RiotApiConfig::load().and_then(|c| RiotApi::new(c).ok()))
            .clone()
    }

    /// Esquece a partida de um perfil que não é `puuid`.
    pub(super) fn forget_player_live_game(&self, puuid: &str) {
        self.update(|shared| {
            if shared
                .state
                .player_live_game
                .as_ref()
                .is_some_and(|g| g.puuid == puuid)
            {
                return None;
            }
            write_player_live_game(shared, None)
        });
    }

    /// Busca a partida de `puuid` numa tarefa separada.
    pub(super) fn spawn_player_live_game(&self, session: u64, puuid: String) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let hub = self.clone();
        runtime.spawn(async move { hub.load_player_live_game(session, puuid).await });
    }

    async fn load_player_live_game(&self, session: u64, puuid: String) {
        let Some(api) = self.riot_api() else {
            return;
        };
        let Some((client, _, me)) = self.client_and_session() else {
            return;
        };
        let platform = match client.get::<Value>(riot_api::REGION_LOCALE_PATH).await {
            Ok(body) => body
                .get("region")
                .and_then(Value::as_str)
                .and_then(riot_api::platform_from_region),
            Err(_) => None,
        };
        let Some(platform) = platform else {
            eprintln!("easylobby: região do client desconhecida; aba Em jogo desligada");
            return;
        };
        let game = match api.active_game(platform, &puuid).await {
            Ok(Some(body)) => live_game::from_spectator(&body, me.as_deref()),
            Ok(None) => None,
            Err(error) => {
                eprintln!("easylobby: partida em andamento de outro jogador: {error}");
                None
            }
        };
        let Some(mut game) = game else {
            self.update_in_session(session, |shared| {
                if !profile_open(shared, &puuid) {
                    return None;
                }
                write_player_live_game(shared, None)
            });
            return;
        };
        let game_id = game.game_id;
        let queue = game.ranked_queue;
        let players: Vec<(String, u32, bool)> = game
            .players()
            .filter(|p| !p.hidden && !p.bot && player_profile::valid_puuid(&p.puuid))
            .map(|p| (p.puuid.clone(), p.champion_id, p.game_name.is_empty()))
            .collect();
        let published = self.update_in_session(session, |shared| {
            if !profile_open(shared, &puuid) {
                return None;
            }
            if let Some(q) = shared
                .state
                .queues
                .iter()
                .find(|q| i64::from(q.queue_id) == i64::from(game.queue_id))
            {
                game.queue_name.clone_from(&q.name);
            }
            write_player_live_game(
                shared,
                Some(PlayerLiveGame {
                    puuid: puuid.clone(),
                    game,
                }),
            )
        });
        if !published {
            return;
        }
        for (player, champion_id, needs_name) in players {
            self.spawn_player_live_game_stats(
                session,
                puuid.clone(),
                game_id,
                player,
                champion_id,
                needs_name,
                queue,
            );
        }
    }

    /// Completa um jogador da partida do perfil pela LCU.
    #[allow(clippy::too_many_arguments)]
    fn spawn_player_live_game_stats(
        &self,
        session: u64,
        profile: String,
        game_id: u64,
        player: String,
        champion_id: u32,
        needs_name: bool,
        queue: RankedQueue,
    ) {
        let Some(client) = self.client() else {
            return;
        };
        let hub = self.clone();
        tokio::spawn(async move {
            let stats: LivePlayerStats =
                live_game::player_stats(&client, &player, champion_id, queue).await;
            let name = if needs_name {
                live_game::riot_id(&client, &player).await.ok().flatten()
            } else {
                None
            };
            hub.update_in_session(session, |shared| {
                let mut current = shared.state.player_live_game.clone()?;
                if current.puuid != profile || current.game.game_id != game_id {
                    return None;
                }
                for p in current.game.players_mut().filter(|p| p.puuid == player) {
                    p.stats = Some(stats.clone());
                    if let Some((name, tag)) = &name {
                        p.game_name.clone_from(name);
                        p.tag_line.clone_from(tag);
                    }
                }
                current.game.refresh_averages();
                write_player_live_game(shared, Some(current))
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_game::tests::spectator_json;

    fn game() -> PlayerLiveGame {
        PlayerLiveGame {
            puuid: "fake-puuid-0002".into(),
            game: live_game::from_spectator(&spectator_json(), None).unwrap(),
        }
    }

    #[tokio::test]
    async fn outro_perfil_esquece_a_partida() {
        let hub = Hub::new();
        hub.set_player_live_game(Some(game()));
        hub.forget_player_live_game("fake-puuid-0002");
        assert!(
            hub.state().player_live_game.is_some(),
            "o mesmo perfil fica"
        );
        hub.forget_player_live_game("fake-puuid-0003");
        assert_eq!(hub.state().player_live_game, None);
    }

    #[tokio::test]
    async fn queda_limpa_a_partida() {
        let hub = Hub::new();
        hub.set_player_live_game(Some(game()));
        hub.apply(crate::connection::ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(hub.state().player_live_game, None);
    }

    #[tokio::test]
    async fn sem_api_nao_busca() {
        let hub = Hub::new();
        hub.set_riot_api(None);
        hub.load_player_live_game(0, "fake-puuid-0002".into()).await;
        assert_eq!(hub.state().player_live_game, None);
    }
}
