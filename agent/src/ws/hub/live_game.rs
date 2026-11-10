//! Partida ao vivo (#268): monta e mantém a [`LiveGame`] enquanto a partida
//! está em andamento.
//!
//! Quando a fase vira `GameStart`, `InProgress` ou `Reconnect` (ou o agente
//! conecta numa delas), uma tarefa:
//!
//! 1. lê a sessão do gameflow até ela trazer as equipes;
//! 2. busca, uma tarefa por jogador, o Riot ID, o elo e as partidas
//!    recentes (bots e anônimos ficam de fora);
//! 3. lê a Live Client Data API do jogo a cada poucos segundos até as runas
//!    chegarem, e depois de vez em quando, só para acertar o relógio.
//!
//! Cada resultado entra numa cache por partida e a partida é publicada de
//! novo, montada a partir dela. Ao sair da partida (ou com o LoL fechado), a
//! cache é esquecida e a partida vira `null`; uma tarefa velha percebe pela
//! geração e para.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::{Hub, Shared};
use crate::champions::ChampionCatalog;
use crate::gameflow::GameflowPhase;
use crate::live_client::{LiveClient, LiveClientData};
use crate::live_game::{self, LiveGame, LivePlayerStats, LiveRunes};
use crate::player_profile;
use crate::ws::protocol::ServerMessage;

/// Intervalo entre as leituras enquanto falta a sessão ou as runas.
const FAST_POLL: Duration = Duration::from_secs(3);
/// Intervalo depois que tudo chegou: só acerta o relógio.
const SLOW_POLL: Duration = Duration::from_secs(30);
/// Diferença, em segundos, entre o relógio do jogo e a conta da UI que faz
/// o agente mandar o tempo de novo (pausa, reconexão).
const CLOCK_DRIFT_SECS: u64 = 3;

/// Se a fase é de partida em andamento.
pub(super) fn in_game(phase: Option<GameflowPhase>) -> bool {
    matches!(
        phase,
        Some(GameflowPhase::GameStart | GameflowPhase::InProgress | GameflowPhase::Reconnect)
    )
}

/// O que o agente já sabe da partida ao vivo.
#[derive(Debug, Default)]
pub(super) struct LiveGameSync {
    /// Muda a cada partida começada ou encerrada: a tarefa de uma partida
    /// velha para.
    generation: u64,
    /// Uma tarefa está acompanhando a partida.
    running: bool,
    /// A partida como a sessão do gameflow trouxe.
    base: Option<LiveGame>,
    /// Riot ID e números de cada `puuid`.
    names: HashMap<String, (String, String)>,
    stats: HashMap<String, LivePlayerStats>,
    /// Última leitura da Live Client Data API.
    live: Option<LiveClientData>,
    /// Tempo publicado e quando foi lido.
    clock: Option<(u32, Instant)>,
}

/// A partida da cache, com nomes, números, runas e tempo.
fn compose(sync: &LiveGameSync, catalog: Option<&ChampionCatalog>) -> Option<LiveGame> {
    let mut game = sync.base.clone()?;
    for player in game.players_mut() {
        if let Some((name, tag)) = sync.names.get(&player.puuid) {
            player.game_name.clone_from(name);
            player.tag_line.clone_from(tag);
        }
        player.stats = sync.stats.get(&player.puuid).cloned();
    }
    if let Some(live) = &sync.live {
        apply_live_client(&mut game, live, catalog);
    }
    game.game_time_secs = sync.clock.map(|(secs, _)| secs);
    game.refresh_averages();
    Some(game)
}

/// Põe as runas da Live Client Data API em cada jogador: pela equipe e pelo
/// campeão (o `alias` do catálogo) e, sem catálogo, pelo Riot ID. Quem
/// ficou sem nome e não é anônimo nem bot ganha o Riot ID do jogo.
fn apply_live_client(
    game: &mut LiveGame,
    live: &LiveClientData,
    catalog: Option<&ChampionCatalog>,
) {
    let mut used = vec![false; live.players.len()];
    for team in &mut game.teams {
        for player in &mut team.players {
            let alias = catalog
                .and_then(|c| c.get(player.champion_id))
                .map(|c| c.alias.as_str());
            let found = live.players.iter().enumerate().find(|(i, l)| {
                if used[*i] || l.team_id != team.team_id {
                    return false;
                }
                let by_champion = alias.is_some_and(|a| a.eq_ignore_ascii_case(&l.champion_alias));
                let by_name = !player.game_name.is_empty()
                    && player.game_name.eq_ignore_ascii_case(&l.game_name)
                    && player.tag_line.eq_ignore_ascii_case(&l.tag_line);
                by_champion || by_name
            });
            let Some((i, l)) = found else {
                continue;
            };
            used[i] = true;
            if l.keystone_id != 0 || l.primary_style_id != 0 {
                player.runes = Some(LiveRunes {
                    keystone_id: l.keystone_id,
                    primary_style_id: l.primary_style_id,
                    sub_style_id: l.sub_style_id,
                });
            }
            if player.game_name.is_empty() && !player.hidden && !player.bot {
                player.game_name.clone_from(&l.game_name);
                player.tag_line.clone_from(&l.tag_line);
            }
        }
    }
}

/// Publica a partida montada da cache, se mudou.
fn write_live_game(shared: &mut Shared) -> Option<ServerMessage> {
    let game = compose(&shared.live_game, shared.state.champion_catalog.as_ref());
    if shared.state.live_game == game {
        return None;
    }
    shared.state.live_game = game.clone();
    Some(ServerMessage::LiveGame { live_game: game })
}

impl Hub {
    /// Acompanha a troca de fase: começa a partida ao vivo ao entrar numa
    /// fase de partida e a esquece ao sair.
    pub(super) fn follow_live_game(&self, phase: Option<GameflowPhase>) {
        if in_game(phase) {
            self.start_live_game();
        } else {
            self.stop_live_game();
        }
    }

    /// Começa a acompanhar a partida, se ainda não está.
    fn start_live_game(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (generation, session) = {
            let mut shared = self.lock();
            if shared.live_game.running || shared.client.is_none() {
                return;
            }
            let sync = &mut shared.live_game;
            sync.generation += 1;
            sync.running = true;
            (sync.generation, shared.session)
        };
        let hub = self.clone();
        runtime.spawn(async move { hub.watch_live_game(generation, session).await });
    }

    /// Esquece a partida e para a tarefa dela.
    pub(super) fn stop_live_game(&self) {
        self.update(|shared| {
            let generation = shared.live_game.generation + 1;
            shared.live_game = LiveGameSync {
                generation,
                ..LiveGameSync::default()
            };
            write_live_game(shared)
        });
    }

    /// Se a tarefa da `generation` ainda vale.
    fn live_game_current(&self, generation: u64, session: u64) -> bool {
        let shared = self.lock();
        shared.session == session && shared.live_game.generation == generation
    }

    /// Muda a cache com `f` e publica, se a partida ainda é a mesma.
    fn update_live_game(&self, generation: u64, session: u64, f: impl FnOnce(&mut LiveGameSync)) {
        self.update(|shared| {
            if shared.session != session || shared.live_game.generation != generation {
                return None;
            }
            f(&mut shared.live_game);
            write_live_game(shared)
        });
    }

    async fn watch_live_game(&self, generation: u64, session: u64) {
        let live_client = LiveClient::new().ok();
        loop {
            if !self.live_game_current(generation, session) {
                return;
            }
            let (client, me, has_base, has_runes) = {
                let shared = self.lock();
                let Some(client) = shared.client.clone() else {
                    return;
                };
                let me = shared.state.summoner.as_ref().map(|s| s.puuid.clone());
                let sync = &shared.live_game;
                let has_runes = shared
                    .state
                    .live_game
                    .as_ref()
                    .is_some_and(|g| g.players().any(|p| p.runes.is_some()));
                (client, me, sync.base.is_some(), has_runes)
            };
            if !has_base {
                if let Ok(Some(game)) = live_game::current(&client, me.as_deref()).await {
                    let players: Vec<(String, u32)> = game
                        .players()
                        .filter(|p| !p.hidden && !p.bot && player_profile::valid_puuid(&p.puuid))
                        .map(|p| (p.puuid.clone(), p.champion_id))
                        .collect();
                    let queue = game.ranked_queue;
                    self.update_live_game(generation, session, |sync| sync.base = Some(game));
                    for (puuid, champion_id) in players {
                        self.spawn_live_player(generation, session, puuid, champion_id, queue);
                    }
                }
            }
            let mut polled = false;
            if let Some(live_client) = &live_client {
                if let Some(data) = live_client.game_data().await {
                    polled = true;
                    self.update_live_game(generation, session, |sync| {
                        let now = Instant::now();
                        let drifted = sync.clock.is_none_or(|(secs, at)| {
                            let expected = u64::from(secs) + now.duration_since(at).as_secs();
                            expected.abs_diff(u64::from(data.game_time_secs)) > CLOCK_DRIFT_SECS
                        });
                        if drifted {
                            sync.clock = Some((data.game_time_secs, now));
                        }
                        sync.live = Some(data);
                    });
                }
            }
            let done = has_base && has_runes && polled;
            tokio::time::sleep(if done { SLOW_POLL } else { FAST_POLL }).await;
        }
    }

    /// Busca o Riot ID e os números de um jogador numa tarefa separada.
    fn spawn_live_player(
        &self,
        generation: u64,
        session: u64,
        puuid: String,
        champion_id: u32,
        queue: live_game::RankedQueue,
    ) {
        let Some(client) = self.client() else {
            return;
        };
        let hub = self.clone();
        tokio::spawn(async move {
            let (name, stats) = tokio::join!(
                live_game::riot_id(&client, &puuid),
                live_game::player_stats(&client, &puuid, champion_id, queue),
            );
            hub.update_live_game(generation, session, |sync| {
                if let Ok(Some(name)) = name {
                    sync.names.insert(puuid.clone(), name);
                }
                sync.stats.insert(puuid, stats);
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_client::LiveClientPlayer;
    use crate::live_game::tests::gravacao_em_jogo;
    use crate::ranked::RankedEntry;
    use serde_json::json;

    fn base() -> LiveGame {
        live_game::from_session(&gravacao_em_jogo(), Some("fake-puuid-0001")).unwrap()
    }

    fn catalog() -> ChampionCatalog {
        ChampionCatalog::from_json(&json!([
            {"id": 121, "name": "Kha'Zix", "alias": "Khazix"},
            {"id": 26, "name": "Zilean", "alias": "Zilean"}
        ]))
        .unwrap()
    }

    fn live_player(team_id: u32, alias: &str, name: &str, keystone: u32) -> LiveClientPlayer {
        LiveClientPlayer {
            team_id,
            champion_alias: alias.into(),
            game_name: name.into(),
            tag_line: "BR1".into(),
            keystone_id: keystone,
            primary_style_id: 8000,
            sub_style_id: 8100,
        }
    }

    #[test]
    fn fases_de_partida() {
        assert!(in_game(Some(GameflowPhase::InProgress)));
        assert!(in_game(Some(GameflowPhase::GameStart)));
        assert!(in_game(Some(GameflowPhase::Reconnect)));
        assert!(!in_game(Some(GameflowPhase::EndOfGame)));
        assert!(!in_game(Some(GameflowPhase::ChampSelect)));
        assert!(!in_game(None));
    }

    #[test]
    fn monta_com_nomes_numeros_runas_e_tempo() {
        let gold = RankedEntry {
            tier: Some("GOLD".into()),
            division: Some("II".into()),
            ..RankedEntry::default()
        };
        let mut sync = LiveGameSync {
            base: Some(base()),
            ..LiveGameSync::default()
        };
        sync.names
            .insert("fake-puuid-0001".into(), ("Eu".into(), "000".into()));
        sync.stats.insert(
            "fake-puuid-0001".into(),
            LivePlayerStats {
                rank: Some(gold),
                ..LivePlayerStats::default()
            },
        );
        sync.live = Some(LiveClientData {
            game_time_secs: 90,
            players: vec![
                live_player(100, "Khazix", "Eu", 8010),
                // O anônimo: as runas entram, o nome não.
                live_player(200, "Zilean", "Escondido", 8214),
            ],
        });
        sync.clock = Some((90, Instant::now()));
        let game = compose(&sync, Some(&catalog())).unwrap();
        assert_eq!(game.game_time_secs, Some(90));
        let me = game.players().find(|p| p.me).unwrap();
        assert_eq!((me.game_name.as_str(), me.tag_line.as_str()), ("Eu", "000"));
        assert_eq!(me.runes.unwrap().keystone_id, 8010);
        let hidden = game.players().find(|p| p.hidden).unwrap();
        assert_eq!(hidden.runes.unwrap().keystone_id, 8214);
        assert_eq!(hidden.game_name, "");
        assert_eq!(
            game.teams[0].average_tier.as_ref().map(|t| t.tier.as_str()),
            Some("GOLD")
        );
        assert_eq!(game.teams[1].average_tier, None);
        // Quem ainda não chegou fica sem números.
        assert_eq!(game.players().filter(|p| p.stats.is_some()).count(), 1);
    }

    #[test]
    fn sem_catalogo_acha_pelo_riot_id() {
        let mut sync = LiveGameSync {
            base: Some(base()),
            ..LiveGameSync::default()
        };
        sync.names
            .insert("fake-puuid-0001".into(), ("Eu".into(), "br1".into()));
        sync.live = Some(LiveClientData {
            game_time_secs: 0,
            players: vec![live_player(100, "Outro", "EU", 8010)],
        });
        let game = compose(&sync, None).unwrap();
        let me = game.players().find(|p| p.me).unwrap();
        assert_eq!(me.runes.unwrap().keystone_id, 8010);
        assert_eq!(game.game_time_secs, None);
    }

    #[test]
    fn sair_da_partida_limpa() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.update(|shared| {
            shared.live_game.base = Some(base());
            write_live_game(shared)
        });
        assert!(matches!(
            rx.try_recv(),
            Ok(ServerMessage::LiveGame { live_game: Some(_) })
        ));
        let generation = hub.lock().live_game.generation;
        hub.follow_live_game(Some(GameflowPhase::EndOfGame));
        assert!(matches!(
            rx.try_recv(),
            Ok(ServerMessage::LiveGame { live_game: None })
        ));
        assert_eq!(hub.state().live_game, None);
        // Uma tarefa velha não escreve mais.
        let session = hub.lock().session;
        assert!(!hub.live_game_current(generation, session));
        hub.update_live_game(generation, 0, |sync| sync.base = Some(base()));
        assert_eq!(hub.state().live_game, None);
    }
}
