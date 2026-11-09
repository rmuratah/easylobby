//! Elo (#275) e Riot ID (#325) de cada jogador no card do lobby.
//!
//! O lobby da LCU não traz o elo nem o nome dos membros. A cada lobby novo o
//! agente busca `GET /lol-ranked/v1/ranked-stats/{puuid}` e
//! `GET /lol-summoner/v2/summoners/puuid/{puuid}` de quem ainda não está na
//! cache, numa tarefa separada, e publica o lobby de novo com
//! [`LobbyMember::ranked`](crate::lobby::LobbyMember::ranked) e
//! [`LobbyMember::game_name`](crate::lobby::LobbyMember::game_name)
//! preenchidos. O jogador logado usa os ranks que o agente já acompanha
//! (#171), que mudam no fim de uma ranqueada, e o próprio Riot ID.
//!
//! A cache vale enquanto o lobby existe: limpa quando ele fecha (e com o LoL
//! fechado, que fecha o lobby) e no fim de uma partida, quando os ranks
//! mudam.

use std::collections::{HashMap, HashSet};

use super::{Hub, Shared};
use crate::lobby::Lobby;
use crate::player_profile;
use crate::ranked::RankedStats;
use crate::ws::protocol::ServerMessage;

/// Tentativas por jogador enquanto ele está no lobby. Uma falha (LCU ainda
/// subindo) tenta de novo no próximo evento do lobby, sem repetir para
/// sempre.
const MAX_ATTEMPTS: u8 = 3;

/// Riot ID de um membro: nome e tag.
type RiotId = (String, String);

/// Ranks e nomes já buscados dos membros do lobby.
#[derive(Debug, Default)]
pub(super) struct LobbyRanks {
    known: HashMap<String, RankedStats>,
    names: HashMap<String, RiotId>,
    attempts: HashMap<String, u8>,
    in_flight: HashSet<String>,
}

impl LobbyRanks {
    /// Esquece tudo. Uma busca que ainda está saindo continua, e o resultado
    /// dela entra se o jogador ainda estiver no lobby.
    pub(super) fn clear(&mut self) {
        self.known.clear();
        self.names.clear();
        self.attempts.clear();
        self.in_flight.clear();
    }
}

/// Preenche o elo e o Riot ID dos membros de `lobby` com o que já se sabe.
pub(super) fn fill_ranks(shared: &Shared, lobby: &mut Lobby) {
    let summoner = shared.state.summoner.as_ref();
    for member in &mut lobby.members {
        let me = summoner.filter(|s| s.puuid == member.puuid);
        let mine = me.is_some() && shared.state.ranked.is_some();
        member.ranked = if mine {
            shared.state.ranked.clone()
        } else {
            shared.lobby_ranks.known.get(&member.puuid).cloned()
        };
        let (game_name, tag_line) = match me {
            Some(s) => (s.game_name.clone(), s.tag_line.clone()),
            None => shared
                .lobby_ranks
                .names
                .get(&member.puuid)
                .cloned()
                .unwrap_or_default(),
        };
        member.game_name = game_name;
        member.tag_line = tag_line;
    }
}

/// Publica o lobby atual de novo se o elo de algum membro mudou. Não conta
/// como escrita do lobby (veja [`Stamp`](super::Stamp)): nada da LCU mudou.
fn write_refilled_lobby(shared: &mut Shared) -> Option<ServerMessage> {
    let mut lobby = shared.state.lobby.clone()?;
    fill_ranks(shared, &mut lobby);
    if shared.state.lobby.as_ref() == Some(&lobby) {
        return None;
    }
    shared.state.lobby = Some(lobby.clone());
    Some(ServerMessage::Lobby { lobby: Some(lobby) })
}

/// Membros do lobby cujo elo ou nome ainda falta buscar, já marcados como em
/// busca.
fn take_wanted(shared: &mut Shared) -> Vec<String> {
    let Some(lobby) = &shared.state.lobby else {
        return Vec::new();
    };
    let me = shared.state.summoner.as_ref().map(|s| s.puuid.as_str());
    let ranks = &mut shared.lobby_ranks;
    let wanted: Vec<String> = lobby
        .members
        .iter()
        .map(|m| m.puuid.as_str())
        .filter(|puuid| player_profile::valid_puuid(puuid) && Some(*puuid) != me)
        .filter(|puuid| {
            !(ranks.known.contains_key(*puuid) && ranks.names.contains_key(*puuid))
                && !ranks.in_flight.contains(*puuid)
        })
        .filter(|puuid| ranks.attempts.get(*puuid).copied().unwrap_or(0) < MAX_ATTEMPTS)
        .map(str::to_owned)
        .collect();
    for puuid in &wanted {
        ranks.in_flight.insert(puuid.clone());
        *ranks.attempts.entry(puuid.clone()).or_default() += 1;
    }
    wanted
}

/// Guarda o elo e o nome que chegaram de `puuid`, se ele ainda está no
/// lobby, e publica o lobby com eles.
fn write_fetched(
    shared: &mut Shared,
    puuid: &str,
    ranked: Option<RankedStats>,
    name: Option<RiotId>,
) -> Option<ServerMessage> {
    shared.lobby_ranks.in_flight.remove(puuid);
    if ranked.is_none() && name.is_none() {
        return None;
    }
    let member = shared
        .state
        .lobby
        .as_ref()
        .is_some_and(|lobby| lobby.members.iter().any(|m| m.puuid == puuid));
    if !member {
        return None;
    }
    if let Some(ranked) = ranked {
        shared.lobby_ranks.known.insert(puuid.to_owned(), ranked);
    }
    if let Some(name) = name {
        shared.lobby_ranks.names.insert(puuid.to_owned(), name);
    }
    write_refilled_lobby(shared)
}

impl Hub {
    /// Publica o lobby de novo com os ranks do jogador logado, que acabaram
    /// de mudar.
    pub(super) fn refill_lobby_ranks(&self) {
        self.update(write_refilled_lobby);
    }

    /// Esquece os ranks dos outros membros (fim de partida) e busca de novo.
    pub(super) fn reload_lobby_ranks(&self) {
        self.lock().lobby_ranks.clear();
        self.spawn_fetch_lobby_ranks();
    }

    /// Busca o elo e o nome de quem entrou no lobby, uma tarefa por jogador.
    pub(super) fn spawn_fetch_lobby_ranks(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (client, session, wanted) = {
            let mut guard = self.lock();
            let shared = &mut *guard;
            let Some(client) = shared.client.clone() else {
                return;
            };
            (client, shared.session, take_wanted(shared))
        };
        for puuid in wanted {
            let hub = self.clone();
            let client = client.clone();
            runtime.spawn(async move {
                let (ranked, summoner) = tokio::join!(
                    player_profile::ranked(&client, &puuid),
                    player_profile::summoner(&client, &puuid),
                );
                let ranked = ranked.ok().flatten();
                // Sem Riot ID (conta antiga), o nome antigo de invocador.
                let name = summoner
                    .ok()
                    .flatten()
                    .map(|(s, _)| {
                        if s.game_name.is_empty() {
                            (s.display_name, String::new())
                        } else {
                            (s.game_name, s.tag_line)
                        }
                    })
                    .filter(|(name, _)| !name.is_empty());
                hub.update(|shared| {
                    if shared.session != session {
                        return None;
                    }
                    write_fetched(shared, &puuid, ranked, name)
                });
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ranked::RankedEntry;
    use serde_json::json;

    fn lobby(puuids: &[&str]) -> Lobby {
        let members: Vec<_> = puuids
            .iter()
            .map(|p| json!({"puuid": p, "summonerId": 1, "isLeader": false}))
            .collect();
        Lobby::from_json(&json!({"gameConfig": {"queueId": 420}, "members": members})).unwrap()
    }

    fn gold() -> RankedStats {
        RankedStats {
            solo_duo: RankedEntry {
                tier: Some("GOLD".into()),
                division: Some("II".into()),
                league_points: 45,
                ..RankedEntry::default()
            },
            ..RankedStats::default()
        }
    }

    fn login(hub: &Hub, puuid: &str) {
        let summoner =
            serde_json::from_value(json!({"puuid": puuid, "summonerLevel": 1, "summonerId": 1}))
                .unwrap();
        hub.set_summoner(Some(summoner));
    }

    fn ana() -> RiotId {
        ("Ana".to_owned(), "BR1".to_owned())
    }

    fn names_of(hub: &Hub) -> Vec<String> {
        let lobby = hub.state().lobby.unwrap();
        lobby
            .members
            .into_iter()
            .map(|m| format!("{}#{}", m.game_name, m.tag_line))
            .collect()
    }

    fn ranks_of(hub: &Hub) -> Vec<Option<RankedStats>> {
        let state = hub.state();
        let lobby = state.lobby.unwrap();
        lobby.members.into_iter().map(|m| m.ranked).collect()
    }

    #[test]
    fn busca_so_quem_falta_e_sem_repetir() {
        let hub = Hub::new();
        login(&hub, "eu");
        hub.set_lobby(Some(lobby(&["eu", "amigo", ""])));
        let mut shared = hub.lock();
        // O jogador logado usa os próprios ranks; puuid vazio não se busca.
        assert_eq!(take_wanted(&mut shared), vec!["amigo".to_owned()]);
        // Em busca: não sai outra.
        assert!(take_wanted(&mut shared).is_empty());
        // Falhou: tenta de novo até o limite.
        for _ in 1..MAX_ATTEMPTS {
            write_fetched(&mut shared, "amigo", None, None);
            assert_eq!(take_wanted(&mut shared).len(), 1);
        }
        write_fetched(&mut shared, "amigo", None, None);
        assert!(take_wanted(&mut shared).is_empty());
    }

    #[test]
    fn elo_buscado_entra_no_lobby() {
        let hub = Hub::new();
        login(&hub, "eu");
        hub.set_lobby(Some(lobby(&["eu", "amigo"])));
        let (_, mut rx) = hub.subscribe();
        hub.update(|shared| {
            take_wanted(shared);
            write_fetched(shared, "amigo", Some(gold()), None)
        });
        assert!(matches!(rx.try_recv(), Ok(ServerMessage::Lobby { .. })));
        assert_eq!(ranks_of(&hub), vec![None, Some(gold())]);
        // O próximo evento do lobby já vem com o elo da cache.
        hub.set_lobby(Some(lobby(&["eu", "amigo"])));
        assert!(rx.try_recv().is_err(), "nada mudou");
        assert_eq!(ranks_of(&hub), vec![None, Some(gold())]);
    }

    #[test]
    fn ranks_do_jogador_logado() {
        let hub = Hub::new();
        login(&hub, "eu");
        hub.set_lobby(Some(lobby(&["eu"])));
        assert_eq!(ranks_of(&hub), vec![None]);
        hub.set_ranked(Some(gold()));
        assert_eq!(ranks_of(&hub), vec![Some(gold())]);
    }

    #[test]
    fn quem_saiu_nao_entra_e_lobby_fechado_limpa() {
        let hub = Hub::new();
        hub.set_lobby(Some(lobby(&["amigo"])));
        hub.update(|shared| {
            take_wanted(shared);
            write_fetched(shared, "outro", Some(gold()), None)
        });
        assert!(hub.lock().lobby_ranks.known.is_empty());
        hub.update(|shared| write_fetched(shared, "amigo", Some(gold()), None));
        assert_eq!(ranks_of(&hub), vec![Some(gold())]);
        hub.set_lobby(None);
        assert!(hub.lock().lobby_ranks.known.is_empty());
        hub.set_lobby(Some(lobby(&["amigo"])));
        assert_eq!(ranks_of(&hub), vec![None]);
    }

    #[test]
    fn nome_buscado_entra_no_lobby() {
        let hub = Hub::new();
        let summoner = serde_json::from_value(json!({
            "puuid": "eu", "gameName": "Eu", "tagLine": "000",
            "summonerLevel": 1, "summonerId": 1
        }))
        .unwrap();
        hub.set_summoner(Some(summoner));
        hub.set_lobby(Some(lobby(&["eu", "outro"])));
        // O jogador logado já tem o nome; o outro chega vazio.
        assert_eq!(names_of(&hub), vec!["Eu#000", "#"]);
        hub.update(|shared| {
            assert_eq!(take_wanted(shared), vec!["outro".to_owned()]);
            write_fetched(shared, "outro", None, Some(ana()))
        });
        assert_eq!(names_of(&hub), vec!["Eu#000", "Ana#BR1"]);
        assert_eq!(ranks_of(&hub), vec![None, None]);
        // O elo ainda falta: busca de novo, e o nome fica na cache.
        let mut shared = hub.lock();
        assert_eq!(take_wanted(&mut shared), vec!["outro".to_owned()]);
        write_fetched(&mut shared, "outro", Some(gold()), None);
        assert!(take_wanted(&mut shared).is_empty());
        drop(shared);
        assert_eq!(names_of(&hub), vec!["Eu#000", "Ana#BR1"]);
        assert_eq!(ranks_of(&hub), vec![None, Some(gold())]);
    }
}
