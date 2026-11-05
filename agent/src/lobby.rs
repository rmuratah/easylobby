//! Lobby e escolha de fila (#17).
//!
//! A LCU guarda o lobby do jogador em `/lol-lobby/v2/lobby`:
//!
//! - `GET` devolve o lobby atual, ou `404` fora de lobby;
//! - `POST {"queueId": 420}` cria um lobby na fila pedida, ou troca a fila do
//!   lobby atual (só o líder pode);
//! - `DELETE` sai do lobby.
//!
//! Cada mudança chega pelo WAMP no mesmo URI. O [`Hub`](crate::ws::Hub) lê o
//! lobby pelo REST a cada conexão e depois segue os eventos; a UI recebe o
//! [`Lobby`] pelo `/ws` e pede as mudanças com os comandos `createLobby` e
//! `leaveLobby`.
//!
//! O [`Lobby`] guarda só o que a tela de lobby usa. Os outros campos da
//! resposta são ignorados, então um campo novo num patch não quebra a
//! leitura. Cargos (#18) e fila (#19) entram depois.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) do lobby.
pub const LOBBY_PATH: &str = "/lol-lobby/v2/lobby";

/// Filas que o easylobby oferece, com o `queueId` da Riot de cada uma.
///
/// | Fila | `queueId` |
/// | --- | --- |
/// | Normal, escolha alternada | 400 |
/// | Jogo Dinâmico (Swiftplay) | 480 |
/// | Ranqueada Solo/Duo | 420 |
/// | Ranqueada Flex | 440 |
/// | ARAM | 450 |
///
/// Os ids estão na lista de filas da LCU (`/lol-game-queues/v1/queues`) da
/// gravação 16.22.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Queue {
    /// Normal com escolha alternada (draft), `queueId` 400.
    NormalDraft,
    /// Jogo Dinâmico, a normal rápida que substituiu a escolha às cegas,
    /// `queueId` 480.
    Swiftplay,
    /// Ranqueada Solo/Duo, `queueId` 420.
    RankedSolo,
    /// Ranqueada Flex, `queueId` 440.
    RankedFlex,
    /// ARAM, `queueId` 450.
    Aram,
}

impl Queue {
    /// Todas as filas, na ordem em que a tela mostra.
    pub const ALL: [Queue; 5] = [
        Queue::NormalDraft,
        Queue::Swiftplay,
        Queue::RankedSolo,
        Queue::RankedFlex,
        Queue::Aram,
    ];

    /// `queueId` da Riot.
    pub const fn id(self) -> u32 {
        match self {
            Queue::NormalDraft => 400,
            Queue::Swiftplay => 480,
            Queue::RankedSolo => 420,
            Queue::RankedFlex => 440,
            Queue::Aram => 450,
        }
    }

    /// A fila de um `queueId`, se for uma das que o easylobby oferece.
    pub fn from_id(id: u32) -> Option<Queue> {
        Queue::ALL.into_iter().find(|q| q.id() == id)
    }
}

/// O lobby do jogador, como a tela de lobby precisa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Lobby {
    /// `queueId` da Riot, mesmo de uma fila que o easylobby não oferece
    /// (personalizada, TFT, evento...).
    pub queue_id: u32,
    /// A fila do lobby, ou `null` quando não é uma das [`Queue`].
    pub queue: Option<Queue>,
    /// Quantos jogadores cabem no grupo nesta fila (2 na Solo/Duo, 5 nas
    /// outras).
    pub max_lobby_size: u32,
    /// Se o jogador logado é o líder: só o líder troca a fila.
    pub is_leader: bool,
    /// Se a LCU deixa entrar na fila agora.
    pub can_start_activity: bool,
    /// Quem está no lobby, o jogador logado incluído.
    pub members: Vec<LobbyMember>,
}

/// Um jogador no lobby. A LCU não manda o nome aqui; a UI cruza o `puuid`
/// com o invocador logado ou com a lista de amigos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LobbyMember {
    pub puuid: String,
    // Cabe num `number` do JavaScript: os ids da Riot são bem menores que 2^53.
    #[ts(type = "number")]
    pub summoner_id: u64,
    pub summoner_icon_id: u32,
    pub summoner_level: u32,
    pub is_leader: bool,
}

impl Lobby {
    /// Lê o lobby do JSON da LCU (resposta do `GET` ou `data` de um evento).
    /// `None` se não for um lobby.
    pub fn from_json(value: &Value) -> Option<Lobby> {
        let raw = RawLobby::deserialize(value).ok()?;
        let members = raw
            .members
            .into_iter()
            .map(|m| LobbyMember {
                puuid: m.puuid,
                summoner_id: m.summoner_id,
                summoner_icon_id: m.summoner_icon_id,
                summoner_level: m.summoner_level,
                is_leader: m.is_leader,
            })
            .collect();
        Some(Lobby {
            queue_id: raw.game_config.queue_id,
            queue: Queue::from_id(raw.game_config.queue_id),
            max_lobby_size: raw.game_config.max_lobby_size,
            is_leader: raw.local_member.is_some_and(|m| m.is_leader),
            can_start_activity: raw.can_start_activity,
            members,
        })
    }
}

/// O lobby como a LCU manda, só com os campos usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLobby {
    game_config: RawGameConfig,
    #[serde(default)]
    can_start_activity: bool,
    #[serde(default)]
    local_member: Option<RawMember>,
    #[serde(default)]
    members: Vec<RawMember>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGameConfig {
    queue_id: u32,
    #[serde(default)]
    max_lobby_size: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMember {
    #[serde(default)]
    puuid: String,
    #[serde(default)]
    summoner_id: u64,
    #[serde(default)]
    summoner_icon_id: u32,
    #[serde(default)]
    summoner_level: u32,
    #[serde(default)]
    is_leader: bool,
}

/// Busca o lobby atual. Fora de lobby a LCU responde `404`, que volta como
/// `Ok(None)`.
pub async fn current_lobby(lcu: &LcuClient) -> Result<Option<Lobby>, LcuError> {
    match lcu.get::<Value>(LOBBY_PATH).await {
        Ok(body) => Ok(Lobby::from_json(&body)),
        Err(error) if error.status() == Some(404) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Cria um lobby na fila `queue`, ou troca a fila do lobby atual
/// (`POST /lol-lobby/v2/lobby`). Devolve o lobby que a LCU respondeu.
pub async fn create_lobby(lcu: &LcuClient, queue: Queue) -> Result<Option<Lobby>, LcuError> {
    let body: Value = lcu
        .post(LOBBY_PATH, &json!({ "queueId": queue.id() }))
        .await?;
    Ok(Lobby::from_json(&body))
}

/// Sai do lobby (`DELETE /lol-lobby/v2/lobby`).
pub async fn leave_lobby(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.delete::<Option<Value>>(LOBBY_PATH).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_das_filas() {
        for queue in Queue::ALL {
            assert_eq!(Queue::from_id(queue.id()), Some(queue));
        }
        assert_eq!(Queue::RankedSolo.id(), 420);
        assert_eq!(Queue::from_id(1700), None);
        assert_eq!(
            serde_json::to_value(Queue::RankedSolo).unwrap(),
            json!("rankedSolo")
        );
        assert_eq!(
            serde_json::from_value::<Queue>(json!("aram")).unwrap(),
            Queue::Aram
        );
    }

    #[test]
    fn le_o_lobby_gravado() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/16.22/rest.json");
        let rest: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let body = rest
            .iter()
            .find(|r| r["path"] == LOBBY_PATH && r["status"] == 200)
            .map(|r| &r["body"])
            .unwrap();
        let lobby = Lobby::from_json(body).unwrap();
        assert_eq!(lobby.queue_id, 420);
        assert_eq!(lobby.queue, Some(Queue::RankedSolo));
        assert_eq!(lobby.max_lobby_size, 2);
        assert!(lobby.is_leader);
        assert_eq!(lobby.members.len(), 1);
        assert_eq!(lobby.members[0].puuid, "fake-puuid-0001");
        assert_eq!(lobby.members[0].summoner_level, 122);
        assert!(lobby.members[0].is_leader);
    }

    #[test]
    fn fila_desconhecida_e_json_que_nao_e_lobby() {
        let lobby = Lobby::from_json(&json!({"gameConfig": {"queueId": 1700}})).unwrap();
        assert_eq!(lobby.queue_id, 1700);
        assert_eq!(lobby.queue, None);
        assert!(!lobby.is_leader);
        assert!(lobby.members.is_empty());

        assert_eq!(Lobby::from_json(&json!(null)), None);
        assert_eq!(Lobby::from_json(&json!({"errorCode": "RPC_ERROR"})), None);
    }

    #[test]
    fn formato_para_a_ui() {
        let lobby = Lobby {
            queue_id: 450,
            queue: Some(Queue::Aram),
            max_lobby_size: 5,
            is_leader: true,
            can_start_activity: true,
            members: vec![LobbyMember {
                puuid: "p".into(),
                summoner_id: 1,
                summoner_icon_id: 2,
                summoner_level: 3,
                is_leader: true,
            }],
        };
        assert_eq!(
            serde_json::to_value(&lobby).unwrap(),
            json!({
                "queueId": 450,
                "queue": "aram",
                "maxLobbySize": 5,
                "isLeader": true,
                "canStartActivity": true,
                "members": [{
                    "puuid": "p",
                    "summonerId": 1,
                    "summonerIconId": 2,
                    "summonerLevel": 3,
                    "isLeader": true,
                }],
            })
        );
    }
}
