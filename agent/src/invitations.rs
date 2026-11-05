//! Convites de lobby (#32): os recebidos, com aceitar e recusar, e o envio de
//! convites para o lobby atual.
//!
//! Na LCU:
//!
//! - `GET /lol-lobby/v2/received-invitations` devolve a lista de convites
//!   recebidos (vazia, ou `404` antes do login). Cada mudança chega pelo WAMP
//!   no mesmo URI, sempre com a lista inteira.
//! - `POST /lol-lobby/v2/received-invitations/{id}/accept` aceita: o jogador
//!   entra no lobby de quem convidou, e o lobby novo chega pelo WAMP em
//!   `/lol-lobby/v2/lobby`.
//! - `POST /lol-lobby/v2/received-invitations/{id}/decline` recusa.
//! - `POST /lol-lobby/v2/lobby/invitations` com
//!   `[{"toSummonerId": 1, "toPuuid": "..."}]` convida para o lobby atual. Os
//!   convites enviados aparecem no próprio lobby, em
//!   [`Lobby::invitations`](crate::lobby::Lobby::invitations).
//!
//! A gravação 16.22 não tem nenhum convite recebido; o formato aqui segue o
//! `LolLobbyReceivedInvitationDto` da LCU, e só os campos usados são lidos.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::lobby::Queue;
use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) dos convites recebidos.
pub const RECEIVED_INVITATIONS_PATH: &str = "/lol-lobby/v2/received-invitations";

/// Endpoint para convidar jogadores para o lobby atual.
pub const LOBBY_INVITATIONS_PATH: &str = "/lol-lobby/v2/lobby/invitations";

/// Um convite recebido que ainda espera resposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedInvitation {
    /// Id do convite, usado em `acceptInvitation` e `declineInvitation`.
    pub invitation_id: String,
    /// Quem convidou. A UI cruza com a lista de amigos para mostrar o nome.
    // Cabe num `number` do JavaScript: os ids da Riot são bem menores que 2^53.
    #[ts(type = "number")]
    pub from_summoner_id: u64,
    /// Nome de quem convidou como a LCU manda. Com o Riot ID pode vir vazio;
    /// prefira o nome da lista de amigos.
    pub from_summoner_name: String,
    /// `queueId` da Riot do lobby para o qual o convite é.
    pub queue_id: u32,
    /// A fila do convite, ou `null` quando não é uma das [`Queue`].
    pub queue: Option<Queue>,
    /// Se a LCU deixa aceitar agora. É `false`, por exemplo, quando o
    /// jogador está na fila ou em partida.
    pub can_accept: bool,
}

impl ReceivedInvitation {
    /// Lê a lista de convites do JSON da LCU (resposta do `GET` ou `data` de
    /// um evento). Fica só com os convites pendentes: os aceitos, recusados
    /// ou cancelados não pedem nada da UI. `None` se não for uma lista.
    pub fn list_from_json(value: &Value) -> Option<Vec<ReceivedInvitation>> {
        let raw = Vec::<RawReceivedInvitation>::deserialize(value).ok()?;
        Some(
            raw.into_iter()
                .filter(|i| i.state == "Pending" && !i.invitation_id.is_empty())
                .map(|i| ReceivedInvitation {
                    invitation_id: i.invitation_id,
                    from_summoner_id: i.from_summoner_id,
                    from_summoner_name: i.from_summoner_name,
                    queue_id: i.game_config.queue_id,
                    queue: Queue::from_id(i.game_config.queue_id),
                    can_accept: i.can_accept_invitation,
                })
                .collect(),
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawReceivedInvitation {
    #[serde(default)]
    invitation_id: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    from_summoner_id: u64,
    #[serde(default)]
    from_summoner_name: String,
    #[serde(default)]
    can_accept_invitation: bool,
    #[serde(default)]
    game_config: RawInvitationGameConfig,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInvitationGameConfig {
    #[serde(default)]
    queue_id: u32,
}

/// Quem convidar para o lobby: o `summonerId` e o `puuid` de um amigo, como a
/// lista de amigos traz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Invitee {
    #[ts(type = "number")]
    pub summoner_id: u64,
    pub puuid: String,
}

/// Por que um comando de convite não vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidInvitation {
    /// O id do convite está vazio ou tem caracteres que não aparecem nos ids
    /// da LCU (ele vai no caminho da URL).
    BadId,
    /// Nenhum jogador para convidar.
    NoInvitees,
    /// Um convidado sem `summonerId` nem `puuid`.
    EmptyInvitee,
}

impl std::fmt::Display for InvalidInvitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InvalidInvitation::BadId => "id de convite inválido",
            InvalidInvitation::NoInvitees => "ninguém para convidar",
            InvalidInvitation::EmptyInvitee => "convidado sem `summonerId` nem `puuid`",
        })
    }
}

impl std::error::Error for InvalidInvitation {}

/// Confere o id de um convite antes de pôr no caminho da URL: só letras,
/// números, `-`, `_` e `.`, como os ids que a LCU gera. Assim um id vindo da
/// UI não muda o endpoint chamado.
pub fn check_invitation_id(id: &str) -> Result<(), InvalidInvitation> {
    let valid = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && id != "."
        && id != "..";
    if valid {
        Ok(())
    } else {
        Err(InvalidInvitation::BadId)
    }
}

/// Confere a lista de convidados de [`invite`].
pub fn check_invitees(invitees: &[Invitee]) -> Result<(), InvalidInvitation> {
    if invitees.is_empty() {
        return Err(InvalidInvitation::NoInvitees);
    }
    if invitees
        .iter()
        .any(|i| i.summoner_id == 0 && i.puuid.is_empty())
    {
        return Err(InvalidInvitation::EmptyInvitee);
    }
    Ok(())
}

/// Erro de [`accept`], [`decline`] e [`invite`].
#[derive(Debug, thiserror::Error)]
pub enum InvitationError {
    /// O comando não vale; nada foi mandado para a LCU.
    #[error("{0}")]
    Invalid(InvalidInvitation),
    /// A LCU recusou (convite que já expirou, fora de lobby, lobby cheio...)
    /// ou não respondeu.
    #[error(transparent)]
    Lcu(#[from] LcuError),
}

/// Busca os convites recebidos pendentes. `404` (antes do login) volta como
/// lista vazia.
pub async fn received_invitations(lcu: &LcuClient) -> Result<Vec<ReceivedInvitation>, LcuError> {
    match lcu.get::<Value>(RECEIVED_INVITATIONS_PATH).await {
        Ok(body) => Ok(ReceivedInvitation::list_from_json(&body).unwrap_or_default()),
        Err(error) if error.status() == Some(404) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Aceita o convite `id`
/// (`POST /lol-lobby/v2/received-invitations/{id}/accept`). O lobby de quem
/// convidou chega pelo WAMP.
pub async fn accept(lcu: &LcuClient, id: &str) -> Result<(), InvitationError> {
    answer(lcu, id, "accept").await
}

/// Recusa o convite `id`
/// (`POST /lol-lobby/v2/received-invitations/{id}/decline`).
pub async fn decline(lcu: &LcuClient, id: &str) -> Result<(), InvitationError> {
    answer(lcu, id, "decline").await
}

async fn answer(lcu: &LcuClient, id: &str, action: &str) -> Result<(), InvitationError> {
    check_invitation_id(id).map_err(InvitationError::Invalid)?;
    let path = format!("{RECEIVED_INVITATIONS_PATH}/{id}/{action}");
    lcu.post::<_, Option<Value>>(&path, &json!({})).await?;
    Ok(())
}

/// Convida jogadores para o lobby atual (`POST /lol-lobby/v2/lobby/invitations`).
/// Os convites aparecem no lobby, que chega pelo WAMP.
pub async fn invite(lcu: &LcuClient, invitees: &[Invitee]) -> Result<(), InvitationError> {
    check_invitees(invitees).map_err(InvitationError::Invalid)?;
    lcu.post::<_, Option<Value>>(LOBBY_INVITATIONS_PATH, &invite_body(invitees))
        .await?;
    Ok(())
}

/// Corpo do `POST` de [`invite`]. Manda o `summonerId` e o `puuid`: as versões
/// recentes da LCU têm os dois campos no convite (`toSummonerId` e
/// `toPuuid`), e um vazio fica de fora.
fn invite_body(invitees: &[Invitee]) -> Value {
    Value::Array(
        invitees
            .iter()
            .map(|i| {
                let mut entry = serde_json::Map::new();
                if i.summoner_id != 0 {
                    entry.insert("toSummonerId".into(), json!(i.summoner_id));
                }
                if !i.puuid.is_empty() {
                    entry.insert("toPuuid".into(), json!(i.puuid));
                }
                Value::Object(entry)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invitation(id: &str, state: &str) -> Value {
        json!({
            "canAcceptInvitation": true,
            "fromSummonerId": 1003,
            "fromSummonerName": "Amigo",
            "gameConfig": {"gameMode": "CLASSIC", "inviteGameType": "RANKED_GAME_SOLO",
                           "mapId": 11, "queueId": 420},
            "invitationId": id,
            "invitationType": "lobby",
            "restrictions": [],
            "state": state,
            "timestamp": "1793841180340",
        })
    }

    #[test]
    fn le_os_convites_pendentes() {
        let list = ReceivedInvitation::list_from_json(&json!([
            invitation("a", "Pending"),
            invitation("b", "Declined"),
            invitation("c", "Accepted"),
            invitation("", "Pending"),
        ]))
        .unwrap();
        assert_eq!(
            list,
            [ReceivedInvitation {
                invitation_id: "a".into(),
                from_summoner_id: 1003,
                from_summoner_name: "Amigo".into(),
                queue_id: 420,
                queue: Some(Queue::RankedSolo),
                can_accept: true,
            }]
        );
        assert_eq!(ReceivedInvitation::list_from_json(&json!([])), Some(vec![]));
        assert_eq!(ReceivedInvitation::list_from_json(&json!(null)), None);
        assert_eq!(
            ReceivedInvitation::list_from_json(&json!({"errorCode": "RPC_ERROR"})),
            None
        );
        // Campos que faltam não quebram a leitura.
        let list =
            ReceivedInvitation::list_from_json(&json!([{"invitationId": "x", "state": "Pending"}]))
                .unwrap();
        assert_eq!(list[0].queue, None);
        assert!(!list[0].can_accept);
    }

    #[test]
    fn formato_para_a_ui() {
        let list =
            ReceivedInvitation::list_from_json(&json!([invitation("a", "Pending")])).unwrap();
        assert_eq!(
            serde_json::to_value(&list[0]).unwrap(),
            json!({
                "invitationId": "a",
                "fromSummonerId": 1003,
                "fromSummonerName": "Amigo",
                "queueId": 420,
                "queue": "rankedSolo",
                "canAccept": true,
            })
        );
    }

    #[test]
    fn ids_de_convite() {
        for id in ["a", "3f2c1d7e-6a1b-4c1e-9d55-0d0b8f3e2a11", "abc_DEF.123"] {
            assert_eq!(check_invitation_id(id), Ok(()), "{id}");
        }
        for id in ["", ".", "..", "a/b", "../lobby", "a?b", "a b", "a%2Fb", "ç"] {
            assert_eq!(
                check_invitation_id(id),
                Err(InvalidInvitation::BadId),
                "{id}"
            );
        }
        assert_eq!(
            check_invitation_id(&"a".repeat(129)),
            Err(InvalidInvitation::BadId)
        );
    }

    #[test]
    fn corpo_do_convite() {
        let invitees = [
            Invitee {
                summoner_id: 1003,
                puuid: "p1".into(),
            },
            Invitee {
                summoner_id: 0,
                puuid: "p2".into(),
            },
        ];
        assert_eq!(check_invitees(&invitees), Ok(()));
        assert_eq!(
            invite_body(&invitees),
            json!([{"toSummonerId": 1003, "toPuuid": "p1"}, {"toPuuid": "p2"}])
        );
        assert_eq!(check_invitees(&[]), Err(InvalidInvitation::NoInvitees));
        assert_eq!(
            check_invitees(&[Invitee {
                summoner_id: 0,
                puuid: String::new()
            }]),
            Err(InvalidInvitation::EmptyInvitee)
        );
    }
}
