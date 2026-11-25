//! Nick e ícone de quem não é amigo (#675): convites recebidos, sugeridos e
//! recentes.
//!
//! Sempre que uma dessas listas muda, o agente vê quem nelas não é amigo (os
//! amigos já têm nome e ícone na lista de amigos) e ainda não tem
//! [`PlayerCard`], busca o invocador de cada um numa tarefa separada e
//! publica `playerCards` com o que chegou. Os cards de quem saiu das listas
//! somem, e tudo é esquecido quando o LoL fecha.

use std::collections::{HashMap, HashSet};

use super::{Hub, Shared};
use crate::player_cards::{self, PlayerCard, PlayerKey};
use crate::ws::protocol::ServerMessage;

/// Tentativas por jogador enquanto ele aparece nas listas. Uma falha (LCU
/// ainda subindo) tenta de novo na próxima mudança das listas.
const MAX_ATTEMPTS: u8 = 3;

/// O que já foi pedido à LCU.
#[derive(Debug, Default)]
pub(super) struct PlayerCardSync {
    attempts: HashMap<PlayerKey, u8>,
    in_flight: HashSet<PlayerKey>,
}

/// Quem aparece nos convites recebidos, nos sugeridos e nos recentes, sem o
/// jogador logado e sem os amigos que já têm ícone, sem repetir.
fn referenced(shared: &Shared) -> Vec<PlayerKey> {
    let state = &shared.state;
    let me = state.summoner.as_ref();
    let is_me = |puuid: &str, id: u64| {
        me.is_some_and(|s| {
            (!puuid.is_empty() && s.puuid == puuid) || (id != 0 && s.summoner_id == id)
        })
    };
    let is_friend = |puuid: &str, id: u64| {
        state.friends.iter().any(|f| {
            f.profile_icon_id.is_some()
                && ((!puuid.is_empty() && f.puuid == puuid) || (id != 0 && f.summoner_id == id))
        })
    };
    let invitations = state
        .received_invitations
        .iter()
        .map(|i| (String::new(), i.from_summoner_id));
    let suggested = state
        .suggested_players
        .iter()
        .map(|p| (p.puuid.clone(), p.summoner_id));
    let recent = state
        .recent_players
        .iter()
        .map(|p| (p.puuid.clone(), p.summoner_id));
    let mut seen = HashSet::new();
    invitations
        .chain(suggested)
        .chain(recent)
        .filter(|(puuid, id)| !is_me(puuid, *id) && !is_friend(puuid, *id))
        .filter_map(|(puuid, id)| PlayerKey::of(&puuid, id))
        .filter(|key| seen.insert(key.clone()))
        .collect()
}

/// Tira os cards de quem não aparece mais. Publica se algum saiu.
fn write_pruned(shared: &mut Shared, keys: &[PlayerKey]) -> Option<ServerMessage> {
    let before = shared.state.player_cards.len();
    shared
        .state
        .player_cards
        .retain(|card| keys.iter().any(|key| key.matches(card)));
    if shared.state.player_cards.len() == before {
        return None;
    }
    Some(ServerMessage::PlayerCards {
        player_cards: shared.state.player_cards.clone(),
    })
}

/// Quem ainda falta buscar, já marcado como em busca.
fn take_wanted(shared: &mut Shared, keys: Vec<PlayerKey>) -> Vec<PlayerKey> {
    let cards = &shared.state.player_cards;
    let sync = &mut shared.player_cards;
    let wanted: Vec<PlayerKey> = keys
        .into_iter()
        .filter(|key| !cards.iter().any(|card| key.matches(card)))
        .filter(|key| !sync.in_flight.contains(key))
        .filter(|key| sync.attempts.get(key).copied().unwrap_or(0) < MAX_ATTEMPTS)
        .collect();
    for key in &wanted {
        sync.in_flight.insert(key.clone());
        *sync.attempts.entry(key.clone()).or_default() += 1;
    }
    wanted
}

/// Guarda o card que chegou de `key`, se ele ainda aparece nas listas.
fn write_fetched(
    shared: &mut Shared,
    key: &PlayerKey,
    card: Option<PlayerCard>,
) -> Option<ServerMessage> {
    shared.player_cards.in_flight.remove(key);
    let card = card?;
    if !referenced(shared).contains(key) {
        return None;
    }
    let cards = &mut shared.state.player_cards;
    match cards.iter_mut().find(|c| key.matches(c)) {
        Some(old) if *old == card => return None,
        Some(old) => *old = card,
        None => cards.push(card),
    }
    Some(ServerMessage::PlayerCards {
        player_cards: cards.clone(),
    })
}

impl Hub {
    /// Acerta os cards com as listas atuais: tira os de quem saiu e busca
    /// quem entrou, um jogador por tarefa. Sem LoL só tira.
    pub(super) fn sync_player_cards(&self) {
        let fetch = {
            let mut guard = self.lock();
            let shared = &mut *guard;
            let keys = referenced(shared);
            if keys.is_empty() {
                // LoL fechado ou listas vazias: recomeça as tentativas.
                shared.player_cards = PlayerCardSync::default();
            }
            if let Some(message) = write_pruned(shared, &keys) {
                let _ = self.inner.tx.send(message);
            }
            let runtime = tokio::runtime::Handle::try_current().ok();
            match (shared.client.clone(), runtime) {
                (Some(client), Some(runtime)) => {
                    Some((client, runtime, shared.session, take_wanted(shared, keys)))
                }
                _ => None,
            }
        };
        let Some((client, runtime, session, wanted)) = fetch else {
            return;
        };
        for key in wanted {
            let hub = self.clone();
            let client = client.clone();
            runtime.spawn(async move {
                let card = player_cards::fetch(&client, &key).await.ok().flatten();
                hub.update(|shared| {
                    if shared.session != session {
                        return None;
                    }
                    write_fetched(shared, &key, card)
                });
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invitations::ReceivedInvitation;
    use serde_json::json;

    fn invitation(from: u64) -> ReceivedInvitation {
        ReceivedInvitation::list_from_json(&json!([{
            "invitationId": format!("c{from}"), "state": "Pending", "fromSummonerId": from,
            "canAcceptInvitation": true, "gameConfig": {"queueId": 450}
        }]))
        .unwrap()
        .remove(0)
    }

    fn card(puuid: &str, id: u64) -> PlayerCard {
        PlayerCard {
            puuid: puuid.into(),
            summoner_id: id,
            game_name: "Zed".into(),
            tag_line: "BR1".into(),
            profile_icon_id: 29,
        }
    }

    #[test]
    fn busca_quem_nao_e_amigo_nem_eu() {
        let hub = Hub::new();
        let summoner =
            serde_json::from_value(json!({"puuid": "eu", "summonerLevel": 1, "summonerId": 1}))
                .unwrap();
        hub.set_summoner(Some(summoner));
        let friend = crate::friends::Friend {
            id: "a".into(),
            puuid: "amigo".into(),
            summoner_id: 2,
            game_name: "Ana".into(),
            tag_line: "BR1".into(),
            name: "Ana".into(),
            profile_icon_id: Some(7),
            availability: crate::friends::Availability::Online,
            status_message: String::new(),
            group_id: 0,
            group_name: String::new(),
            activity: None,
        };
        hub.set_friends(vec![friend]);
        hub.set_received_invitations(vec![
            invitation(1),
            invitation(2),
            invitation(3),
            invitation(3),
        ]);
        let shared = hub.lock();
        assert_eq!(referenced(&shared), [PlayerKey::SummonerId(3)]);
    }

    #[test]
    fn guarda_o_card_de_quem_ainda_aparece_e_tira_quem_saiu() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.set_received_invitations(vec![invitation(3)]);
        let _ = rx.try_recv();
        let key = PlayerKey::SummonerId(3);
        hub.update(|shared| write_fetched(shared, &key, Some(card("p3", 3))));
        assert_eq!(hub.state().player_cards, [card("p3", 3)]);
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::PlayerCards {
                player_cards: vec![card("p3", 3)]
            }
        );
        // Repetido não publica; de quem não aparece não entra.
        hub.update(|shared| write_fetched(shared, &key, Some(card("p3", 3))));
        hub.update(|shared| write_fetched(shared, &PlayerKey::SummonerId(9), Some(card("p9", 9))));
        assert!(rx.try_recv().is_err());
        // O convite some: o card some junto.
        hub.set_received_invitations(Vec::new());
        assert!(hub.state().player_cards.is_empty());
        let seen: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(seen.contains(&ServerMessage::PlayerCards {
            player_cards: vec![]
        }));
    }

    #[test]
    fn tenta_no_maximo_tres_vezes() {
        let hub = Hub::new();
        hub.set_received_invitations(vec![invitation(3)]);
        let mut shared = hub.lock();
        let key = PlayerKey::SummonerId(3);
        for _ in 0..MAX_ATTEMPTS {
            assert_eq!(
                take_wanted(&mut shared, vec![key.clone()]),
                std::slice::from_ref(&key)
            );
            // Em busca: não pede de novo.
            assert!(take_wanted(&mut shared, vec![key.clone()]).is_empty());
            write_fetched(&mut shared, &key, None);
        }
        assert!(take_wanted(&mut shared, vec![key]).is_empty());
    }
}
