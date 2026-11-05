//! Lista de amigos com presença (#30).
//!
//! A LCU guarda os amigos no chat, em `GET /lol-chat/v1/friends`: um array
//! com um objeto por amigo, com o Riot ID, a disponibilidade (`chat`,
//! `away`, `dnd`, `mobile`, `offline`), o jogo aberto (`product`) e, para
//! quem está no LoL, o objeto `lol` com o que a pessoa está fazendo
//! (`gameStatus`: fora de partida, em lobby, na fila, na seleção, em jogo).
//! Todos os valores dentro de `lol` são strings, até os números.
//!
//! Cada mudança chega pelo WAMP em `/lol-chat/v1/friends/<id>`, com o amigo
//! inteiro em `data`: `Create` quando alguém vira amigo, `Update` quando a
//! presença muda e `Delete` quando a amizade acaba. O `<id>` é o `pid` do
//! amigo (`puuid@plataforma.pvp.net`), com o `@` escapado como `%40`.
//!
//! O [`Friend`] guarda só o que a lista de amigos usa. Os outros campos (grupo,
//! nota, ranque, Discord, banner...) são ignorados, então um campo novo num
//! patch não quebra a leitura. Em particular, os grupos de amigos ficam de
//! fora: os nomes deles são preferências do jogador
//! ([`crate::wamp::EASYLOBBY_EVENT_EXCLUDED`]).

use std::borrow::Cow;
use std::cmp::Ordering;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint da lista de amigos.
pub const FRIENDS_PATH: &str = "/lol-chat/v1/friends";

/// Prefixo dos eventos WAMP de cada amigo: `/lol-chat/v1/friends/<id>`.
pub const FRIEND_EVENT_PREFIX: &str = "/lol-chat/v1/friends/";

/// `product` da LCU para quem está no LoL.
const LEAGUE_PRODUCT: &str = "league_of_legends";

/// Um amigo e o que ele está fazendo agora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    /// Identificador do amigo no chat (`pid`). É o que os comandos de amigos
    /// e de chat vão usar, e o que vem no URI dos eventos.
    pub id: String,
    pub puuid: String,
    /// Nome do Riot ID (antes do `#`). Vazio em contas antigas sem Riot ID;
    /// nesse caso use `name`.
    pub game_name: String,
    /// Tag do Riot ID (depois do `#`).
    pub tag_line: String,
    /// Nome antigo de invocador, quando a LCU manda.
    pub name: String,
    /// Ícone de invocador, para `/assets/profileIcon/<id>`. `null` quando a
    /// LCU não manda um ícone válido.
    pub profile_icon_id: Option<u32>,
    pub availability: Availability,
    /// Mensagem de status que o amigo escreveu. Vazia quando não tem.
    pub status_message: String,
    /// O que o amigo está fazendo: no LoL (e em que parte), em outro jogo da
    /// Riot ou `null` (offline, no celular ou sem informação).
    pub activity: Option<Activity>,
}

impl Friend {
    /// Lê um amigo do `data` de um evento WAMP ou de um item da lista REST.
    /// `None` quando o valor não tem o formato esperado.
    pub fn from_json(value: &Value) -> Option<Self> {
        let raw = LcuFriend::deserialize(value).ok()?;
        let id = if raw.id.is_empty() { raw.pid } else { raw.id };
        if id.is_empty() {
            return None;
        }
        let activity = match raw.availability {
            Availability::Offline | Availability::Mobile => None,
            _ if raw.product == LEAGUE_PRODUCT => {
                Some(Activity::League(LeagueActivity::from_lcu(raw.lol.as_ref())))
            }
            _ if raw.product.is_empty() => None,
            _ => Some(Activity::OtherGame {
                product_name: if raw.product_name.is_empty() {
                    raw.product
                } else {
                    raw.product_name
                },
            }),
        };
        Some(Friend {
            id,
            puuid: raw.puuid,
            game_name: raw.game_name,
            tag_line: raw.game_tag,
            name: raw.name,
            profile_icon_id: raw.icon.and_then(|icon| u32::try_from(icon).ok()),
            availability: raw.availability,
            status_message: raw.status_message,
            activity,
        })
    }

    /// Lê a lista de `GET /lol-chat/v1/friends`, já na ordem de
    /// [`sort_friends`]. Itens que não são amigos válidos ficam de fora.
    pub fn list_from_json(value: &Value) -> Vec<Self> {
        let mut friends: Vec<Friend> = value
            .as_array()
            .map(|items| items.iter().filter_map(Friend::from_json).collect())
            .unwrap_or_default();
        sort_friends(&mut friends);
        friends
    }

    /// Nome para mostrar: o Riot ID `nome#tag`, ou o nome antigo quando o
    /// Riot ID não veio.
    pub fn display_name(&self) -> String {
        match (self.game_name.is_empty(), self.tag_line.is_empty()) {
            (false, false) => format!("{}#{}", self.game_name, self.tag_line),
            (false, true) => self.game_name.clone(),
            (true, _) => self.name.clone(),
        }
    }
}

/// Ordem da lista: disponíveis primeiro ([`Availability::rank`]), depois por
/// nome sem diferenciar maiúsculas, e o `id` para desempatar.
pub fn sort_friends(friends: &mut [Friend]) {
    friends.sort_by(compare);
}

fn compare(a: &Friend, b: &Friend) -> Ordering {
    a.availability
        .rank()
        .cmp(&b.availability.rank())
        .then_with(|| {
            a.display_name()
                .to_lowercase()
                .cmp(&b.display_name().to_lowercase())
        })
        .then_with(|| a.id.cmp(&b.id))
}

/// Disponibilidade no chat. Os nomes da LCU são outros (veja
/// [`Availability::lcu_name`]); para a UI vão em camelCase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    /// Disponível (`chat`).
    Online,
    /// Ocupado (`dnd`). O client oficial põe assim quem está em partida.
    Busy,
    /// Ausente (`away`).
    Away,
    /// Só no app de celular (`mobile`).
    Mobile,
    /// Offline (`offline`).
    #[default]
    Offline,
    /// Valor que o agente não conhece.
    Unknown,
}

impl Availability {
    pub const ALL: [Availability; 6] = [
        Availability::Online,
        Availability::Busy,
        Availability::Away,
        Availability::Mobile,
        Availability::Offline,
        Availability::Unknown,
    ];

    /// Nome na LCU.
    pub fn lcu_name(self) -> &'static str {
        match self {
            Availability::Online => "chat",
            Availability::Busy => "dnd",
            Availability::Away => "away",
            Availability::Mobile => "mobile",
            Availability::Offline => "offline",
            Availability::Unknown => "unknown",
        }
    }

    /// Nome que vai para a UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Availability::Online => "online",
            Availability::Busy => "busy",
            Availability::Away => "away",
            Availability::Mobile => "mobile",
            Availability::Offline => "offline",
            Availability::Unknown => "unknown",
        }
    }

    /// Lê o nome da LCU ou o da UI (para o JSON do `/ws` voltar igual).
    /// Valores novos viram [`Availability::Unknown`].
    pub fn from_lcu_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|a| a.lcu_name() == name || a.as_str() == name)
            .unwrap_or(Availability::Unknown)
    }

    /// Posição na lista: quem pode jogar agora vem antes.
    pub fn rank(self) -> u8 {
        match self {
            Availability::Online => 0,
            Availability::Busy => 1,
            Availability::Away => 2,
            Availability::Unknown => 3,
            Availability::Mobile => 4,
            Availability::Offline => 5,
        }
    }
}

impl<'de> Deserialize<'de> for Availability {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::from_lcu_name(&name))
    }
}

/// O que um amigo online está fazendo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Activity {
    /// No LoL.
    League(LeagueActivity),
    /// Em outro jogo da Riot (VALORANT, TFT mobile...), pelo nome que a LCU
    /// manda.
    OtherGame { product_name: String },
}

/// O que um amigo está fazendo dentro do LoL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LeagueActivity {
    pub status: LeagueStatus,
    /// Fila do lobby, da busca ou da partida (`420` é a ranqueada solo).
    /// `null` fora de partida e quando a LCU não manda.
    pub queue_id: Option<i32>,
    /// Campeão da partida em andamento, para `/assets/champion/<id>`. `null`
    /// fora de partida.
    pub champion_id: Option<u32>,
    /// Quando o status atual começou (na fila, em jogo), em milissegundos
    /// desde 1970. `null` quando a LCU não manda.
    #[ts(type = "number | null")]
    pub since: Option<u64>,
}

impl LeagueActivity {
    fn from_lcu(lol: Option<&LcuLeaguePresence>) -> Self {
        let Some(lol) = lol else {
            return LeagueActivity {
                status: LeagueStatus::Unknown,
                queue_id: None,
                champion_id: None,
                since: None,
            };
        };
        LeagueActivity {
            status: LeagueStatus::from_lcu(&lol.game_status),
            queue_id: parse_number(&lol.queue_id).filter(|&id: &i32| id > 0),
            champion_id: parse_number(&lol.champion_id).filter(|&id: &u32| id > 0),
            since: parse_number(&lol.time_stamp).filter(|&ms: &u64| ms > 0),
        }
    }
}

/// Parte do LoL em que o amigo está, pelo `lol.gameStatus` da LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LeagueStatus {
    /// No client, fora de lobby (`outOfGame`).
    OutOfGame,
    /// Num lobby (`hosting_<fila>`, como `hosting_RANKED_SOLO_5x5`).
    InLobby,
    /// Na fila (`inQueue`).
    InQueue,
    /// Na seleção de campeões (`championSelect`).
    ChampionSelect,
    /// Em partida (`inGame`).
    InGame,
    /// Assistindo uma partida (`spectating`).
    Spectating,
    /// Status que o agente não conhece, ou que a LCU não mandou.
    Unknown,
}

impl LeagueStatus {
    pub fn from_lcu(game_status: &str) -> Self {
        match game_status {
            "outOfGame" => LeagueStatus::OutOfGame,
            "inQueue" => LeagueStatus::InQueue,
            "championSelect" => LeagueStatus::ChampionSelect,
            "inGame" => LeagueStatus::InGame,
            "spectating" => LeagueStatus::Spectating,
            s if s.starts_with("hosting_") => LeagueStatus::InLobby,
            _ => LeagueStatus::Unknown,
        }
    }
}

/// Um amigo como a LCU manda.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcuFriend {
    #[serde(default)]
    id: String,
    #[serde(default)]
    pid: String,
    #[serde(default)]
    puuid: String,
    #[serde(default, deserialize_with = "null_as_default")]
    game_name: String,
    #[serde(default, deserialize_with = "null_as_default")]
    game_tag: String,
    #[serde(default, deserialize_with = "null_as_default")]
    name: String,
    #[serde(default)]
    icon: Option<i64>,
    #[serde(default = "offline")]
    availability: Availability,
    #[serde(default, deserialize_with = "null_as_default")]
    status_message: String,
    #[serde(default, deserialize_with = "null_as_default")]
    product: String,
    #[serde(default, deserialize_with = "null_as_default")]
    product_name: String,
    #[serde(default)]
    lol: Option<LcuLeaguePresence>,
}

fn offline() -> Availability {
    Availability::Offline
}

/// O objeto `lol` da presença, com os campos que a lista usa. Tudo string.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcuLeaguePresence {
    #[serde(default, deserialize_with = "null_as_default")]
    game_status: String,
    #[serde(default, deserialize_with = "null_as_default")]
    queue_id: String,
    #[serde(default, deserialize_with = "null_as_default")]
    champion_id: String,
    #[serde(default, deserialize_with = "null_as_default")]
    time_stamp: String,
}

fn null_as_default<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn parse_number<T: std::str::FromStr>(text: &str) -> Option<T> {
    text.trim().parse().ok()
}

/// O `id` do amigo num URI de evento (`/lol-chat/v1/friends/<id>`), com o
/// escape `%XX` desfeito. `None` para outros URIs, inclusive subrecursos
/// (`/lol-chat/v1/friends/<id>/...`) e a própria lista.
pub fn friend_id_from_uri(uri: &str) -> Option<String> {
    let encoded = uri.strip_prefix(FRIEND_EVENT_PREFIX)?;
    if encoded.is_empty() || encoded.contains('/') {
        return None;
    }
    percent_decode(encoded)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Busca a lista de amigos, já ordenada. Com o chat ainda conectando a LCU
/// pode responder erro ou uma lista vazia; os eventos de cada amigo
/// completam a lista depois.
pub async fn friends(lcu: &LcuClient) -> Result<Vec<Friend>, LcuError> {
    let value = lcu.get::<Value>(FRIENDS_PATH).await?;
    Ok(Friend::list_from_json(&value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lcu(id: &str, availability: &str, product: &str, lol: Value) -> Value {
        json!({
            "availability": availability,
            "discordId": null,
            "displayGroupId": 0,
            "displayGroupName": "**Default",
            "gameName": format!("nome-{id}"),
            "gameTag": "BR1",
            "groupId": 0,
            "groupName": "**Default",
            "icon": 4977,
            "id": id,
            "lastSeenOnlineTimestamp": null,
            "lol": lol,
            "name": "",
            "note": "",
            "patchline": "live",
            "pid": id,
            "platformId": "BR1",
            "product": product,
            "productName": if product == "valorant" { "VALORANT" } else { "League of Legends" },
            "puuid": format!("puuid-{id}"),
            "statusMessage": "",
            "summonerId": 1003,
            "time": 1793841180340u64,
        })
    }

    #[test]
    fn le_um_amigo_no_lol() {
        let friend = Friend::from_json(&lcu(
            "a@br1.pvp.net",
            "dnd",
            "league_of_legends",
            json!({
                "gameStatus": "inGame",
                "gameQueueType": "RANKED_SOLO_5x5",
                "queueId": "420",
                "championId": "121",
                "timeStamp": "1793841299452",
                "rankedLeagueTier": "DIAMOND",
            }),
        ))
        .unwrap();
        assert_eq!(friend.id, "a@br1.pvp.net");
        assert_eq!(friend.puuid, "puuid-a@br1.pvp.net");
        assert_eq!(friend.display_name(), "nome-a@br1.pvp.net#BR1");
        assert_eq!(friend.profile_icon_id, Some(4977));
        assert_eq!(friend.availability, Availability::Busy);
        assert_eq!(
            friend.activity,
            Some(Activity::League(LeagueActivity {
                status: LeagueStatus::InGame,
                queue_id: Some(420),
                champion_id: Some(121),
                since: Some(1_793_841_299_452),
            }))
        );
    }

    #[test]
    fn status_do_lol() {
        for (status, expected) in [
            ("outOfGame", LeagueStatus::OutOfGame),
            ("hosting_RANKED_SOLO_5x5", LeagueStatus::InLobby),
            ("hosting_KIWI", LeagueStatus::InLobby),
            ("inQueue", LeagueStatus::InQueue),
            ("championSelect", LeagueStatus::ChampionSelect),
            ("inGame", LeagueStatus::InGame),
            ("spectating", LeagueStatus::Spectating),
            ("algoNovo", LeagueStatus::Unknown),
            ("", LeagueStatus::Unknown),
        ] {
            assert_eq!(LeagueStatus::from_lcu(status), expected, "{status}");
        }
        // Números vazios (fora de partida) viram `null`.
        let friend = Friend::from_json(&lcu(
            "b",
            "chat",
            "league_of_legends",
            json!({"gameStatus": "outOfGame", "queueId": "", "championId": "", "timeStamp": ""}),
        ))
        .unwrap();
        assert_eq!(
            friend.activity,
            Some(Activity::League(LeagueActivity {
                status: LeagueStatus::OutOfGame,
                queue_id: None,
                champion_id: None,
                since: None,
            }))
        );
    }

    #[test]
    fn outro_jogo_offline_e_celular() {
        let valorant = Friend::from_json(&lcu("v", "dnd", "valorant", json!({}))).unwrap();
        assert_eq!(
            valorant.activity,
            Some(Activity::OtherGame {
                product_name: "VALORANT".into()
            })
        );
        let offline = Friend::from_json(&lcu("o", "offline", "", json!({}))).unwrap();
        assert_eq!(offline.availability, Availability::Offline);
        assert_eq!(offline.activity, None);
        // O app de celular diz `league_of_legends`, mas não há o que mostrar.
        let mobile =
            Friend::from_json(&lcu("m", "mobile", "league_of_legends", json!({}))).unwrap();
        assert_eq!(mobile.availability, Availability::Mobile);
        assert_eq!(mobile.activity, None);
        // Online no LoL sem o objeto `lol` ainda.
        let sem_lol =
            Friend::from_json(&lcu("s", "chat", "league_of_legends", json!(null))).unwrap();
        assert!(matches!(
            sem_lol.activity,
            Some(Activity::League(LeagueActivity {
                status: LeagueStatus::Unknown,
                ..
            }))
        ));
    }

    #[test]
    fn campos_faltando_ou_estranhos() {
        assert_eq!(Friend::from_json(&json!(null)), None);
        assert_eq!(Friend::from_json(&json!([])), None);
        assert_eq!(Friend::from_json(&json!({})), None, "sem id");
        let minimo = Friend::from_json(&json!({"pid": "x", "icon": -1, "gameName": null})).unwrap();
        assert_eq!(minimo.id, "x");
        assert_eq!(minimo.profile_icon_id, None);
        assert_eq!(minimo.availability, Availability::Offline);
        assert_eq!(minimo.display_name(), "");
        let novo = Friend::from_json(&json!({"id": "y", "availability": "invisivel"})).unwrap();
        assert_eq!(novo.availability, Availability::Unknown);
    }

    #[test]
    fn lista_ordenada_por_disponibilidade_e_nome() {
        let lista = json!([
            lcu("1", "offline", "", json!({})),
            lcu("2", "away", "league_of_legends", json!({"gameStatus": "outOfGame"})),
            {"sem": "id"},
            lcu("3", "chat", "league_of_legends", json!({"gameStatus": "hosting_NORMAL"})),
            lcu("4", "mobile", "league_of_legends", json!({})),
            lcu("5", "dnd", "valorant", json!({})),
            lcu("0", "chat", "league_of_legends", json!({"gameStatus": "outOfGame"})),
        ]);
        let ids: Vec<String> = Friend::list_from_json(&lista)
            .into_iter()
            .map(|f| f.id)
            .collect();
        assert_eq!(ids, ["0", "3", "5", "2", "4", "1"]);
        assert!(Friend::list_from_json(&json!({"erro": 1})).is_empty());
    }

    #[test]
    fn id_do_amigo_no_uri() {
        assert_eq!(
            friend_id_from_uri("/lol-chat/v1/friends/abc%40br1.pvp.net").as_deref(),
            Some("abc@br1.pvp.net")
        );
        assert_eq!(
            friend_id_from_uri("/lol-chat/v1/friends/fake-pid-0016").as_deref(),
            Some("fake-pid-0016")
        );
        assert_eq!(friend_id_from_uri("/lol-chat/v1/friends"), None);
        assert_eq!(friend_id_from_uri("/lol-chat/v1/friends/"), None);
        assert_eq!(friend_id_from_uri("/lol-chat/v1/friends/a/b"), None);
        assert_eq!(friend_id_from_uri("/lol-chat/v1/friends/a%4"), None);
        assert_eq!(friend_id_from_uri("/lol-chat/v1/friend-counts"), None);
    }

    #[test]
    fn formato_para_a_ui() {
        let friend = Friend::from_json(&lcu(
            "a",
            "chat",
            "league_of_legends",
            json!({"gameStatus": "inQueue", "queueId": "420", "timeStamp": "1000"}),
        ))
        .unwrap();
        assert_eq!(
            serde_json::to_value(&friend).unwrap(),
            json!({
                "id": "a",
                "puuid": "puuid-a",
                "gameName": "nome-a",
                "tagLine": "BR1",
                "name": "",
                "profileIconId": 4977,
                "availability": "online",
                "statusMessage": "",
                "activity": {
                    "type": "league",
                    "status": "inQueue",
                    "queueId": 420,
                    "championId": null,
                    "since": 1000,
                },
            })
        );
        let valorant = Friend::from_json(&lcu("v", "dnd", "valorant", json!({}))).unwrap();
        assert_eq!(
            serde_json::to_value(valorant.activity).unwrap(),
            json!({"type": "otherGame", "productName": "VALORANT"})
        );
        for availability in Availability::ALL {
            assert_eq!(
                Availability::from_lcu_name(availability.lcu_name()),
                availability
            );
            assert_eq!(
                serde_json::to_value(availability).unwrap(),
                json!(availability.as_str())
            );
        }
        // O JSON do `/ws` volta igual (os testes do mock-lcu leem as
        // mensagens do agente).
        let back: Friend = serde_json::from_value(serde_json::to_value(&friend).unwrap()).unwrap();
        assert_eq!(back, friend);
    }
}
