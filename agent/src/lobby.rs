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
//! Posições em ordem (#230): no Rift Clássico (`gameMode` `JADE`, a fila
//! 4310 da gravação 16.22) o jogador não escolhe primário e secundário:
//! ordena as 5 posições de 1 a 5, ou marca "Qualquer posição" (`FILL`). O
//! lobby diz em [`Lobby::ordered_positions`] e cada membro traz a ordem em
//! [`LobbyMember::position_order`], lida de `firstPositionPreference` até
//! `fifthPositionPreference`. O agente salva com o mesmo `PUT` dos cargos,
//! mandando de `firstPreference` até `fifthPreference`; a gravação 16.22 não
//! tem esse `PUT`, então a rota e o corpo ainda precisam ser confirmados no
//! LoL real.
//!
//! Lobby aberto ou fechado (#225): com o lobby aberto (`partyType` `open`),
//! amigos entram sem convite. O líder troca com
//! `PUT /lol-lobby/v2/lobby/partyType`, mandando `"open"` ou `"closed"`.
//!
//! Equipes da Arena (#224): na Arena 3x6 o lobby tem até 18 jogadores em 6
//! equipes de 3, e cada jogador escolhe a equipe e a vaga dentro dela. A
//! LCU manda o tamanho em `gameConfig` (`numberOfTeamsInLobby` e
//! `maxTeamSize`) e a vaga de cada membro em `subteamIndex` e
//! `intraSubteamPosition`; o agente passa em [`Lobby::subteams`] e
//! [`LobbyMember::subteam`]. O jogador troca de vaga com
//! `PUT /lol-lobby/v2/lobby/subteamData`, mandando
//! `{"subteamIndex": N, "intraSubteamPosition": M}`. Os índices vão para a
//! UI como a LCU manda. No LoL real (#303) os índices começam em 1 e o
//! `gameConfig` traz o lobby inteiro como uma equipe só (`maxTeamSize` 18,
//! `numberOfTeamsInLobby` 1); o tamanho das equipes sai de
//! [`arena_team_size`].
//!
//! O [`Lobby`] guarda só o que a tela de lobby usa. Os outros campos da
//! resposta são ignorados, então um campo novo num patch não quebra a
//! leitura.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::custom_lobby::CustomLobby;
use crate::lobby_restrictions::{self, LobbyRestriction};
use crate::quick_play::PlayerSlot;
use crate::ranked::RankedStats;
use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) do lobby.
pub const LOBBY_PATH: &str = "/lol-lobby/v2/lobby";

/// Endpoint dos cargos do jogador logado (#18).
pub const POSITION_PREFERENCES_PATH: &str =
    "/lol-lobby/v2/lobby/members/localMember/position-preferences";

/// Endpoint do lobby aberto ou fechado (#225).
pub const PARTY_TYPE_PATH: &str = "/lol-lobby/v2/lobby/partyType";

/// Endpoint da equipe e da vaga do jogador logado na Arena (#224).
pub const SUBTEAM_DATA_PATH: &str = "/lol-lobby/v2/lobby/subteamData";

/// `gameMode` da Arena na LCU.
const ARENA_GAME_MODE: &str = "CHERRY";

/// `gameMode` do Rift Clássico (Jade) na LCU, a fila com as 5 posições em
/// ordem (#230).
const ORDERED_POSITIONS_GAME_MODE: &str = "JADE";

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

/// As 5 posições de rota, sem preenchimento: o que o jogador ordena no Rift
/// Clássico (#230).
pub const LANES: [Position; 5] = [
    Position::Top,
    Position::Jungle,
    Position::Middle,
    Position::Bottom,
    Position::Utility,
];

/// Por que uma ordem de posições (#230) não vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidPositionOrder {
    /// Nem as 5 posições nem só `fill`.
    WrongLength,
    /// Uma posição aparece duas vezes, ou a lista tem `fill` ou
    /// `unselected` junto das rotas.
    NotEachLaneOnce,
}

impl std::fmt::Display for InvalidPositionOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InvalidPositionOrder::WrongLength => {
                "mande as 5 posições em ordem ou só `fill` (qualquer posição)"
            }
            InvalidPositionOrder::NotEachLaneOnce => {
                "a ordem precisa ter topo, selva, meio, atirador e suporte uma vez cada"
            }
        })
    }
}

impl std::error::Error for InvalidPositionOrder {}

/// Confere uma ordem de posições do Rift Clássico (#230): as 5 rotas, cada
/// uma uma vez, ou só [`Position::Fill`] para "Qualquer posição".
pub fn check_position_order(order: &[Position]) -> Result<(), InvalidPositionOrder> {
    match order {
        [Position::Fill] => Ok(()),
        _ if order.len() != LANES.len() => Err(InvalidPositionOrder::WrongLength),
        _ if LANES.iter().all(|lane| order.contains(lane)) => Ok(()),
        _ => Err(InvalidPositionOrder::NotEachLaneOnce),
    }
}

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
    /// Quantos jogadores cabem no grupo nesta fila (2 na Solo/Duo, 18 na
    /// Arena 3x6, 5 nas outras). No lobby com equipes, nunca menos que
    /// `subteams.count * subteams.size`.
    pub max_lobby_size: u32,
    /// Se o jogador logado é o líder: só o líder troca a fila.
    pub is_leader: bool,
    /// Se a LCU deixa entrar na fila agora.
    pub can_start_activity: bool,
    /// Se a fila tem escolha de cargos (#18): `true` na normal alternada e
    /// nas ranqueadas, `false` no ARAM e no Jogo Dinâmico.
    pub show_position_selector: bool,
    /// Se o jogador ordena as 5 posições em vez de escolher primário e
    /// secundário (#230): `true` no Rift Clássico (`gameMode` `JADE`).
    /// Cada membro traz a ordem em [`LobbyMember::position_order`]; o
    /// jogador salva com o comando `setPositionOrder`.
    pub ordered_positions: bool,
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
    /// Por que a fila não pode começar (#227), como a LCU manda em
    /// `restrictions`. Vazia quando nada bloqueia. A LCU pode mandar itens
    /// mesmo com `canStartActivity`; a UI só mostra com a fila bloqueada.
    pub restrictions: Vec<LobbyRestriction>,
    /// As duas equipes, numa personalizada como o Modo de Treino (#229), ou
    /// `null` nas filas comuns. Veja [`crate::custom_lobby`].
    pub custom: Option<CustomLobby>,
    /// As equipes do lobby da Arena (#224): quantas são e quantas vagas tem
    /// cada uma. `null` nas filas sem equipes no lobby. A vaga de cada
    /// jogador vem em [`LobbyMember::subteam`].
    pub subteams: Option<SubteamLayout>,
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

/// As equipes de um lobby da Arena (#224).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubteamLayout {
    /// Quantas equipes cabem no lobby (6 na Arena 3x6).
    pub count: u32,
    /// Quantas vagas tem cada equipe (3 na Arena 3x6).
    pub size: u32,
}

/// A equipe e a vaga de um jogador no lobby da Arena (#224), com os índices
/// como a LCU manda (`subteamIndex` e `intraSubteamPosition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SubteamSlot {
    /// A equipe (`subteamIndex`).
    pub team: u32,
    /// A vaga dentro da equipe (`intraSubteamPosition`).
    pub position: u32,
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

/// Um jogador no lobby. A LCU não manda o nome aqui: o agente busca o Riot
/// ID pelo `puuid` e preenche [`game_name`](Self::game_name) depois (#325).
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
    /// As preferências de posição em ordem, da 1ª à 5ª (#230), só nas filas
    /// com [`Lobby::ordered_positions`]. `["fill"]` com "Qualquer posição".
    /// Vazia nas outras filas e antes de o jogador escolher; as posições
    /// que a LCU manda vazias ficam de fora. A 1ª é a mesma de
    /// `first_position`.
    pub position_order: Vec<Position>,
    /// Posições e campeões do Jogo Dinâmico (#218), na ordem dos slots.
    /// Vazia nas outras filas e antes de o jogador escolher.
    pub player_slots: Vec<PlayerSlot>,
    /// Equipe e vaga no lobby da Arena (#224). `null` nas filas sem equipes
    /// e enquanto a LCU não põe o jogador numa vaga.
    pub subteam: Option<SubteamSlot>,
    /// Elo do jogador na Solo/Duo e na Flex (#275). A LCU não manda no
    /// lobby: o agente busca pelo `puuid` e preenche depois, então chega
    /// `null` primeiro e numa mensagem `lobby` seguinte com os ranks. Também
    /// `null` se a LCU não respondeu.
    pub ranked: Option<RankedStats>,
    /// Nome do Riot ID (antes do `#`) (#325). A LCU não manda no lobby: o
    /// agente busca pelo `puuid` (`/lol-summoner/v2/summoners/puuid/`),
    /// junto com o elo, então chega vazio primeiro. Vazio também se a LCU
    /// não respondeu; aí a UI procura na lista de amigos.
    pub game_name: String,
    /// Tag do Riot ID (depois do `#`). Vazia junto com `game_name`.
    pub tag_line: String,
}

impl Lobby {
    /// Lê o lobby do JSON da LCU (resposta do `GET` ou `data` de um evento).
    /// `None` se não for um lobby.
    pub fn from_json(value: &Value) -> Option<Lobby> {
        let raw = RawLobby::deserialize(value).ok()?;
        let ordered_positions = raw.game_config.show_position_selector
            && raw.game_config.game_mode == ORDERED_POSITIONS_GAME_MODE;
        let members = raw
            .members
            .into_iter()
            .map(|m| {
                let position_order = if ordered_positions {
                    m.position_order()
                } else {
                    Vec::new()
                };
                LobbyMember {
                    puuid: m.puuid,
                    summoner_id: m.summoner_id,
                    summoner_icon_id: m.summoner_icon_id,
                    summoner_level: m.summoner_level,
                    is_leader: m.is_leader,
                    first_position: Position::from_lcu(m.first_position_preference.as_deref()),
                    second_position: Position::from_lcu(m.second_position_preference.as_deref()),
                    position_order,
                    player_slots: PlayerSlot::list_from_json(&m.player_slots),
                    subteam: match (m.subteam_index, m.intra_subteam_position) {
                        (Some(team), Some(position)) => Some(SubteamSlot { team, position }),
                        _ => None,
                    },
                    ranked: None,
                    game_name: String::new(),
                    tag_line: String::new(),
                }
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
        let subteams = subteam_layout(&raw.game_config, &members);
        let max_lobby_size = match subteams {
            Some(layout) => raw
                .game_config
                .max_lobby_size
                .max(layout.count * layout.size),
            None => raw.game_config.max_lobby_size,
        };
        Some(Lobby {
            queue_id: raw.game_config.queue_id,
            queue: Queue::from_id(raw.game_config.queue_id),
            max_lobby_size,
            is_leader: local_member.is_some_and(|m| m.is_leader),
            can_start_activity: raw.can_start_activity,
            show_position_selector: raw.game_config.show_position_selector,
            ordered_positions,
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
            restrictions: lobby_restrictions::from_json(&raw.restrictions),
            custom: value
                .get("gameConfig")
                .and_then(CustomLobby::from_game_config),
            subteams,
            chat_id: raw.multi_user_chat_id,
            chat_password: raw.multi_user_chat_password,
            chat_muc_jwt: raw.muc_jwt_dto,
        })
    }
}

/// As equipes do lobby (#224), ou `None` numa fila sem equipes no lobby.
///
/// Têm equipes a Arena (`gameMode` `CHERRY`), o lobby que a LCU diz ter mais
/// de uma equipe (`numberOfTeamsInLobby`) e o que já tem alguém numa vaga.
/// A personalizada fica de fora: as equipes dela vêm em [`Lobby::custom`].
/// Com `numberOfTeamsInLobby` em 1 (como a lista de filas manda a Arena), o
/// número de equipes sai de `maxLobbySize / maxTeamSize`.
///
/// A LCU manda a Arena como uma equipe só do tamanho do lobby (#303): a 1750
/// vem com `maxTeamSize` 18 e `numberOfTeamsInLobby` 1. Nesse caso o tamanho
/// da equipe sai de [`arena_team_size`].
fn subteam_layout(config: &RawGameConfig, members: &[LobbyMember]) -> Option<SubteamLayout> {
    if config.is_custom {
        return None;
    }
    let is_arena = config.game_mode == ARENA_GAME_MODE;
    let has_subteams = is_arena
        || config.number_of_teams_in_lobby > 1
        || members.iter().any(|m| m.subteam.is_some());
    let whole_lobby_is_one_team =
        config.number_of_teams_in_lobby <= 1 && config.max_team_size >= config.max_lobby_size;
    let size = if is_arena && whole_lobby_is_one_team {
        arena_team_size(config.max_lobby_size)
    } else {
        config.max_team_size
    };
    if !has_subteams || size == 0 {
        return None;
    }
    let count = if config.number_of_teams_in_lobby > 1 {
        config.number_of_teams_in_lobby
    } else {
        config.max_lobby_size.div_ceil(size)
    };
    (count > 0).then_some(SubteamLayout { count, size })
}

/// Jogadores por equipe na Arena (#303), pelo tamanho do lobby, já que a LCU
/// não manda: 3 quando o lobby divide em trios (18 na Arena 3x6, 6 equipes de
/// 3, como no client oficial) e 2 nas outras (16 na Arena de duplas, 8
/// equipes de 2).
fn arena_team_size(max_lobby_size: u32) -> u32 {
    if max_lobby_size > 0 && max_lobby_size.is_multiple_of(3) {
        3
    } else {
        2
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
    #[serde(default)]
    restrictions: Value,
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
    #[serde(default)]
    game_mode: String,
    #[serde(default)]
    is_custom: bool,
    #[serde(default)]
    number_of_teams_in_lobby: u32,
    #[serde(default)]
    max_team_size: u32,
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
    third_position_preference: Option<String>,
    #[serde(default)]
    fourth_position_preference: Option<String>,
    #[serde(default)]
    fifth_position_preference: Option<String>,
    #[serde(default)]
    player_slots: Value,
    #[serde(default)]
    subteam_index: Option<u32>,
    #[serde(default)]
    intra_subteam_position: Option<u32>,
}

impl RawMember {
    /// As 5 preferências em ordem (#230), sem as vazias. Com a 1ª em
    /// preenchimento, só `[Fill]`.
    fn position_order(&self) -> Vec<Position> {
        let order: Vec<Position> = [
            &self.first_position_preference,
            &self.second_position_preference,
            &self.third_position_preference,
            &self.fourth_position_preference,
            &self.fifth_position_preference,
        ]
        .into_iter()
        .map(|name| Position::from_lcu(name.as_deref()))
        .filter(|&p| p != Position::Unselected)
        .collect();
        match order.first() {
            Some(Position::Fill) => vec![Position::Fill],
            _ => order,
        }
    }
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

/// Salva a ordem das 5 posições do jogador logado no Rift Clássico (#230),
/// depois de conferir com [`check_position_order`]. Usa o mesmo `PUT` dos
/// cargos (`/lol-lobby/v2/lobby/members/localMember/position-preferences`),
/// de `firstPreference` até `fifthPreference`; "Qualquer posição" vai como
/// `FILL` na 1ª e `UNSELECTED` nas outras. A rota e o corpo saíram da
/// estrutura do lobby e ainda precisam ser confirmados no LoL real. A LCU
/// responde sem corpo; o lobby novo chega pelo WAMP (ou por
/// [`current_lobby`]).
pub async fn set_position_order(
    lcu: &LcuClient,
    order: &[Position],
) -> Result<(), SetPositionOrderError> {
    let body = position_order_body(order).map_err(SetPositionOrderError::Invalid)?;
    lcu.put::<_, Option<Value>>(POSITION_PREFERENCES_PATH, &body)
        .await
        .map_err(SetPositionOrderError::Lcu)?;
    Ok(())
}

/// O corpo do `PUT` de [`set_position_order`].
pub fn position_order_body(order: &[Position]) -> Result<Value, InvalidPositionOrder> {
    check_position_order(order)?;
    let name = |i: usize| order.get(i).copied().unwrap_or_default().lcu_name();
    Ok(json!({
        "firstPreference": name(0),
        "secondPreference": name(1),
        "thirdPreference": name(2),
        "fourthPreference": name(3),
        "fifthPreference": name(4),
    }))
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

/// O que o líder faz com outro membro do lobby (#267).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberAction {
    /// Tira o membro do grupo (`kick`).
    Kick,
    /// Passa a liderança para o membro (`promote`).
    Promote,
}

impl MemberAction {
    fn lcu_name(self) -> &'static str {
        match self {
            MemberAction::Kick => "kick",
            MemberAction::Promote => "promote",
        }
    }
}

/// Caminho de [`member_action`]: `/lol-lobby/v2/lobby/members/{summonerId}/kick`
/// ou `.../promote`.
pub fn member_action_path(summoner_id: u64, action: MemberAction) -> String {
    format!("{LOBBY_PATH}/members/{summoner_id}/{}", action.lcu_name())
}

/// Remove o membro `summoner_id` do grupo ou passa a liderança para ele
/// (`POST /lol-lobby/v2/lobby/members/{summonerId}/kick` ou `/promote`, #267).
/// Só o líder pode; a LCU recusa os outros. O caminho usa o `summonerId`,
/// como na lista `/help` da LCU; falta conferir no LoL real. O lobby novo
/// chega pelo WAMP (ou por [`current_lobby`]).
pub async fn member_action(
    lcu: &LcuClient,
    summoner_id: u64,
    action: MemberAction,
) -> Result<(), LcuError> {
    lcu.post::<_, Option<Value>>(&member_action_path(summoner_id, action), &json!({}))
        .await
        .map(|_| ())
}

/// Põe o jogador logado na vaga `slot` do lobby da Arena
/// (`PUT /lol-lobby/v2/lobby/subteamData`, #224). A LCU recusa fora de
/// lobby e numa fila sem equipes. O lobby novo chega pelo WAMP (ou por
/// [`current_lobby`]).
pub async fn set_subteam_slot(lcu: &LcuClient, slot: SubteamSlot) -> Result<(), LcuError> {
    let body = json!({
        "subteamIndex": slot.team,
        "intraSubteamPosition": slot.position,
    });
    lcu.put::<_, Option<Value>>(SUBTEAM_DATA_PATH, &body)
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

/// Erro de [`set_position_order`].
#[derive(Debug, thiserror::Error)]
pub enum SetPositionOrderError {
    /// A ordem não vale; nada foi mandado para a LCU.
    #[error("{0}")]
    Invalid(InvalidPositionOrder),
    /// A LCU recusou (fora de lobby, fila sem posições em ordem...) ou não
    /// respondeu.
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
        assert_eq!(lobby.custom, None);
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
            ordered_positions: false,
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
                position_order: Vec::new(),
                player_slots: Vec::new(),
                subteam: None,
                ranked: None,
                game_name: "Ana".into(),
                tag_line: "BR1".into(),
            }],
            can_invite: true,
            invitations: vec![LobbyInvitation {
                puuid: "amigo".into(),
                summoner_id: 4,
                state: InvitationState::Pending,
            }],
            is_open: true,
            restrictions: Vec::new(),
            custom: None,
            subteams: None,
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
                "orderedPositions": false,
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
                    "positionOrder": [],
                    "playerSlots": [],
                    "subteam": null,
                    "ranked": null,
                    "gameName": "Ana",
                    "tagLine": "BR1",
                }],
                "canInvite": true,
                "invitations": [{"puuid": "amigo", "summonerId": 4, "state": "Pending"}],
                "isOpen": true,
                "restrictions": [],
                "custom": null,
                "subteams": null,
            })
        );
    }

    #[test]
    fn le_as_equipes_da_arena() {
        let lobby = Lobby::from_json(&json!({
            "gameConfig": {
                "queueId": 1750, "gameMode": "CHERRY", "isCustom": false,
                "maxLobbySize": 18, "maxTeamSize": 3, "numberOfTeamsInLobby": 6,
            },
            "members": [
                {"puuid": "eu", "subteamIndex": 1, "intraSubteamPosition": 0},
                {"puuid": "amigo", "subteamIndex": 4, "intraSubteamPosition": 2},
                {"puuid": "chegando", "subteamIndex": null, "intraSubteamPosition": null},
            ],
        }))
        .unwrap();
        assert_eq!(lobby.subteams, Some(SubteamLayout { count: 6, size: 3 }));
        assert_eq!(lobby.max_lobby_size, 18);
        let slots: Vec<Option<SubteamSlot>> = lobby.members.iter().map(|m| m.subteam).collect();
        assert_eq!(
            slots,
            [
                Some(SubteamSlot {
                    team: 1,
                    position: 0
                }),
                Some(SubteamSlot {
                    team: 4,
                    position: 2
                }),
                None,
            ]
        );
        assert_eq!(
            serde_json::to_value(lobby.members[1].subteam).unwrap(),
            json!({"team": 4, "position": 2})
        );
        assert_eq!(
            serde_json::to_value(lobby.subteams).unwrap(),
            json!({"count": 6, "size": 3})
        );
    }

    #[test]
    fn equipes_da_arena_sem_o_numero_de_equipes() {
        // Como a lista de filas manda a Arena: uma equipe só no lobby. O
        // número sai do tamanho do lobby.
        let lobby = Lobby::from_json(&json!({
            "gameConfig": {
                "queueId": 1750, "gameMode": "CHERRY",
                "maxLobbySize": 18, "maxTeamSize": 3, "numberOfTeamsInLobby": 1,
            },
        }))
        .unwrap();
        assert_eq!(lobby.subteams, Some(SubteamLayout { count: 6, size: 3 }));

        // O lobby nunca diz caber menos gente que as equipes.
        let pequeno = Lobby::from_json(&json!({
            "gameConfig": {
                "queueId": 1750, "gameMode": "CHERRY",
                "maxLobbySize": 5, "maxTeamSize": 3, "numberOfTeamsInLobby": 6,
            },
        }))
        .unwrap();
        assert_eq!(pequeno.max_lobby_size, 18);
    }

    #[test]
    fn arena_com_o_lobby_inteiro_numa_equipe() {
        // Como a LCU manda no LoL real (#303, lido no LoL em 09/11/2026):
        // `maxTeamSize` e `numPlayersPerTeam` do tamanho do lobby e
        // `numberOfTeamsInLobby` 1. Tem que virar 6 equipes de 3, não 1
        // equipe de 18. O líder sozinho veio na equipe 1, vaga 3.
        let lobby = Lobby::from_json(&json!({
            "gameConfig": {
                "queueId": 1750, "gameMode": "CHERRY", "isCustom": false,
                "maxLobbySize": 18, "maxTeamSize": 18, "numPlayersPerTeam": 18,
                "numberOfTeamsInLobby": 1,
            },
            "members": [{"puuid": "eu", "isLeader": true, "subteamIndex": 1, "intraSubteamPosition": 3}],
        }))
        .unwrap();
        assert_eq!(lobby.subteams, Some(SubteamLayout { count: 6, size: 3 }));
        assert_eq!(
            lobby.members[0].subteam,
            Some(SubteamSlot {
                team: 1,
                position: 3
            })
        );
        assert_eq!(lobby.max_lobby_size, 18);

        // A Arena de duplas: 8 equipes de 2.
        let duplas = Lobby::from_json(&json!({
            "gameConfig": {
                "queueId": 1700, "gameMode": "CHERRY",
                "maxLobbySize": 16, "maxTeamSize": 16, "numberOfTeamsInLobby": 1,
            },
        }))
        .unwrap();
        assert_eq!(duplas.subteams, Some(SubteamLayout { count: 8, size: 2 }));
    }

    #[test]
    fn filas_sem_equipes_no_lobby() {
        // A gravação 16.22 (Solo/Duo) manda `maxTeamSize` 5 e
        // `numberOfTeamsInLobby` 1, com `subteamIndex` em `null`.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/16.22/rest.json");
        let rest: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for record in rest
            .iter()
            .filter(|r| r["path"] == LOBBY_PATH && r["status"] == 200)
        {
            let lobby = Lobby::from_json(&record["body"]).unwrap();
            assert_eq!(lobby.subteams, None);
            assert_eq!(lobby.max_lobby_size, 2);
            assert!(lobby.members.iter().all(|m| m.subteam.is_none()));
        }

        // A personalizada tem as equipes em `custom`, não em `subteams`.
        let custom = Lobby::from_json(&json!({
            "gameConfig": {
                "queueId": 3140, "gameMode": "PRACTICETOOL", "isCustom": true,
                "maxLobbySize": 10, "maxTeamSize": 5, "numberOfTeamsInLobby": 2,
            },
        }))
        .unwrap();
        assert_eq!(custom.subteams, None);
        assert_eq!(custom.max_lobby_size, 10);
    }

    #[test]
    fn le_o_motivo_da_fila_bloqueada() {
        let lobby = Lobby::from_json(&json!({
            "gameConfig": {"queueId": 440},
            "canStartActivity": false,
            "restrictions": [{"restrictionCode": "TeamSizeRestriction", "puuids": []}],
        }))
        .unwrap();
        assert!(!lobby.can_start_activity);
        assert_eq!(lobby.restrictions.len(), 1);
        assert_eq!(lobby.restrictions[0].code, "TeamSizeRestriction");
        assert!(lobby.restrictions[0].message.is_some());
    }

    #[test]
    fn le_as_posicoes_em_ordem_do_rift_classico() {
        let lobby = Lobby::from_json(&json!({
            "gameConfig": {"queueId": 4310, "gameMode": "JADE", "showPositionSelector": true},
            "members": [
                {"puuid": "eu",
                 "firstPositionPreference": "MIDDLE", "secondPositionPreference": "JUNGLE",
                 "thirdPositionPreference": "TOP", "fourthPositionPreference": "UTILITY",
                 "fifthPositionPreference": "BOTTOM"},
                {"puuid": "amigo",
                 "firstPositionPreference": "FILL", "secondPositionPreference": "UNSELECTED",
                 "thirdPositionPreference": "UNSELECTED", "fourthPositionPreference": null,
                 "fifthPositionPreference": null},
                {"puuid": "novo", "firstPositionPreference": "UNSELECTED"},
            ],
        }))
        .unwrap();
        use Position::*;
        assert!(lobby.ordered_positions);
        assert_eq!(
            lobby.members[0].position_order,
            [Middle, Jungle, Top, Utility, Bottom]
        );
        assert_eq!(lobby.members[0].first_position, Middle);
        assert_eq!(lobby.members[1].position_order, [Fill]);
        assert_eq!(lobby.members[2].position_order, []);

        // A Jade contra IA não tem escolha de posição.
        let bots = Lobby::from_json(&json!({
            "gameConfig": {"queueId": 4320, "gameMode": "JADE", "showPositionSelector": false},
            "members": [{"puuid": "eu"}],
        }))
        .unwrap();
        assert!(!bots.ordered_positions);

        // Na gravação 16.22 (Solo/Duo) a 3ª à 5ª vêm `null`: primário e
        // secundário, sem ordem.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/16.22/rest.json");
        let rest: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for body in rest
            .iter()
            .filter(|r| r["path"] == LOBBY_PATH && r["status"] == 200)
        {
            let lobby = Lobby::from_json(&body["body"]).unwrap();
            assert!(!lobby.ordered_positions);
            assert!(lobby.members.iter().all(|m| m.position_order.is_empty()));
        }
    }

    #[test]
    fn regras_da_ordem_das_posicoes() {
        use Position::*;
        assert_eq!(
            check_position_order(&[Top, Jungle, Middle, Bottom, Utility]),
            Ok(())
        );
        assert_eq!(
            check_position_order(&[Utility, Bottom, Middle, Jungle, Top]),
            Ok(())
        );
        assert_eq!(check_position_order(&[Fill]), Ok(()));
        assert_eq!(
            check_position_order(&[]),
            Err(InvalidPositionOrder::WrongLength)
        );
        assert_eq!(
            check_position_order(&[Top, Jungle]),
            Err(InvalidPositionOrder::WrongLength)
        );
        assert_eq!(
            check_position_order(&[Top, Top, Middle, Bottom, Utility]),
            Err(InvalidPositionOrder::NotEachLaneOnce)
        );
        assert_eq!(
            check_position_order(&[Fill, Jungle, Middle, Bottom, Utility]),
            Err(InvalidPositionOrder::NotEachLaneOnce)
        );
        assert_eq!(
            position_order_body(&[Middle, Jungle, Top, Utility, Bottom]).unwrap(),
            json!({
                "firstPreference": "MIDDLE", "secondPreference": "JUNGLE",
                "thirdPreference": "TOP", "fourthPreference": "UTILITY",
                "fifthPreference": "BOTTOM",
            })
        );
        assert_eq!(
            position_order_body(&[Fill]).unwrap(),
            json!({
                "firstPreference": "FILL", "secondPreference": "UNSELECTED",
                "thirdPreference": "UNSELECTED", "fourthPreference": "UNSELECTED",
                "fifthPreference": "UNSELECTED",
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
