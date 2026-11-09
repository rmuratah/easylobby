//! Chat do grupo (#206): a sala do lobby, como a aba de chat do grupo no
//! client oficial.
//!
//! Como a sala da seleção de campeões (#27), a LCU trata o chat do lobby como
//! mais uma conversa em `/lol-chat/v1/conversations`, com `type:
//! "customGame"`. O [`chat`](crate::chat) do #31 descarta essas salas; este
//! módulo lê as do lobby:
//!
//! - a sala é reconhecida pelo `type` ou pelo `id`, que começa com o
//!   `multiUserChatId` do lobby (`<id>@<servidor>`). Os dois juntos cobrem o
//!   caso de a LCU trocar o nome do tipo;
//! - as mensagens chegam como no chat 1:1 (`.../{id}/messages/{msg}`), e
//!   `POST .../{id}/messages {"body", "type": "chat"}` manda uma;
//! - no lobby os nomes não são escondidos: a mensagem traz `fromPuuid`, e o
//!   agente preenche o Riot ID de quem mandou (o jogador logado é `fromMe`);
//! - os avisos de entrada e saída da sala (`type: "system"`, corpo
//!   `joined_room` ou `left_room`) viram as linhas "X entrou no saguão" e
//!   "X saiu do saguão", como no client oficial; os outros avisos ficam de
//!   fora;
//! - quem entra na sala é a UI oficial. Com ela fechada pelo `kill-ux`, a
//!   sala nunca aparece em `/lol-chat/v1/conversations`, então o agente entra
//!   sozinho ([`join_room`]) com o `multiUserChatPassword` e o `mucJwtDto` do
//!   lobby, como a UI oficial faz.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::chat::{self, SendMessageError, CONVERSATIONS_PATH, MAX_MESSAGES};
use crate::lobby::Lobby;
use crate::rest::{LcuClient, LcuError};

/// O jogador logado no chat (`id` = `<puuid>@<região>.pvp.net`).
const CHAT_ME_PATH: &str = "/lol-chat/v1/me";

/// Domínio padrão das salas de grupo (`<id>@sec.<região>.pvp.net`).
const ROOM_DOMAIN: &str = "sec";

/// `type` da LCU para a sala do lobby.
const ROOM_TYPE: &str = "customGame";

/// `type` das mensagens escritas por jogadores.
const CHAT_TYPE: &str = "chat";

/// `type` dos avisos da sala, e o corpo dos avisos de entrada e saída.
const SYSTEM_TYPE: &str = "system";
const JOINED_BODY: &str = "joined_room";
const LEFT_BODY: &str = "left_room";

/// A sala de chat do grupo no lobby.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LobbyChat {
    /// Mensagens da sala, da mais velha para a mais nova (até
    /// [`MAX_MESSAGES`]).
    pub messages: Vec<LobbyChatMessage>,
    /// Identificador da sala na LCU, para mandar mensagens. A UI não precisa:
    /// o comando `sendLobbyMessage` vai sempre para a sala aberta.
    #[serde(skip)]
    #[ts(skip)]
    pub id: String,
}

/// O que é uma linha do chat do grupo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LobbyChatMessageKind {
    /// Mensagem escrita por um jogador.
    Chat,
    /// Aviso de que o jogador entrou na sala ("X entrou no saguão"). `body`
    /// vem vazio.
    Joined,
    /// Aviso de que o jogador saiu da sala ("X saiu do saguão"). `body` vem
    /// vazio.
    Left,
}

/// Uma linha do chat do grupo: mensagem ou aviso de entrada e saída.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LobbyChatMessage {
    /// Identificador da mensagem na LCU, único dentro da sala.
    pub id: String,
    pub kind: LobbyChatMessageKind,
    pub body: String,
    /// `true` quando foi o jogador logado que mandou (ou entrou, ou saiu).
    pub from_me: bool,
    /// `puuid` de quem mandou. Vazio quando a LCU não manda.
    pub from_puuid: String,
    /// Riot ID de quem mandou (`gameName#tagLine`), que o agente preenche
    /// pelo jogador logado, pela lista de amigos ou pela LCU. Chega vazio
    /// primeiro e numa mensagem `lobbyChat` seguinte com o nome; vazio
    /// também se a LCU não respondeu.
    pub game_name: String,
    pub tag_line: String,
    /// Quando foi mandada, em ISO 8601 UTC (`2026-11-05T01:13:41.755Z`).
    pub timestamp: String,
}

/// Se o `data` de um evento ou um item de `GET /lol-chat/v1/conversations` é
/// a sala do lobby cujo `multiUserChatId` é `lobby_chat_id` (vazio quando o
/// lobby não mandou).
pub fn is_room(value: &Value, lobby_chat_id: &str) -> bool {
    if value.get("type").and_then(Value::as_str) == Some(ROOM_TYPE) {
        return true;
    }
    let id = value.get("id").and_then(Value::as_str).unwrap_or_default();
    !lobby_chat_id.is_empty() && id.split('@').next() == Some(lobby_chat_id)
}

/// Se a sala `room_id` é a do lobby cujo `multiUserChatId` é
/// `lobby_chat_id`: o começo do `id` da sala, antes do `@`, é o do lobby.
pub fn room_belongs_to(room_id: &str, lobby_chat_id: &str) -> bool {
    let room = room_id.split('@').next().unwrap_or_default();
    let lobby = lobby_chat_id.split('@').next().unwrap_or_default();
    !room.is_empty() && room.eq_ignore_ascii_case(lobby)
}

impl LobbyChatMessage {
    /// Lê uma mensagem da sala. `None` para os avisos de sistema que não
    /// são de entrada ou saída e para valores sem o formato esperado.
    pub fn from_json(value: &Value, my_puuid: Option<&str>) -> Option<Self> {
        let raw = LcuMessage::deserialize(value).ok()?;
        if raw.id.is_empty() {
            return None;
        }
        let (kind, body) = match (raw.kind.as_str(), raw.body.as_str()) {
            (CHAT_TYPE, _) => (LobbyChatMessageKind::Chat, raw.body),
            (SYSTEM_TYPE, JOINED_BODY) => (LobbyChatMessageKind::Joined, String::new()),
            (SYSTEM_TYPE, LEFT_BODY) => (LobbyChatMessageKind::Left, String::new()),
            _ => return None,
        };
        // Sem `fromPuuid`, o `fromId` (`<puuid>@<servidor>`) diz quem foi.
        let from_puuid = if raw.from_puuid.is_empty() {
            raw.from_id.split('@').next().unwrap_or_default().to_owned()
        } else {
            raw.from_puuid
        };
        let from_me = my_puuid.is_some_and(|me| !me.is_empty() && me == from_puuid);
        Some(LobbyChatMessage {
            id: raw.id,
            kind,
            body,
            from_me,
            from_puuid,
            game_name: String::new(),
            tag_line: String::new(),
            timestamp: raw.timestamp,
        })
    }

    /// Lê uma lista de mensagens, pulando as que
    /// [`LobbyChatMessage::from_json`] recusa. Aceita também uma mensagem só
    /// (o `data` de `.../messages/{msg}`).
    pub fn list_from_json(value: &Value, my_puuid: Option<&str>) -> Vec<Self> {
        match value.as_array() {
            Some(items) => items
                .iter()
                .filter_map(|m| Self::from_json(m, my_puuid))
                .collect(),
            None => Self::from_json(value, my_puuid).into_iter().collect(),
        }
    }
}

impl LobbyChat {
    /// Sala vazia com o `id` da LCU.
    pub fn new(id: impl Into<String>) -> Self {
        LobbyChat {
            messages: Vec::new(),
            id: id.into(),
        }
    }

    /// Junta `messages` ao histórico: troca as que já existiam (mesmo `id`),
    /// acrescenta as novas, ordena por horário e corta em [`MAX_MESSAGES`].
    /// Devolve se algo mudou.
    pub fn merge_messages(&mut self, messages: impl IntoIterator<Item = LobbyChatMessage>) -> bool {
        let mut changed = false;
        for mut message in messages {
            match self.messages.iter_mut().find(|m| m.id == message.id) {
                Some(existing) => {
                    // A LCU não manda o nome: o que o agente já achou fica.
                    if message.game_name.is_empty() {
                        message.game_name.clone_from(&existing.game_name);
                        message.tag_line.clone_from(&existing.tag_line);
                    }
                    if *existing != message {
                        *existing = message;
                        changed = true;
                    }
                }
                None => {
                    self.messages.push(message);
                    changed = true;
                }
            }
        }
        if changed {
            // O horário da LCU tem sempre o mesmo formato: a ordem do texto é
            // a ordem do tempo (como no chat 1:1).
            self.messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            if self.messages.len() > MAX_MESSAGES {
                let excess = self.messages.len() - MAX_MESSAGES;
                self.messages.drain(..excess);
            }
        }
        changed
    }

    /// Preenche o Riot ID das mensagens sem nome com o que `name` achar pelo
    /// `puuid`. Devolve se algo mudou.
    pub fn fill_names(&mut self, name: impl Fn(&str) -> Option<(String, String)>) -> bool {
        let mut changed = false;
        for message in &mut self.messages {
            if !message.game_name.is_empty() || message.from_puuid.is_empty() {
                continue;
            }
            if let Some((game_name, tag_line)) = name(&message.from_puuid) {
                if !game_name.is_empty() {
                    message.game_name = game_name;
                    message.tag_line = tag_line;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Os `puuid` de quem mandou mensagem e ainda está sem nome.
    pub fn unnamed_senders(&self) -> Vec<String> {
        let mut puuids: Vec<String> = Vec::new();
        for message in &self.messages {
            if message.game_name.is_empty()
                && !message.from_puuid.is_empty()
                && !puuids.contains(&message.from_puuid)
            {
                puuids.push(message.from_puuid.clone());
            }
        }
        puuids
    }
}

/// A mensagem como a LCU manda, só com os campos usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcuMessage {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    from_puuid: String,
    #[serde(default)]
    from_id: String,
    #[serde(default)]
    timestamp: String,
}

/// As mensagens da sala `id`, cruas (`GET .../{id}/messages`).
pub async fn messages(lcu: &LcuClient, id: &str) -> Result<Value, LcuError> {
    lcu.get::<Value>(&format!("{}/messages", chat::conversation_path(id)))
        .await
}

/// A sala do lobby aberta agora e as mensagens dela, cruas, pelo REST
/// (`GET /lol-chat/v1/conversations` e `.../{id}/messages`). `None` sem sala.
pub async fn current_room(
    lcu: &LcuClient,
    lobby_chat_id: &str,
) -> Result<Option<(String, Value)>, LcuError> {
    let list: Value = lcu.get(CONVERSATIONS_PATH).await?;
    let Some(id) = list
        .as_array()
        .and_then(|items| items.iter().find(|c| is_room(c, lobby_chat_id)))
        .and_then(|c| c.get("id")?.as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
    else {
        return Ok(None);
    };
    // Sem as mensagens, a sala abre vazia e o WAMP traz as próximas.
    let messages = lcu
        .get::<Value>(&format!("{}/messages", chat::conversation_path(&id)))
        .await
        .unwrap_or(Value::Null);
    Ok(Some((id, messages)))
}

/// `id` da sala do grupo de `lobby`: `<multiUserChatId>@<domínio>.<servidor>`.
/// O domínio e a região vêm do `mucJwtDto` (`domain`, `targetRegion`); sem
/// eles, o domínio é `sec` e o servidor é `fallback_host`, a parte depois do
/// `@` do `id` do jogador no chat (`br1.pvp.net`). `None` sem sala no lobby
/// ou sem servidor.
pub fn room_id(lobby: &Lobby, fallback_host: Option<&str>) -> Option<String> {
    if lobby.chat_id.is_empty() {
        return None;
    }
    // A LCU pode já mandar o `id` completo.
    if lobby.chat_id.contains('@') {
        return Some(lobby.chat_id.clone());
    }
    let field = |name: &str| {
        lobby
            .chat_muc_jwt
            .get(name)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
    };
    let domain = field("domain").unwrap_or(ROOM_DOMAIN);
    let host = match field("targetRegion") {
        Some(region) => format!("{}.pvp.net", region.to_lowercase()),
        None => fallback_host.filter(|h| !h.is_empty())?.to_owned(),
    };
    Some(format!("{}@{domain}.{host}", lobby.chat_id))
}

/// O corpo do `POST /lol-chat/v1/conversations` que entra na sala `id`.
fn join_body(lobby: &Lobby, id: &str) -> Value {
    let mut body = json!({ "id": id, "type": ROOM_TYPE });
    if !lobby.chat_password.is_empty() {
        body["password"] = json!(lobby.chat_password);
    }
    if !lobby.chat_muc_jwt.is_null() {
        body["mucJwtDto"] = lobby.chat_muc_jwt.clone();
    }
    body
}

/// Entra na sala do grupo de `lobby` (`POST /lol-chat/v1/conversations`),
/// como a UI oficial faz ao abrir o lobby. Devolve o `id` da sala e as
/// mensagens dela, cruas, como [`current_room`]. `None` quando o lobby não
/// tem sala.
pub async fn join_room(
    lcu: &LcuClient,
    lobby: &Lobby,
) -> Result<Option<(String, Value)>, LcuError> {
    let host = if lobby.chat_id.contains('@') || lobby.chat_muc_jwt.get("targetRegion").is_some() {
        None
    } else {
        lcu.get::<Value>(CHAT_ME_PATH).await.ok().and_then(|me| {
            let id = me.get("id")?.as_str()?;
            Some(id.split_once('@')?.1.to_owned())
        })
    };
    let Some(id) = room_id(lobby, host.as_deref()) else {
        return Ok(None);
    };
    let response: Option<Value> = lcu.post(CONVERSATIONS_PATH, &join_body(lobby, &id)).await?;
    // A LCU responde a conversa; o `id` dela é o que vale para mandar.
    let id = response
        .as_ref()
        .and_then(|c| c.get("id")?.as_str())
        .filter(|id| !id.is_empty())
        .map_or(id, str::to_owned);
    let messages = lcu
        .get::<Value>(&format!("{}/messages", chat::conversation_path(&id)))
        .await
        .unwrap_or(Value::Null);
    Ok(Some((id, messages)))
}

/// Manda `body` na sala `room_id`
/// (`POST /lol-chat/v1/conversations/{id}/messages`), depois de conferir com
/// [`chat::check_message`]. Devolve a mensagem que a LCU respondeu, crua,
/// quando ela responde uma; a mensagem também chega pelo WAMP.
pub async fn send_message(
    lcu: &LcuClient,
    room_id: &str,
    body: &str,
) -> Result<Option<Value>, SendMessageError> {
    chat::check_message(room_id, body)?;
    let response: Option<Value> = lcu
        .post(
            &format!("{}/messages", chat::conversation_path(room_id)),
            &json!({ "body": body, "type": CHAT_TYPE }),
        )
        .await?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EU: &str = "eu-puuid";

    fn mensagem(id: &str, puuid: &str, kind: &str, t: &str) -> Value {
        json!({
            "body": format!("texto {id}"), "fromId": "x", "fromPid": "x", "fromPuuid": puuid,
            "fromSummonerId": 0, "id": id, "isHistorical": false, "timestamp": t, "type": kind,
        })
    }

    #[test]
    fn reconhece_a_sala_pelo_tipo_ou_pelo_id_do_lobby() {
        assert!(is_room(
            &json!({"type": "customGame", "id": "a@sec.br1.pvp.net"}),
            ""
        ));
        assert!(is_room(
            &json!({"type": "outro", "id": "grupo-1@sec.br1.pvp.net"}),
            "grupo-1"
        ));
        assert!(!is_room(
            &json!({"type": "chat", "id": "amigo@br1.pvp.net"}),
            "grupo-1"
        ));
        assert!(!is_room(
            &json!({"type": "championSelect", "id": "s@lol-champ-select.br1.pvp.net"}),
            ""
        ));
    }

    #[test]
    fn marca_as_minhas_e_pula_avisos_de_sistema() {
        let t = "2026-11-07T20:00:00.000Z";
        let lista = json!([
            mensagem("1", "amigo", "chat", t),
            mensagem("2", EU, "chat", t),
            mensagem("3", "", "system", t),
        ]);
        let mensagens = LobbyChatMessage::list_from_json(&lista, Some(EU));
        assert_eq!(mensagens.len(), 2);
        assert!(!mensagens[0].from_me);
        assert_eq!(mensagens[0].from_puuid, "amigo");
        assert!(mensagens[1].from_me);
        let uma = LobbyChatMessage::list_from_json(&mensagem("4", "amigo", "chat", t), None);
        assert_eq!(uma.len(), 1);
    }

    #[test]
    fn le_os_avisos_de_entrada_e_saida() {
        let t = "2026-11-07T20:00:00.000Z";
        let entrou = json!({"id": "s1", "type": "system", "body": "joined_room", "fromId": "ana@br1.pvp.net", "fromPuuid": "", "timestamp": t});
        let m = LobbyChatMessage::from_json(&entrou, Some(EU)).unwrap();
        assert_eq!(m.kind, LobbyChatMessageKind::Joined);
        assert_eq!(m.body, "");
        // Sem `fromPuuid`, pelo `fromId`.
        assert_eq!(m.from_puuid, "ana");
        let saiu = mensagem("s2", EU, "system", t);
        assert!(LobbyChatMessage::from_json(&saiu, Some(EU)).is_none());
        let mut saiu = saiu;
        saiu["body"] = json!("left_room");
        let m = LobbyChatMessage::from_json(&saiu, Some(EU)).unwrap();
        assert_eq!(m.kind, LobbyChatMessageKind::Left);
        assert!(m.from_me);
    }

    #[test]
    fn o_nome_achado_fica_quando_a_mensagem_volta() {
        let t = "2026-11-07T20:00:00.000Z";
        let mut sala = LobbyChat::new("grupo@sec");
        let ler =
            || LobbyChatMessage::from_json(&mensagem("a", "ana", "chat", t), Some(EU)).unwrap();
        sala.merge_messages([ler()]);
        assert_eq!(sala.unnamed_senders(), ["ana"]);
        assert!(sala.fill_names(|p| (p == "ana").then(|| ("Ana".into(), "br1".into()))));
        // A releitura traz a mesma mensagem sem nome: nada muda.
        assert!(!sala.merge_messages([ler()]));
        assert_eq!(sala.messages[0].game_name, "Ana");
        assert!(sala.unnamed_senders().is_empty());
    }

    #[test]
    fn reconhece_a_sala_do_lobby_pelo_id() {
        assert!(room_belongs_to("grupo-1@sec.br1.pvp.net", "grupo-1"));
        assert!(room_belongs_to(
            "GRUPO-1@sec.br1.pvp.net",
            "grupo-1@sec.br1.pvp.net"
        ));
        assert!(!room_belongs_to("grupo-2@sec.br1.pvp.net", "grupo-1"));
        assert!(!room_belongs_to("", ""));
    }

    #[test]
    fn junta_o_historico_em_ordem_e_sem_repetir() {
        let mut sala = LobbyChat::new("grupo@sec");
        let ler = |id: &str, t: &str| {
            LobbyChatMessage::from_json(&mensagem(id, "amigo", "chat", t), Some(EU)).unwrap()
        };
        assert!(sala.merge_messages([
            ler("b", "2026-11-07T20:00:02.000Z"),
            ler("a", "2026-11-07T20:00:01.000Z")
        ]));
        assert!(!sala.merge_messages([ler("a", "2026-11-07T20:00:01.000Z")]));
        assert_eq!(
            sala.messages
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
    }

    fn lobby(chat_id: &str, muc: Value) -> Lobby {
        let mut lobby = Lobby::from_json(&json!({"gameConfig": {"queueId": 440}})).unwrap();
        lobby.chat_id = chat_id.into();
        lobby.chat_password = "senha".into();
        lobby.chat_muc_jwt = muc;
        lobby
    }

    #[test]
    fn monta_o_id_da_sala_do_grupo() {
        let muc = json!({"jwt": "x", "domain": "sec", "targetRegion": "BR1"});
        assert_eq!(
            room_id(&lobby("grupo-1", muc), None).as_deref(),
            Some("grupo-1@sec.br1.pvp.net")
        );
        // Sem o `mucJwtDto`, pelo servidor do jogador no chat.
        assert_eq!(
            room_id(&lobby("grupo-1", Value::Null), Some("br1.pvp.net")).as_deref(),
            Some("grupo-1@sec.br1.pvp.net")
        );
        assert_eq!(room_id(&lobby("grupo-1", Value::Null), None), None);
        assert_eq!(room_id(&lobby("", Value::Null), Some("br1.pvp.net")), None);
        assert_eq!(
            room_id(&lobby("g@sec.br1.pvp.net", Value::Null), None).as_deref(),
            Some("g@sec.br1.pvp.net")
        );
    }

    #[test]
    fn entra_com_a_senha_e_o_jwt_do_lobby() {
        let muc = json!({"jwt": "x", "targetRegion": "br1"});
        let corpo = join_body(&lobby("grupo-1", muc.clone()), "grupo-1@sec.br1.pvp.net");
        assert_eq!(
            corpo,
            json!({"id": "grupo-1@sec.br1.pvp.net", "type": "customGame", "password": "senha", "mucJwtDto": muc})
        );
    }

    #[test]
    fn o_id_da_sala_nao_vai_para_a_ui() {
        let json = serde_json::to_value(LobbyChat::new("grupo@sec")).unwrap();
        assert_eq!(json, json!({"messages": []}));
    }
}
