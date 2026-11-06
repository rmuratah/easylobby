//! Lista de amigos e a presença de cada um (#30).

use super::Hub;

use crate::friends::{self, Friend};
use crate::ws::protocol::ServerMessage;

impl Hub {
    /// Troca a lista de amigos inteira (#30), que deve vir na ordem de
    /// [`friends::sort_friends`].
    pub fn set_friends(&self, list: Vec<Friend>) {
        self.update(|shared| {
            if shared.state.friends == list {
                return None;
            }
            shared.state.friends = list.clone();
            Some(ServerMessage::Friends { friends: list })
        });
    }

    /// Põe ou atualiza um amigo na lista, mantendo a ordem. Publica a lista
    /// inteira só se algo que a UI mostra mudou.
    pub(super) fn upsert_friend(&self, friend: Friend) {
        self.update(|shared| {
            let list = &mut shared.state.friends;
            match list.iter_mut().find(|f| f.id == friend.id) {
                Some(current) if *current == friend => return None,
                Some(current) => *current = friend,
                None => list.push(friend),
            }
            friends::sort_friends(list);
            Some(ServerMessage::Friends {
                friends: list.clone(),
            })
        });
    }

    /// Tira um amigo da lista (amizade desfeita).
    pub(super) fn remove_friend(&self, id: &str) {
        self.update(|shared| {
            let list = &mut shared.state.friends;
            let before = list.len();
            list.retain(|f| f.id != id);
            if list.len() == before {
                return None;
            }
            Some(ServerMessage::Friends {
                friends: list.clone(),
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::connection::ConnectionEvent;
    use crate::wamp::EventType;
    use serde_json::json;

    fn friend(id: &str, availability: &str, game_status: &str) -> serde_json::Value {
        json!({
            "id": id,
            "pid": id,
            "puuid": format!("puuid-{id}"),
            "gameName": id,
            "gameTag": "BR1",
            "icon": 1,
            "availability": availability,
            "product": "league_of_legends",
            "productName": "League of Legends",
            "lol": {"gameStatus": game_status},
            "time": 1,
        })
    }

    fn friend_ids(message: ServerMessage) -> Vec<String> {
        match message {
            ServerMessage::Friends { friends } => friends.into_iter().map(|f| f.id).collect(),
            other => panic!("esperava a lista de amigos: {other:?}"),
        }
    }

    #[tokio::test]
    async fn segue_os_amigos() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.set_friends(Friend::list_from_json(&json!([
            friend("b@br1.pvp.net", "chat", "outOfGame"),
            friend("c@br1.pvp.net", "offline", ""),
        ])));
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["b@br1.pvp.net", "c@br1.pvp.net"]
        );

        // Alguém ficou online: entra na posição certa. O URI vem escapado.
        hub.apply(event(
            "/lol-chat/v1/friends/c%40br1.pvp.net",
            EventType::Update,
            friend("c@br1.pvp.net", "chat", "inQueue"),
        ))
        .await;
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["b@br1.pvp.net", "c@br1.pvp.net"]
        );
        assert_eq!(
            hub.state().friends[1].activity,
            crate::friends::Friend::from_json(&friend("c@br1.pvp.net", "chat", "inQueue"))
                .unwrap()
                .activity
        );

        // Só o `time` mudou: nada que a UI mostre, nenhuma mensagem.
        let mut same = friend("c@br1.pvp.net", "chat", "inQueue");
        same["time"] = json!(2);
        hub.apply(event(
            "/lol-chat/v1/friends/c%40br1.pvp.net",
            EventType::Update,
            same,
        ))
        .await;
        assert!(rx.try_recv().is_err());

        // Amigo novo, e um que vai para o fim da lista.
        hub.apply(event(
            "/lol-chat/v1/friends/a%40br1.pvp.net",
            EventType::Create,
            friend("a@br1.pvp.net", "chat", "outOfGame"),
        ))
        .await;
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["a@br1.pvp.net", "b@br1.pvp.net", "c@br1.pvp.net"]
        );
        hub.apply(event(
            "/lol-chat/v1/friends/a%40br1.pvp.net",
            EventType::Update,
            friend("a@br1.pvp.net", "offline", ""),
        ))
        .await;
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["b@br1.pvp.net", "c@br1.pvp.net", "a@br1.pvp.net"]
        );

        // Amizade desfeita, com `data` vazio.
        hub.apply(event(
            "/lol-chat/v1/friends/b%40br1.pvp.net",
            EventType::Delete,
            json!(null),
        ))
        .await;
        assert_eq!(
            friend_ids(rx.try_recv().unwrap()),
            ["c@br1.pvp.net", "a@br1.pvp.net"]
        );

        // Outros recursos do chat e lixo: ignorados.
        hub.apply(event(
            "/lol-chat/v1/friend-counts",
            EventType::Update,
            json!({"numFriends": 2}),
        ))
        .await;
        hub.apply(event(
            "/lol-chat/v1/friends/x%40br1.pvp.net",
            EventType::Update,
            json!("lixo"),
        ))
        .await;
        hub.apply(event(
            "/lol-chat/v1/friends/c%40br1.pvp.net/algo",
            EventType::Delete,
            json!(null),
        ))
        .await;
        assert!(rx.try_recv().is_err());

        // Queda do LoL esvazia a lista.
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        let mut cleared = false;
        while let Ok(message) = rx.try_recv() {
            if let ServerMessage::Friends { friends } = message {
                assert!(friends.is_empty());
                cleared = true;
            }
        }
        assert!(cleared);
        assert!(hub.state().friends.is_empty());
    }
}
