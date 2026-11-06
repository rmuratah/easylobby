//! Chat da seleção de campeões (#27): a sala do time durante a seleção.
//!
//! A LCU trata a sala como mais uma conversa em `/lol-chat/v1/conversations`,
//! com `type: "championSelect"`. Ela abre (`Create`) logo depois da sessão da
//! seleção e fecha (`Delete`) quando a seleção acaba, por partida abrindo ou
//! por dodge. O [`chat`](crate::chat) do #31 descarta essas salas; este módulo
//! lê o que interessa delas:
//!
//! - o `id` da sala vem do URI dos eventos (`.../conversations/{id}`), como no
//!   chat 1:1. No REST, é o `id` da conversa;
//! - as mensagens chegam como no chat 1:1 (`.../{id}/messages/{msg}`), e
//!   `POST .../{id}/messages {"body", "type": "chat"}` manda uma;
//! - quem mandou não vem pelo nome. Na ranqueada a LCU esconde até os nomes
//!   do próprio time: a mensagem traz `fromObfuscatedPuuid`, o mesmo
//!   `obfuscatedPuuid` do jogador na sessão da seleção. Quando o nome está
//!   visível (o próprio jogador, normais), vem `fromPuuid`, o `puuid` da
//!   sessão. O agente cruza os dois com o `myTeam` e manda a `cellId` de quem
//!   escreveu; a UI acha o jogador (cargo, campeão, nome quando visível) pela
//!   célula. Na troca de ordem de escolha os jogadores mudam de célula (a
//!   gravação 16.22 tem uma), então o agente refaz o cruzamento a cada
//!   mudança da seleção;
//! - os avisos de entrada e saída da sala vêm com `type: "system"` e ficam de
//!   fora, como no chat 1:1.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::champ_select::ChampSelect;
use crate::chat::{self, SendMessageError, CONVERSATIONS_PATH, MAX_MESSAGES};
use crate::rest::{LcuClient, LcuError};

/// `type` da LCU para a sala da seleção de campeões.
const ROOM_TYPE: &str = "championSelect";

/// `type` das mensagens escritas por jogadores.
const CHAT_TYPE: &str = "chat";

/// A sala de chat do time na seleção de campeões.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectChat {
    /// Mensagens da sala, da mais velha para a mais nova (até
    /// [`MAX_MESSAGES`]).
    pub messages: Vec<ChampSelectChatMessage>,
    /// Identificador da sala na LCU, para mandar mensagens. A UI não precisa:
    /// o comando `sendChampSelectMessage` vai sempre para a sala aberta.
    #[serde(skip)]
    #[ts(skip)]
    pub id: String,
}

/// Uma mensagem da sala da seleção.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectChatMessage {
    /// Identificador da mensagem na LCU, único dentro da sala.
    pub id: String,
    pub body: String,
    /// `true` quando foi o jogador logado que mandou.
    pub from_me: bool,
    /// `cellId` de quem mandou, para achar o jogador em `myTeam` do
    /// `champSelect`. É a célula onde o jogador está agora: se ele trocar de
    /// lugar na ordem de escolha, a sala chega de novo com a célula nova.
    /// `null` quando o agente não conseguiu ligar a mensagem a um jogador do
    /// time.
    pub from_cell_id: Option<i32>,
    /// Quando foi mandada, em ISO 8601 UTC (`2026-11-05T01:13:41.755Z`).
    pub timestamp: String,
    /// `fromPuuid` e `fromObfuscatedPuuid` da LCU, para refazer
    /// `fromCellId` quando o time muda. Não vão para a UI.
    #[serde(skip)]
    #[ts(skip)]
    pub from_puuid: String,
    #[serde(skip)]
    #[ts(skip)]
    pub from_obfuscated_puuid: String,
}

/// Quem é quem na sala: o jogador logado e o time da seleção.
#[derive(Debug, Clone, Copy, Default)]
pub struct Senders<'a> {
    /// `puuid` do jogador logado (`/lol-summoner/v1/current-summoner`).
    pub my_puuid: Option<&'a str>,
    pub champ_select: Option<&'a ChampSelect>,
}

impl Senders<'_> {
    /// A célula do jogador com este `puuid` ou `obfuscatedPuuid`.
    fn cell_of(&self, puuid: &str, obfuscated_puuid: &str) -> Option<i32> {
        let team = &self.champ_select?.my_team;
        team.iter()
            .find(|p| {
                (!puuid.is_empty() && p.puuid == puuid)
                    || (!obfuscated_puuid.is_empty() && p.obfuscated_puuid == obfuscated_puuid)
            })
            .map(|p| p.cell_id)
    }

    fn local_cell(&self) -> Option<i32> {
        self.champ_select.map(|cs| cs.local_player_cell_id)
    }
}

/// Se o `data` de um evento ou um item de `GET /lol-chat/v1/conversations` é
/// a sala da seleção de campeões.
pub fn is_room(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some(ROOM_TYPE)
}

impl ChampSelectChatMessage {
    /// Lê uma mensagem da sala. `None` para avisos de sistema
    /// (`type: "system"`) e valores sem o formato esperado.
    pub fn from_json(value: &Value, senders: &Senders) -> Option<Self> {
        let raw = LcuMessage::deserialize(value).ok()?;
        if raw.kind != CHAT_TYPE || raw.id.is_empty() {
            return None;
        }
        let mut message = ChampSelectChatMessage {
            id: raw.id,
            body: raw.body,
            from_me: false,
            from_cell_id: None,
            timestamp: raw.timestamp,
            from_puuid: raw.from_puuid,
            from_obfuscated_puuid: raw.from_obfuscated_puuid,
        };
        message.resolve(senders);
        Some(message)
    }

    /// Refaz `fromCellId` e `fromMe` com o time de `senders`. Devolve se algo
    /// mudou.
    pub fn resolve(&mut self, senders: &Senders) -> bool {
        let from_cell_id = senders.cell_of(&self.from_puuid, &self.from_obfuscated_puuid);
        let is_my_puuid = senders
            .my_puuid
            .is_some_and(|me| !me.is_empty() && me == self.from_puuid);
        let from_me =
            is_my_puuid || (from_cell_id.is_some() && from_cell_id == senders.local_cell());
        let changed = (from_cell_id, from_me) != (self.from_cell_id, self.from_me);
        self.from_cell_id = from_cell_id;
        self.from_me = from_me;
        changed
    }

    /// Lê uma lista de mensagens, pulando as que
    /// [`ChampSelectChatMessage::from_json`] recusa. Aceita também uma
    /// mensagem só (o `data` de `.../messages/{msg}`).
    pub fn list_from_json(value: &Value, senders: &Senders) -> Vec<Self> {
        match value.as_array() {
            Some(items) => items
                .iter()
                .filter_map(|m| Self::from_json(m, senders))
                .collect(),
            None => Self::from_json(value, senders).into_iter().collect(),
        }
    }
}

impl ChampSelectChat {
    /// Sala vazia com o `id` da LCU.
    pub fn new(id: impl Into<String>) -> Self {
        ChampSelectChat {
            messages: Vec::new(),
            id: id.into(),
        }
    }

    /// Refaz quem mandou cada mensagem com o time de `senders` (veja
    /// [`ChampSelectChatMessage::resolve`]). Devolve se algo mudou.
    pub fn resolve_senders(&mut self, senders: &Senders) -> bool {
        let mut changed = false;
        for message in &mut self.messages {
            changed |= message.resolve(senders);
        }
        changed
    }

    /// Junta `messages` ao histórico: troca as que já existiam (mesmo `id`),
    /// acrescenta as novas, ordena por horário e corta em [`MAX_MESSAGES`].
    /// Devolve se algo mudou.
    pub fn merge_messages(
        &mut self,
        messages: impl IntoIterator<Item = ChampSelectChatMessage>,
    ) -> bool {
        let mut changed = false;
        for message in messages {
            match self.messages.iter_mut().find(|m| m.id == message.id) {
                Some(existing) if *existing == message => {}
                Some(existing) => {
                    *existing = message;
                    changed = true;
                }
                None => {
                    self.messages.push(message);
                    changed = true;
                }
            }
        }
        if changed {
            // Estável, e o horário da LCU tem sempre o mesmo formato: a ordem
            // do texto é a ordem do tempo (como no chat 1:1).
            self.messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            if self.messages.len() > MAX_MESSAGES {
                let excess = self.messages.len() - MAX_MESSAGES;
                self.messages.drain(..excess);
            }
        }
        changed
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
    from_obfuscated_puuid: String,
    #[serde(default)]
    timestamp: String,
}

/// A sala aberta agora e as mensagens dela, cruas, pelo REST
/// (`GET /lol-chat/v1/conversations` e `.../{id}/messages`). `None` fora da
/// seleção. As mensagens ficam em JSON para serem lidas com o time da
/// seleção mais novo, na hora de entrar no estado.
pub async fn current_room(lcu: &LcuClient) -> Result<Option<(String, Value)>, LcuError> {
    let list: Value = lcu.get(CONVERSATIONS_PATH).await?;
    let Some(id) = list
        .as_array()
        .and_then(|items| items.iter().find(|c| is_room(c)))
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

    /// Seleção com o jogador logado na célula 2 (nome visível) e um aliado
    /// escondido na célula 0, como na ranqueada da gravação 16.22.
    fn selecao() -> ChampSelect {
        let player = |cell: i32, puuid: &str, obfuscated: &str| {
            json!({
                "cellId": cell, "gameName": "", "tagLine": "", "assignedPosition": "middle",
                "puuid": puuid, "obfuscatedPuuid": obfuscated,
            })
        };
        ChampSelect::from_json(&json!({
            "id": "s", "localPlayerCellId": 2,
            "timer": {"phase": "BAN_PICK", "adjustedTimeLeftInPhase": 1000.0, "totalTimeInPhase": 30000.0},
            "myTeam": [player(0, "", "aliado-escondido"), player(2, EU, "")],
            "theirTeam": [], "actions": [],
        }))
        .unwrap()
    }

    fn mensagem(id: &str, puuid: &str, obfuscated: &str, kind: &str, t: &str) -> Value {
        json!({
            "body": format!("oi {id}"), "fromId": "0", "fromObfuscatedPuuid": obfuscated,
            "fromObfuscatedSummonerId": 0, "fromPid": "", "fromPuuid": puuid,
            "fromSummonerId": 0, "id": id, "isHistorical": false, "timestamp": t, "type": kind,
        })
    }

    const T: &str = "2026-11-05T01:14:46.516Z";

    #[test]
    fn reconhece_a_sala() {
        assert!(is_room(&json!({"type": "championSelect"})));
        assert!(!is_room(&json!({"type": "chat"})));
        assert!(!is_room(&Value::Null));
    }

    #[test]
    fn liga_a_mensagem_ao_jogador() {
        let cs = selecao();
        let senders = Senders {
            my_puuid: Some(EU),
            champ_select: Some(&cs),
        };
        let aliado = ChampSelectChatMessage::from_json(
            &mensagem("1", "", "aliado-escondido", "chat", T),
            &senders,
        )
        .unwrap();
        assert_eq!(aliado.from_cell_id, Some(0));
        assert!(!aliado.from_me);
        assert_eq!(aliado.body, "oi 1");
        assert_eq!(aliado.timestamp, T);

        let minha =
            ChampSelectChatMessage::from_json(&mensagem("2", EU, "", "chat", T), &senders).unwrap();
        assert_eq!(minha.from_cell_id, Some(2));
        assert!(minha.from_me);

        // Sem a seleção ainda: não dá para achar a célula, mas o `puuid`
        // basta para saber que é do jogador logado.
        let sem_selecao = Senders {
            my_puuid: Some(EU),
            champ_select: None,
        };
        let minha =
            ChampSelectChatMessage::from_json(&mensagem("3", EU, "", "chat", T), &sem_selecao)
                .unwrap();
        assert_eq!(minha.from_cell_id, None);
        assert!(minha.from_me);
        let desconhecida =
            ChampSelectChatMessage::from_json(&mensagem("4", "", "x", "chat", T), &senders)
                .unwrap();
        assert_eq!(desconhecida.from_cell_id, None);
        assert!(!desconhecida.from_me);
    }

    #[test]
    fn segue_o_jogador_que_troca_de_celula() {
        let cs = selecao();
        let senders = Senders {
            my_puuid: Some(EU),
            champ_select: Some(&cs),
        };
        let mut sala = ChampSelectChat::new("sala");
        sala.merge_messages(ChampSelectChatMessage::list_from_json(
            &json!([mensagem("1", "", "aliado-escondido", "chat", T)]),
            &senders,
        ));
        assert_eq!(sala.messages[0].from_cell_id, Some(0));
        // Nada mudou no time: nada a refazer.
        assert!(!sala.resolve_senders(&senders));

        // O aliado troca de lugar com o jogador logado.
        let mut trocada = cs.clone();
        trocada.my_team[0].cell_id = 2;
        trocada.my_team[1].cell_id = 0;
        trocada.local_player_cell_id = 0;
        let senders = Senders {
            my_puuid: Some(EU),
            champ_select: Some(&trocada),
        };
        assert!(sala.resolve_senders(&senders));
        assert_eq!(sala.messages[0].from_cell_id, Some(2));
        assert!(!sala.messages[0].from_me);
    }

    #[test]
    fn avisos_de_sistema_ficam_de_fora() {
        let senders = Senders::default();
        assert_eq!(
            ChampSelectChatMessage::from_json(&mensagem("1", "", "x", "system", T), &senders),
            None
        );
        let lista = json!([
            mensagem("1", "", "x", "system", T),
            mensagem("2", "", "x", "chat", T),
            {"lixo": true},
        ]);
        let lidas = ChampSelectChatMessage::list_from_json(&lista, &senders);
        assert_eq!(lidas.len(), 1);
        assert_eq!(lidas[0].id, "2");
        // Uma mensagem só, fora de lista.
        let uma =
            ChampSelectChatMessage::list_from_json(&mensagem("3", "", "x", "chat", T), &senders);
        assert_eq!(uma.len(), 1);
    }

    #[test]
    fn junta_o_historico_em_ordem_e_sem_repetir() {
        let senders = Senders::default();
        let m = |id: &str, t: &str| {
            ChampSelectChatMessage::from_json(&mensagem(id, "", "x", "chat", t), &senders).unwrap()
        };
        let mut sala = ChampSelectChat::new("sala");
        assert!(sala.merge_messages([m("b", "2026-11-05T01:00:02.000Z")]));
        assert!(sala.merge_messages([
            m("a", "2026-11-05T01:00:01.000Z"),
            m("b", "2026-11-05T01:00:02.000Z"),
        ]));
        assert!(!sala.merge_messages([m("a", "2026-11-05T01:00:01.000Z")]));
        let ids: Vec<&str> = sala.messages.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);

        let muitas = (0..MAX_MESSAGES + 3).map(|i| {
            m(
                &format!("n{i:04}"),
                &format!("2026-11-05T02:{:02}:{:02}.000Z", i / 60, i % 60),
            )
        });
        assert!(sala.merge_messages(muitas));
        assert_eq!(sala.messages.len(), MAX_MESSAGES);
        assert_eq!(sala.messages[0].id, "n0003");
    }

    #[test]
    fn o_id_da_sala_nao_vai_para_a_ui() {
        let sala = ChampSelectChat::new("sala@lol-champ-select.br1.pvp.net");
        assert_eq!(
            serde_json::to_value(&sala).unwrap(),
            json!({"messages": []})
        );
    }
}
