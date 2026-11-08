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
//! [`Lobby`] pelo `/ws` e pede as mudanças com os comandos `createLobby`,
//! `leaveLobby` e `setPositions`.
//!
//! Cargos (#18): nas filas com escolha de posição (normal alternada e
//! ranqueadas, [`Lobby::show_position_selector`]) cada jogador escolhe um
//! cargo primário e um secundário com
//! `PUT /lol-lobby/v2/lobby/members/localMember/position-preferences`. A
//! LCU responde sem corpo e manda o lobby novo pelo WAMP.
//!
//! Convites (#32): o lobby traz os convites que o grupo mandou em
//! `invitations` e, no `localMember`, se o jogador pode convidar
//! (`allowedInviteOthers`). Veja [`crate::invitations`].
//!
//! Jogo Dinâmico (#218): nas filas com slots
//! ([`Lobby::show_quick_play_slots`]) cada jogador escolhe duas posições e
//! um campeão para cada uma ainda no lobby. Veja [`crate::quick_play`].
//!
//! Lobby aberto ou fechado (#225): com o lobby aberto (`partyType` `open`),
//! amigos entram sem convite. O líder troca com
//! `PUT /lol-lobby/v2/lobby/partyType`, mandando `"open"` ou `"closed"`.
//!
//! O [`Lobby`] guarda só o que a tela de lobby usa. Os outros campos da
//! resposta são ignorados, então um campo novo num patch não quebra a
//! leitura.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::quick_play::PlayerSlot;
use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) do lobby.
pub const LOBBY_PATH: &str = "/lol-lobby/v2/lobby";

/// Endpoint dos cargos do jogador logado (#18).
pub const POSITION_PREFERENCES_PATH: &str =
    "/lol-lobby/v2/lobby/members/localMember/position-preferences";

/// Endpoint do lobby aberto ou fechado (#225).
pub const PARTY_TYPE_PATH: &str = "/lol-lobby/v2/lobby/partyType";

/// Um cargo (posição) no lobby. Na LCU são as strings em maiúsculas de
/// [`Position::lcu_name`]; para a UI vão em camelCase (`"top"`,
/// `"middle"`...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Position {
    /// Topo (`TOP`).
    Top,
    /// Selva (`JUNGLE`).
    Jungle,
    /// Meio (`MIDDLE`).
    Middle,
    /// Atirador (`BOTTOM`).
    Bottom,
    /// Suporte (`UTILITY`).
    Utility,
    /// Preenchimento, qualquer cargo (`FILL`).
    Fill,
    /// Nenhum cargo escolhido ainda (`UNSELECTED`). Também é o que a LCU
    /// manda fora das filas com escolha de posição.
    #[default]
    Unselected,
}

impl Position {
    /// Todos os cargos, na ordem em que a tela mostra.
    pub const ALL: [Position; 7] = [
        Position::Top,
        Position::Jungle,
        Position::Middle,
        Position::Bottom,
        Position::Utility,
        Position::Fill,
        Position::Unselected,
    ];

    /// Nome do cargo na LCU.
    pub const fn lcu_name(self) -> &'static str {
        match self {
            Position::Top => "TOP",
            Position::Jungle => "JUNGLE",
            Position::Middle => "MIDDLE",
            Position::Bottom => "BOTTOM",
            Position::Utility => "UTILITY",
            Position::Fill => "FILL",
            Position::Unselected => "UNSELECTED",
        }
    }

    /// O cargo de um nome da LCU. `null`, vazio ou desconhecido vira
    /// [`Position::Unselected`].
    pub fn from_lcu(name: Option<&str>) -> Position {
        name.and_then(|name| Position::ALL.into_iter().find(|p| p.lcu_name() == name))
            .unwrap_or_default()
    }
}

/// Por que um par de cargos não vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidPositions {
    /// O primário e o secundário são o mesmo cargo.
    Repeated,
    /// Com o primário em preenchimento, o secundário fica sem cargo.
    SecondWithFill,
    /// Secundário escolhido com o primário vazio.
    SecondWithoutFirst,
}

impl std::fmt::Display for InvalidPositions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InvalidPositions::Repeated => "a posição primária e a secundária são iguais",
            InvalidPositions::SecondWithFill => {
                "com o primário em preenchimento, o secundário fica `unselected`"
            }
            InvalidPositions::SecondWithoutFirst => {
                "escolha a posição primária antes da secundária"
            }
        })
    }
}

impl std::error::Error for InvalidPositions {}

/// Confere um par de cargos com as regras do client oficial: cargos
/// diferentes, e sem secundário quando o primário é preenchimento ou está
/// vazio.
pub fn check_positions(first: Position, second: Position) -> Result<(), InvalidPositions> {
    match (first, second) {
        (_, Position::Unselected) => Ok(()),
        (Position::Unselected, _) => Err(InvalidPositions::SecondWithoutFirst),
        (Position::Fill, _) => Err(InvalidPositions::SecondWithFill),
        (a, b) if a == b => Err(InvalidPositions::Repeated),
        _ => Ok(()),
    }
}

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
    /// Se a fila tem escolha de cargos (#18): `true` na normal alternada e
    /// nas ranqueadas, `false` no ARAM e no Jogo Dinâmico.
    pub show_position_selector: bool,
    /// Se a fila pede as posições e os campeões no lobby (#218): `true` no
    /// Jogo Dinâmico. Cada membro traz os seus em
    /// [`LobbyMember::player_slots`].
    pub show_quick_play_slots: bool,
    /// Posições que faltam na fila agora (as prioritárias do client
    /// oficial), na ordem da LCU. Vazia quando a LCU não diz.
    pub scarce_positions: Vec<Position>,
    /// Quem está no lobby, o jogador logado incluído.
    pub members: Vec<LobbyMember>,
    /// Se o jogador logado pode convidar (#32): o líder sempre pode, e os
    /// outros quando o líder libera.
    pub can_invite: bool,
    /// Convites que o grupo mandou e que não foram aceitos (#32): pendentes,
    /// recusados, expulsos ou que deram erro. Quem aceitou aparece em
    /// `members`.
    pub invitations: Vec<LobbyInvitation>,
    /// Se o lobby está aberto (#225): amigos entram sem convite. Vem do
    /// `partyType` da LCU (`open` ou `closed`); só o líder troca.
    pub is_open: bool,
    /// `multiUserChatId` da LCU: o começo do `id` da sala de chat do grupo
    /// (#206). A UI não precisa: o chat chega em `lobbyChat`.
    #[serde(skip)]
    #[ts(skip)]
    pub chat_id: String,
    /// `multiUserChatPassword` e `mucJwtDto` da LCU: o que o agente manda
    /// para entrar na sala do grupo quando a UI oficial está fechada (#206).
    #[serde(skip)]
    #[ts(skip)]
    pub chat_password: String,
    #[serde(skip)]
    #[ts(skip)]
    pub chat_muc_jwt: Value,
}

/// Um convite que o grupo mandou (#32).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LobbyInvitation {
    /// Quem foi convidado. A UI cruza com a lista de amigos para o nome.
    pub puuid: String,
    #[ts(type = "number")]
    pub summoner_id: u64,
    pub state: InvitationState,
}

/// Estado de um convite enviado, como a LCU manda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum InvitationState {
    /// Saindo; a LCU ainda não confirmou o envio.
    Requested,
    /// Enviado, esperando a resposta.
    Pending,
    /// Aceito. O [`Lobby`] não mostra esses: o jogador está em `members`.
    Accepted,
    /// Recusado.
    Declined,
    /// O jogador entrou e foi expulso do lobby.
    Kicked,
    /// Em espera (lobby cheio, por exemplo).
    OnHold,
    /// A LCU não conseguiu entregar.
    Error,
    /// Um estado que a LCU não mandava quando este código foi escrito.
    // Lido por [`InvitationState::from_lcu`]; o `#[serde(other)]` faria o
    // mesmo, mas o ts-rs não o entende.
    Unknown,
}

impl InvitationState {
    /// Todos os estados, com [`InvitationState::Unknown`] no fim.
    pub const ALL: [InvitationState; 8] = [
        InvitationState::Requested,
        InvitationState::Pending,
        InvitationState::Accepted,
        InvitationState::Declined,
        InvitationState::Kicked,
        InvitationState::OnHold,
        InvitationState::Error,
        InvitationState::Unknown,
    ];

    /// Nome do estado na LCU (e no `/ws`).
    pub const fn as_str(self) -> &'static str {
        match self {
            InvitationState::Requested => "Requested",
            InvitationState::Pending => "Pending",
            InvitationState::Accepted => "Accepted",
            InvitationState::Declined => "Declined",
            InvitationState::Kicked => "Kicked",
            InvitationState::OnHold => "OnHold",
            InvitationState::Error => "Error",
            InvitationState::Unknown => "Unknown",
        }
    }

    /// O estado de um nome da LCU; um nome desconhecido vira
    /// [`InvitationState::Unknown`].
    pub fn from_lcu(name: &str) -> InvitationState {
        InvitationState::ALL
            .into_iter()
            .find(|s| s.as_str() == name)
            .unwrap_or(InvitationState::Unknown)
    }
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
    /// Cargo primário (#18). `unselected` antes de escolher e nas filas sem
    /// escolha de cargos.
    pub first_position: Position,
    /// Cargo secundário (#18). `unselected` quando o primário é
    /// preenchimento.
    pub second_position: Position,
    /// Posições e campeões do Jogo Dinâmico (#218), na ordem dos slots.
    /// Vazia nas outras filas e antes de o jogador escolher.
    pub player_slots: Vec<PlayerSlot>,
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
                first_position: Position::from_lcu(m.first_position_preference.as_deref()),
                second_position: Position::from_lcu(m.second_position_preference.as_deref()),
                player_slots: PlayerSlot::list_from_json(&m.player_slots),
            })
            .collect::<Vec<LobbyMember>>();
        // A LCU lista em `invitations` até o próprio jogador logado, como
        // `Accepted`. Quem aceitou está em `members` (ou entrou e saiu), e
        // fica de fora.
        let invitations = raw
            .invitations
            .into_iter()
            .filter(|i| InvitationState::from_lcu(&i.state) != InvitationState::Accepted)
            .filter(|i| i.to_puuid.is_empty() || !members.iter().any(|m| m.puuid == i.to_puuid))
            .map(|i| LobbyInvitation {
                puuid: i.to_puuid,
                summoner_id: i.to_summoner_id,
                state: InvitationState::from_lcu(&i.state),
            })
            .collect();
        let local_member = raw.local_member.as_ref();
        Some(Lobby {
            queue_id: raw.game_config.queue_id,
            queue: Queue::from_id(raw.game_config.queue_id),
            max_lobby_size: raw.game_config.max_lobby_size,
            is_leader: local_member.is_some_and(|m| m.is_leader),
            can_start_activity: raw.can_start_activity,
            show_position_selector: raw.game_config.show_position_selector,
            show_quick_play_slots: raw.game_config.show_quick_play_slot_selection,
            scarce_positions: raw
                .scarce_positions
                .as_array()
                .map(|names| {
                    names
                        .iter()
                        .map(|name| Position::from_lcu(name.as_str()))
                        .filter(|&p| p != Position::Unselected)
                        .collect()
                })
                .unwrap_or_default(),
            members,
            can_invite: local_member.is_some_and(|m| m.is_leader || m.allowed_invite_others),
            invitations,
            is_open: raw.party_type == "open",
            chat_id: raw.multi_user_chat_id,
            chat_password: raw.multi_user_chat_password,
            chat_muc_jwt: raw.muc_jwt_dto,
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
    #[serde(default)]
    invitations: Vec<RawInvitation>,
    #[serde(default)]
    multi_user_chat_id: String,
    #[serde(default)]
    multi_user_chat_password: String,
    #[serde(default)]
    muc_jwt_dto: Value,
    #[serde(default)]
    scarce_positions: Value,
    #[serde(default)]
    party_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInvitation {
    #[serde(default)]
    state: String,
    #[serde(default)]
    to_puuid: String,
    #[serde(default)]
    to_summoner_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGameConfig {
    queue_id: u32,
    #[serde(default)]
    max_lobby_size: u32,
    #[serde(default)]
    show_position_selector: bool,
    #[serde(default)]
    show_quick_play_slot_selection: bool,
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
    #[serde(default)]
    allowed_invite_others: bool,
    #[serde(default)]
    first_position_preference: Option<String>,
    #[serde(default)]
    second_position_preference: Option<String>,
    #[serde(default)]
    player_slots: Value,
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
    create_lobby_by_id(lcu, queue.id()).await
}

/// Como [`create_lobby`], com o `queueId` da Riot de qualquer fila
/// disponível ([`crate::queues`], #146).
pub async fn create_lobby_by_id(lcu: &LcuClient, queue_id: u32) -> Result<Option<Lobby>, LcuError> {
    let body: Value = lcu
        .post(LOBBY_PATH, &json!({ "queueId": queue_id }))
        .await?;
    Ok(Lobby::from_json(&body))
}

/// Sai do lobby (`DELETE /lol-lobby/v2/lobby`).
pub async fn leave_lobby(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.delete::<Option<Value>>(LOBBY_PATH).await.map(|_| ())
}

/// Escolhe os cargos do jogador logado
/// (`PUT /lol-lobby/v2/lobby/members/localMember/position-preferences`),
/// depois de conferir o par com [`check_positions`]. A LCU responde sem corpo;
/// o lobby novo chega pelo WAMP (ou por [`current_lobby`]).
pub async fn set_positions(
    lcu: &LcuClient,
    first: Position,
    second: Position,
) -> Result<(), SetPositionsError> {
    check_positions(first, second).map_err(SetPositionsError::Invalid)?;
    let body = json!({
        "firstPreference": first.lcu_name(),
        "secondPreference": second.lcu_name(),
    });
    lcu.put::<_, Option<Value>>(POSITION_PREFERENCES_PATH, &body)
        .await
        .map_err(SetPositionsError::Lcu)?;
    Ok(())
}

/// Abre ou fecha o lobby (`PUT /lol-lobby/v2/lobby/partyType`, #225). Só o
/// líder pode; a LCU recusa os outros. O lobby novo chega pelo WAMP (ou por
/// [`current_lobby`]).
pub async fn set_lobby_open(lcu: &LcuClient, open: bool) -> Result<(), LcuError> {
    let party_type = if open { "open" } else { "closed" };
    lcu.put::<_, Option<Value>>(PARTY_TYPE_PATH, &json!(party_type))
        .await
        .map(|_| ())
}

/// Erro de [`set_positions`].
#[derive(Debug, thiserror::Error)]
pub enum SetPositionsError {
    /// O par de cargos não vale; nada foi mandado para a LCU.
    #[error("{0}")]
    Invalid(InvalidPositions),
    /// A LCU recusou (fora de lobby, fila sem cargos...) ou não respondeu.
    #[error(transparent)]
    Lcu(#[from] LcuError),
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
        assert!(lobby.show_position_selector);
        assert_eq!(lobby.members[0].first_position, Position::Unselected);
        assert!(lobby.can_invite);
        assert!(!lobby.is_open, "a gravação tem `partyType` `closed`");
        // O convite `Accepted` do próprio jogador não aparece.
        assert_eq!(lobby.invitations, []);

        // Mais adiante na gravação o jogador escolheu meio e selva.
        let escolhido = rest
            .iter()
            .filter(|r| r["path"] == LOBBY_PATH && r["status"] == 200)
            .find_map(|r| {
                let lobby = Lobby::from_json(&r["body"])?;
                (lobby.members[0].first_position != Position::Unselected).then_some(lobby)
            })
            .unwrap();
        assert_eq!(escolhido.members[0].first_position, Position::Middle);
        assert_eq!(escolhido.members[0].second_position, Position::Jungle);
    }

    #[test]
    fn nomes_dos_cargos() {
        for position in Position::ALL {
            assert_eq!(Position::from_lcu(Some(position.lcu_name())), position);
        }
        assert_eq!(Position::from_lcu(None), Position::Unselected);
        assert_eq!(Position::from_lcu(Some("")), Position::Unselected);
        assert_eq!(Position::from_lcu(Some("NONE")), Position::Unselected);
        assert_eq!(
            serde_json::to_value(Position::Utility).unwrap(),
            json!("utility")
        );
        assert_eq!(
            serde_json::from_value::<Position>(json!("middle")).unwrap(),
            Position::Middle
        );
        assert!(serde_json::from_value::<Position>(json!("MIDDLE")).is_err());
    }

    #[test]
    fn regras_dos_cargos() {
        use Position::*;
        assert_eq!(check_positions(Top, Jungle), Ok(()));
        assert_eq!(check_positions(Middle, Unselected), Ok(()));
        assert_eq!(check_positions(Fill, Unselected), Ok(()));
        assert_eq!(check_positions(Unselected, Unselected), Ok(()));
        assert_eq!(check_positions(Bottom, Fill), Ok(()));
        assert_eq!(check_positions(Top, Top), Err(InvalidPositions::Repeated));
        assert_eq!(
            check_positions(Fill, Top),
            Err(InvalidPositions::SecondWithFill)
        );
        assert_eq!(
            check_positions(Fill, Fill),
            Err(InvalidPositions::SecondWithFill)
        );
        assert_eq!(
            check_positions(Unselected, Utility),
            Err(InvalidPositions::SecondWithoutFirst)
        );
    }

    #[test]
    fn fila_desconhecida_e_json_que_nao_e_lobby() {
        let lobby = Lobby::from_json(&json!({"gameConfig": {"queueId": 1700}})).unwrap();
        assert_eq!(lobby.queue_id, 1700);
        assert_eq!(lobby.queue, None);
        assert!(!lobby.is_leader);
        assert!(lobby.members.is_empty());
        assert!(!lobby.can_invite);
        assert!(lobby.invitations.is_empty());
        assert!(!lobby.is_open);
        let aberto =
            Lobby::from_json(&json!({"gameConfig": {"queueId": 400}, "partyType": "open"}));
        assert!(aberto.unwrap().is_open);

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
            show_position_selector: false,
            show_quick_play_slots: false,
            scarce_positions: vec![Position::Jungle],
            members: vec![LobbyMember {
                puuid: "p".into(),
                summoner_id: 1,
                summoner_icon_id: 2,
                summoner_level: 3,
                is_leader: true,
                first_position: Position::Top,
                second_position: Position::Unselected,
                player_slots: Vec::new(),
            }],
            can_invite: true,
            invitations: vec![LobbyInvitation {
                puuid: "amigo".into(),
                summoner_id: 4,
                state: InvitationState::Pending,
            }],
            is_open: true,
            // Não vai para a UI.
            chat_id: "grupo".into(),
            chat_password: "senha".into(),
            chat_muc_jwt: json!({"jwt": "x"}),
        };
        assert_eq!(
            serde_json::to_value(&lobby).unwrap(),
            json!({
                "queueId": 450,
                "queue": "aram",
                "maxLobbySize": 5,
                "isLeader": true,
                "canStartActivity": true,
                "showPositionSelector": false,
                "showQuickPlaySlots": false,
                "scarcePositions": ["jungle"],
                "members": [{
                    "puuid": "p",
                    "summonerId": 1,
                    "summonerIconId": 2,
                    "summonerLevel": 3,
                    "isLeader": true,
                    "firstPosition": "top",
                    "secondPosition": "unselected",
                    "playerSlots": [],
                }],
                "canInvite": true,
                "invitations": [{"puuid": "amigo", "summonerId": 4, "state": "Pending"}],
                "isOpen": true,
            })
        );
    }

    #[test]
    fn le_os_slots_do_jogo_dinamico() {
        let lobby = Lobby::from_json(&json!({
            "gameConfig": {"queueId": 480, "showQuickPlaySlotSelection": true},
            "scarcePositions": ["JUNGLE", "BOTTOM", null, "NONE"],
            "members": [{"puuid": "eu", "playerSlots": [
                {"championId": 22, "positionPreference": "BOTTOM", "spell1": 4, "spell2": 7, "perks": ""},
                {"championId": 266, "positionPreference": "TOP", "spell1": 4, "spell2": 12, "perks": ""},
            ]}],
        }))
        .unwrap();
        assert_eq!(lobby.queue, Some(Queue::Swiftplay));
        assert!(lobby.show_quick_play_slots);
        assert_eq!(lobby.scarce_positions, [Position::Jungle, Position::Bottom]);
        let slots = &lobby.members[0].player_slots;
        assert_eq!(slots.len(), 2);
        assert_eq!(
            (slots[0].champion_id, slots[0].position),
            (22, Position::Bottom)
        );
        assert_eq!(
            (slots[1].champion_id, slots[1].position),
            (266, Position::Top)
        );
    }

    #[test]
    fn convites_enviados() {
        let invitation = |puuid: &str, state: &str| {
            json!({"invitationId": "", "invitationType": "lobby", "state": state,
                   "timestamp": "0", "toPuuid": puuid, "toSummonerId": 7, "toSummonerName": ""})
        };
        let lobby = Lobby::from_json(&json!({
            "gameConfig": {"queueId": 400},
            "localMember": {"puuid": "eu", "isLeader": false, "allowedInviteOthers": true},
            "members": [{"puuid": "eu"}, {"puuid": "entrou"}],
            "invitations": [
                invitation("eu", "Accepted"),
                invitation("entrou", "Accepted"),
                invitation("esperando", "Pending"),
                invitation("recusou", "Declined"),
                invitation("saiu", "Accepted"),
                invitation("novo", "EstadoDeUmPatchNovo"),
            ],
        }))
        .unwrap();
        assert!(lobby.can_invite, "o líder liberou os convites");
        let seen: Vec<(&str, InvitationState)> = lobby
            .invitations
            .iter()
            .map(|i| (i.puuid.as_str(), i.state))
            .collect();
        assert_eq!(
            seen,
            [
                ("esperando", InvitationState::Pending),
                ("recusou", InvitationState::Declined),
                ("novo", InvitationState::Unknown),
            ]
        );
        assert_eq!(lobby.invitations[0].summoner_id, 7);

        let sem_permissao = Lobby::from_json(&json!({
            "gameConfig": {"queueId": 400},
            "localMember": {"puuid": "eu", "isLeader": false, "allowedInviteOthers": false},
        }))
        .unwrap();
        assert!(!sem_permissao.can_invite);
    }
}
