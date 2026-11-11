//! Mensagens do canal `/ws` (#15): o contrato entre o agente e a UI.
//!
//! Tudo é JSON em frames de texto. O agente manda [`ServerMessage`] e a UI
//! manda [`ClientMessage`]. Os dois são enums com o campo `type` dizendo qual
//! mensagem é, e os campos em camelCase:
//!
//! ```json
//! {"type":"hello","protocol":1,"agentVersion":"0.1.0","state":{...}}
//! {"type":"lcu","status":{"state":"connected","session":1}}
//! {"type":"summoner","summoner":{"gameName":"...","tagLine":"...",...}}
//! {"type":"gameflowPhase","phase":"Lobby"}
//! {"type":"readyCheck","readyCheck":{"state":"InProgress","playerResponse":"None","timer":3.0,"duration":13.0}}
//! {"type":"lobby","lobby":{"queueId":420,"queue":"rankedSolo",...}}
//! {"type":"champSelect","champSelect":{"timer":{"phase":"BAN_PICK",...},"myTeam":[...],...}}
//! {"type":"receivedInvitations","invitations":[{"invitationId":"...","fromSummonerId":1,...}]}
//! {"type":"conversation","conversation":{"id":"...","gameName":"...","messages":[...],...}}
//! {"type":"champSelectChat","champSelectChat":{"messages":[{"id":"...","body":"...","fromCellId":0,...}]}}
//! {"type":"runePages","runePages":{"pages":[...],"currentPageId":994528921,"canCreatePage":true}}
//! {"type":"runeCatalog","runeCatalog":{"styles":[...],"perks":[...]}}
//! {"type":"summonerSpellCatalog","summonerSpellCatalog":{"spells":[{"id":4,"name":"Flash",...}]}}
//! {"type":"friends","friends":[{"id":"...","gameName":"...","availability":"online",...}]}
//! {"type":"friendGroups","friendGroups":[{"id":0,"name":"**Default","priority":0,"collapsed":false}]}
//! {"type":"response","id":7,"result":{"status":"ok"}}
//! ```
//!
//! ```json
//! {"id":7,"command":{"type":"ping"}}
//! {"id":8,"command":{"type":"createLobby","queue":"rankedSolo"}}
//! {"id":9,"command":{"type":"createLobbyById","queueId":2400}}
//! {"id":9,"command":{"type":"selectRunePage","pageId":994528921}}
//! {"id":10,"command":{"type":"confirmAction","actionId":13,"championId":121}}
//! {"id":11,"command":{"type":"setSummonerSpells","spell1Id":4,"spell2Id":14}}
//! ```
//!
//! Os tipos TypeScript em `ui/src/bindings/` saem destes tipos com `ts-rs`;
//! veja [`super::bindings`].
//!
//! Para mudar o formato de forma incompatível (tirar ou renomear um campo ou
//! uma mensagem), suba [`PROTOCOL_VERSION`]. Campo ou mensagem nova não
//! precisa: a UI ignora o que não conhece.
//!
//! As mensagens, os comandos e os campos do estado ficam em blocos por
//! assunto (conexão, lobby e fila, seleção, runas, social). Algo novo entra
//! no fim do bloco do seu assunto, e não no fim do enum, para que dois PRs de
//! assuntos diferentes não mexam nas mesmas linhas.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::champ_select::ChampSelect;
use crate::champ_select_chat::ChampSelectChat;
use crate::champions::{ChampSelectChampions, ChampionCatalog};
use crate::chat::Conversation;
use crate::collection::{ChampionSkins, Collection};
use crate::custom_game::CustomGameOptions;
use crate::custom_game_list::CustomGameListing;
use crate::custom_lobby::{BotDifficulty, CustomTeam};
use crate::friends::{Friend, FriendGroup};
use crate::gameflow::GameflowPhase;
use crate::invitations::{Invitee, ReceivedInvitation};
use crate::live_game::{LiveGame, PlayerLiveGame};
use crate::lobby::{Lobby, Position, Queue};
use crate::lobby_chat::LobbyChat;
use crate::lobby_link::LobbyLink;
use crate::mastery::ChampionMastery;
use crate::match_history::{MatchDetails, MatchHistory};
use crate::matchmaking::Matchmaking;
use crate::patch_notes::PatchNotes;
use crate::player_profile::PlayerProfile;
use crate::players::{RecentPlayer, SuggestedPlayer};
use crate::queues::GameQueue;
use crate::quick_play::PlayerSlotInput;
use crate::ranked::RankedStats;
use crate::ready_check::ReadyCheck;
use crate::runes::{RuneCatalog, RunePageInput, RunePages};
use crate::summoner::Summoner;
use crate::summoner_spells::SummonerSpellCatalog;
use crate::swaps::SwapKind;
use crate::training::TutorialPart;
use crate::wallet::Wallet;

/// Versão do formato das mensagens, mandada no [`ServerMessage::Hello`].
pub const PROTOCOL_VERSION: u32 = 1;

/// Mensagem do agente para a UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ServerMessage {
    // Conexão com o LoL e invocador
    /// Primeira mensagem de cada conexão, com o estado inteiro. Também é
    /// mandada de novo se a UI ficar para trás e perder eventos: a UI deve
    /// trocar todo o estado pelo que vier aqui.
    Hello {
        /// [`PROTOCOL_VERSION`] do agente.
        protocol: u32,
        agent_version: String,
        // Em caixa porque o estado (com a seleção de campeões) é bem maior
        // que as outras mensagens; o JSON é o mesmo.
        state: Box<AgentState>,
    },
    /// O estado da conexão com o LoL mudou.
    Lcu { status: LcuStatus },
    /// O invocador logado mudou. `null` quando o LoL fecha ou antes do login.
    Summoner { summoner: Option<Summoner> },
    /// A fase do gameflow mudou (`None`, `Lobby`, `Matchmaking`,
    /// `ReadyCheck`, `ChampSelect`, `InProgress`...; veja [`GameflowPhase`]).
    /// `null` com o LoL fechado ou quando a LCU ainda não respondeu a fase.
    GameflowPhase { phase: Option<GameflowPhase> },

    // Lobby, fila e partida encontrada
    /// O lobby mudou (criado, fila trocada, alguém entrou ou saiu). `null`
    /// fora de lobby e com o LoL fechado.
    Lobby { lobby: Option<Lobby> },
    /// O link para outra pessoa entrar no lobby (#226), gerado com
    /// `createLobbyLink`. Fica no estado enquanto o lobby existir (gerar de
    /// novo pode trazer o mesmo link ou um novo); `null` fora de lobby e com
    /// o LoL fechado.
    LobbyLink { lobby_link: Option<LobbyLink> },
    /// A busca de partida mudou: entrou ou saiu da fila, mais um segundo na
    /// fila (chega a cada segundo enquanto procura), partida encontrada.
    /// `null` fora da fila e com o LoL fechado.
    Matchmaking { matchmaking: Option<Matchmaking> },
    /// O ready check (partida encontrada, #20) abriu, andou ou fechou. Chega
    /// mais ou menos a cada segundo enquanto ele está aberto, com o `timer`
    /// novo. `null` quando não há ready check (fechou, ou o LoL fechou).
    ReadyCheck { ready_check: Option<ReadyCheck> },
    /// Mudou a lista de filas em que dá para criar lobby agora (#146): chega
    /// logo depois de conectar e quando uma fila entra ou sai de rotação.
    /// Sempre a lista inteira, na ordem do `queueId`. Vazia com o LoL
    /// fechado ou se a LCU não mandou a lista. As filas fixas da tela de
    /// escolha de modo vêm mesmo fechadas, com `available: false` (#209).
    /// Mudar só a disponibilidade de uma fila também manda a lista.
    Queues { queues: Vec<GameQueue> },
    /// Mudaram os campeões que o jogador pode pôr nos slots do Jogo
    /// Dinâmico (#218): os dele, os alugados e os da rotação gratuita, em
    /// ordem de id. Chega logo depois de conectar e a cada volta ao lobby.
    /// `null` com o LoL fechado ou enquanto a LCU não respondeu.
    PlayableChampions {
        playable_champions: Option<Vec<u32>>,
    },
    /// Mudou o progresso do Tutorial (#228): as três partes, cada uma
    /// concluída ou não e liberada ou com cadeado. Chega logo depois de
    /// conectar e a cada volta à escolha de modo. `null` com o LoL fechado
    /// ou quando a LCU não diz o progresso (a UI deixa escolher qualquer
    /// parte).
    Tutorial { tutorial: Option<Vec<TutorialPart>> },
    /// Mudaram os campeões que podem ser bots na personalizada (#229), em
    /// ordem de id. Chega quando o jogador entra numa personalizada pela
    /// primeira vez na conexão. `null` com o LoL fechado ou antes disso.
    AvailableBots { available_bots: Option<Vec<u32>> },
    /// Mudou a lista de personalizadas abertas (#264): chega depois de um
    /// `refreshCustomGames` e quando a LCU traz a lista nova pelo WAMP.
    /// Sempre a lista inteira, na ordem da LCU. `null` antes do primeiro
    /// `refreshCustomGames` da conexão e com o LoL fechado.
    CustomGames {
        custom_games: Option<Vec<CustomGameListing>>,
    },

    // Seleção de campeões
    /// A seleção de campeões (#22) abriu ou mudou: hover, ban, pick, troca
    /// de feitiço, troca de etapa ou de vez. Sempre o estado inteiro da
    /// seleção, não só o que mudou. `null` quando a seleção fecha (partida
    /// abrindo, dodge) e com o LoL fechado.
    ChampSelect { champ_select: Option<ChampSelect> },
    /// A sala de chat do time na seleção de campeões (#27) abriu, recebeu
    /// mensagem (de um aliado ou do jogador) ou fechou. Sempre a sala
    /// inteira, com as mensagens desde que ela abriu. `null` quando a sala
    /// fecha (fim da seleção, dodge) e com o LoL fechado.
    ChampSelectChat {
        champ_select_chat: Option<ChampSelectChat>,
    },
    /// A sala de chat do grupo no lobby (#206) abriu, recebeu mensagem ou
    /// fechou. Sempre a sala inteira, com as mensagens desde que ela abriu.
    /// `null` sem lobby e com o LoL fechado.
    LobbyChat { lobby_chat: Option<LobbyChat> },
    /// Os feitiços de invocador do jogo, para a escolha na seleção (#24).
    /// Chega uma vez por conexão com o LoL, logo depois de conectar. `null`
    /// com o LoL fechado ou se a LCU não mandou a lista.
    SummonerSpellCatalog {
        summoner_spell_catalog: Option<SummonerSpellCatalog>,
    },
    /// Os campeões do jogo, para a grade da seleção (#28). Chega uma vez por
    /// conexão com o LoL, logo depois de conectar. `null` com o LoL fechado
    /// ou se a LCU não mandou a lista.
    ChampionCatalog {
        champion_catalog: Option<ChampionCatalog>,
    },
    /// Mudou a lista de campeões que o jogador pode escolher ou banir na
    /// seleção (#28). Sempre as duas listas; cada uma é `null` fora da
    /// seleção ou enquanto a LCU não mandou.
    ChampSelectChampions {
        champ_select_champions: ChampSelectChampions,
    },

    // Runas
    /// As páginas de runas (#26) mudaram: criada, editada, apagada, outra
    /// aplicada, ou a recomendada da seleção apareceu ou sumiu. Sempre a lista
    /// inteira. `null` com o LoL fechado.
    RunePages { rune_pages: Option<RunePages> },
    /// As árvores de runas para o editor (#26). Chega uma vez por conexão
    /// com o LoL, logo depois de conectar. `null` com o LoL fechado ou se a
    /// LCU não mandou as árvores.
    RuneCatalog { rune_catalog: Option<RuneCatalog> },

    // Social: amigos, convites e conversas
    /// A lista de amigos (#30) mudou: alguém ficou online ou offline, entrou
    /// numa partida, virou amigo ou deixou de ser. Sempre a lista inteira,
    /// já ordenada (disponíveis primeiro, depois por nome). Vazia com o LoL
    /// fechado.
    Friends { friends: Vec<Friend> },
    /// As pastas de amigos do client oficial mudaram: criada, renomeada,
    /// apagada ou reordenada. Sempre a lista inteira, na ordem do client.
    /// Vazia com o LoL fechado ou se a LCU não mandou as pastas.
    FriendGroups { friend_groups: Vec<FriendGroup> },
    /// Os jogadores das partidas recentes mudaram (#206), relidos pelo
    /// `loadRecentPlayers`. Sempre a lista inteira, a partida mais nova
    /// primeiro. Vazia com o LoL fechado.
    RecentPlayers { recent_players: Vec<RecentPlayer> },
    /// Os jogadores que o client oficial sugere convidar mudaram (#377),
    /// relidos pelo `loadSuggestedPlayers` ou depois de um
    /// `dismissSuggestedPlayer`. Sempre a lista inteira, na ordem da LCU.
    /// Vazia com o LoL fechado.
    SuggestedPlayers {
        suggested_players: Vec<SuggestedPlayer>,
    },
    /// Os convites de lobby recebidos mudaram (#32): chegou um, ou um foi
    /// aceito, recusado ou cancelado. Sempre a lista inteira dos pendentes;
    /// vazia quando não há nenhum e com o LoL fechado.
    ReceivedInvitations {
        invitations: Vec<ReceivedInvitation>,
    },
    /// Uma conversa 1:1 com um amigo (#31) abriu ou mudou: mensagem nova
    /// (recebida ou mandada), não lidas, nome do amigo. Sempre a conversa
    /// inteira, com o histórico da sessão; a UI troca a conversa de mesmo
    /// `id` por esta, ou acrescenta se não tinha.
    Conversation { conversation: Conversation },
    /// A conversa `id` fechou (o amigo ou o client a fechou, ou o LoL
    /// fechou). A UI tira ela da lista.
    ConversationClosed { id: String },

    // Perfil
    /// Os ranks do jogador logado na Solo/Duo e na Flex (#171) mudaram:
    /// chegam logo depois de conectar e no fim de cada ranqueada. `null`
    /// com o LoL fechado ou enquanto a LCU não respondeu.
    Ranked { ranked: Option<RankedStats> },
    /// Os campeões com mais maestria do jogador logado (#175), no máximo 3,
    /// do que tem mais pontos para o que tem menos. Chegam logo depois de
    /// conectar e no fim de cada partida. Vazia com o LoL fechado.
    ChampionMasteries {
        champion_masteries: Vec<ChampionMastery>,
    },
    /// O histórico de partidas do jogador logado (#172) mudou: a primeira
    /// página logo depois de conectar, uma partida nova no fim de cada
    /// partida, ou mais uma página (`loadMoreMatches`). Sempre a lista
    /// inteira já carregada. `null` com o LoL fechado ou enquanto a LCU não
    /// respondeu.
    MatchHistory { match_history: Option<MatchHistory> },
    /// O placar de uma partida pedido com `openMatchDetails` (#174). Fica
    /// no estado até outro ser pedido. `null` com o LoL fechado.
    MatchDetails { match_details: Option<MatchDetails> },
    /// O perfil de outro jogador pedido com `openPlayerProfile` (#255)
    /// chegou ou ganhou mais partidas (`loadMorePlayerMatches`). Fica no
    /// estado até outro ser pedido. `null` com o LoL fechado.
    PlayerProfile {
        player_profile: Option<PlayerProfile>,
    },

    // Partida ao vivo
    /// A partida em andamento (#268) começou ou mudou: chegou o Riot ID, o
    /// elo ou as partidas recentes de um jogador, as runas (depois da tela
    /// de carregamento) ou o relógio do jogo se afastou da conta da UI.
    /// Sempre a partida inteira. `null` fora de partida e com o LoL fechado.
    LiveGame { live_game: Option<LiveGame> },
    /// A partida em andamento do jogador do perfil aberto com
    /// `openPlayerProfile` (#436), pela API da Riot, para a aba "Em jogo".
    /// Chega depois do perfil e de novo quando o elo e as partidas recentes
    /// de cada jogador chegam da LCU. `null` quando ele não está em partida,
    /// sem a API configurada, ao abrir outro perfil e com o LoL fechado.
    PlayerLiveGame {
        player_live_game: Option<PlayerLiveGame>,
    },

    // Coleção
    /// A coleção de campeões da conta (#369), pedida com `openCollection`.
    /// Fica no estado até ser pedida de novo. `null` com o LoL fechado.
    Collection { collection: Option<Collection> },
    /// As skins de um campeão (#369), pedidas com `openChampionSkins`. Fica
    /// no estado até outro campeão ser pedido. `null` com o LoL fechado.
    ChampionSkins {
        champion_skins: Option<ChampionSkins>,
    },

    // Início
    /// As notas da atualização atual do LoL (#301), lidas ao conectar. Ficam
    /// no estado quando o LoL fecha. `null` enquanto o agente não conectou
    /// nenhuma vez ou se a LCU não deu a versão do jogo.
    PatchNotes { patch_notes: Option<PatchNotes> },

    // Saldo
    /// O saldo de Essência Azul, Essência Laranja e RP (#370) mudou: chega
    /// logo depois de conectar e quando a LCU avisa (fim de partida, compra
    /// no client oficial). `null` com o LoL fechado ou antes do login.
    Wallet { wallet: Option<Wallet> },

    // Respostas aos comandos
    /// Resposta a um [`ClientMessage`], com o mesmo `id`.
    Response { id: u32, result: CommandResult },
    /// A UI mandou algo que não é um [`ClientMessage`] válido. Sem `id`,
    /// porque nem sempre dá para ler o `id` de uma mensagem quebrada.
    InvalidMessage { message: String },
}

/// Tudo o que a UI precisa para desenhar a tela do zero.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    // Conexão com o LoL e invocador
    pub lcu: LcuStatus,
    pub summoner: Option<Summoner>,
    pub gameflow_phase: Option<GameflowPhase>,

    // Lobby, fila e partida encontrada
    pub lobby: Option<Lobby>,
    /// Link para entrar no lobby (#226), depois de um `createLobbyLink`, ou
    /// `null`.
    pub lobby_link: Option<LobbyLink>,
    pub matchmaking: Option<Matchmaking>,
    /// Ready check aberto (partida encontrada, #20), ou `null`.
    pub ready_check: Option<ReadyCheck>,
    /// Filas em que dá para criar lobby agora (#146), lidas da LCU, mais as
    /// fixas da tela de escolha de modo que estão fechadas, com `available:
    /// false` (#209). Vazia com o LoL fechado.
    pub queues: Vec<GameQueue>,
    /// Campeões que o jogador pode pôr nos slots do Jogo Dinâmico (#218),
    /// ou `null`.
    pub playable_champions: Option<Vec<u32>>,
    /// As partes do Tutorial (#228), ou `null` sem o progresso.
    pub tutorial: Option<Vec<TutorialPart>>,
    /// Campeões que podem ser bots na personalizada (#229), ou `null`.
    pub available_bots: Option<Vec<u32>>,
    /// Sala de chat do grupo no lobby (#206), ou `null` sem lobby.
    pub lobby_chat: Option<LobbyChat>,
    /// Personalizadas abertas (#264), depois de um `refreshCustomGames`, ou
    /// `null`.
    pub custom_games: Option<Vec<CustomGameListing>>,

    // Seleção de campeões
    /// Seleção de campeões em andamento (#22), ou `null`.
    pub champ_select: Option<ChampSelect>,
    /// Sala de chat do time na seleção de campeões (#27), ou `null` fora da
    /// seleção.
    pub champ_select_chat: Option<ChampSelectChat>,
    /// Feitiços de invocador do jogo, para a escolha na seleção (#24), ou
    /// `null`.
    pub summoner_spell_catalog: Option<SummonerSpellCatalog>,
    /// Campeões do jogo, para a grade da seleção (#28), ou `null`.
    pub champion_catalog: Option<ChampionCatalog>,
    /// Campeões que o jogador pode escolher e banir na seleção atual (#28).
    pub champ_select_champions: ChampSelectChampions,

    // Runas
    /// Páginas de runas (#26), ou `null` com o LoL fechado.
    pub rune_pages: Option<RunePages>,
    /// Árvores de runas para o editor (#26), ou `null`.
    pub rune_catalog: Option<RuneCatalog>,

    // Social: amigos, convites e conversas
    /// Amigos com a presença de cada um (#30), na ordem da tela.
    pub friends: Vec<Friend>,
    /// Pastas de amigos do client oficial, na ordem dele. Cada amigo diz a
    /// sua em [`Friend::group_id`].
    pub friend_groups: Vec<FriendGroup>,
    /// Jogadores das partidas recentes (#206), a partida mais nova primeiro.
    pub recent_players: Vec<RecentPlayer>,
    /// Jogadores que o client oficial sugere convidar (#377), na ordem da
    /// LCU. Vazia até a UI pedir com `loadSuggestedPlayers`.
    pub suggested_players: Vec<SuggestedPlayer>,
    /// Convites de lobby recebidos e ainda pendentes (#32).
    pub received_invitations: Vec<ReceivedInvitation>,
    /// Conversas 1:1 abertas (#31), cada uma com o histórico da sessão. Sem
    /// ordem definida; a UI ordena (pela última mensagem, por exemplo).
    pub conversations: Vec<Conversation>,

    // Perfil
    /// Ranks do jogador logado (#171), ou `null`.
    pub ranked: Option<RankedStats>,
    /// Até 3 campeões com mais maestria (#175).
    pub champion_masteries: Vec<ChampionMastery>,
    /// Histórico de partidas já carregado (#172), ou `null`.
    pub match_history: Option<MatchHistory>,
    /// Último placar pedido com `openMatchDetails` (#174), ou `null`.
    pub match_details: Option<MatchDetails>,
    /// Último perfil de outro jogador pedido com `openPlayerProfile` (#255),
    /// ou `null`.
    pub player_profile: Option<PlayerProfile>,

    // Partida ao vivo
    /// A partida em andamento (#268), ou `null` fora de partida.
    pub live_game: Option<LiveGame>,
    /// A partida em andamento do jogador do perfil aberto (#436), ou `null`.
    pub player_live_game: Option<PlayerLiveGame>,

    // Coleção
    /// Última coleção pedida com `openCollection` (#369), ou `null`.
    pub collection: Option<Collection>,
    /// Últimas skins pedidas com `openChampionSkins` (#369), ou `null`.
    pub champion_skins: Option<ChampionSkins>,

    // Início
    /// Notas da atualização atual do LoL (#301), ou `null`.
    pub patch_notes: Option<PatchNotes>,

    // Saldo
    /// Essência Azul, Essência Laranja e RP da conta (#370), ou `null`.
    pub wallet: Option<Wallet>,
}

/// Estado da conexão do agente com o LoL.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LcuStatus {
    /// O agente acabou de abrir e ainda não tentou conectar.
    #[default]
    Starting,
    /// Conectado num client do LoL.
    Connected {
        /// Conta as conexões desde que o agente abriu, a partir de 1. Muda
        /// quando o LoL fecha e reabre.
        session: u32,
    },
    /// A conexão caiu (o LoL fechou ou reiniciou). O agente já está
    /// tentando de novo.
    Disconnected { reason: String },
    /// Uma tentativa de conectar falhou; a próxima sai em `retryInMs`.
    Retrying {
        /// Falhas seguidas, a partir de 1.
        attempt: u32,
        retry_in_ms: u32,
        /// O LoL não está aberto. É o estado normal antes de o jogador
        /// abrir o jogo; `false` é um erro de verdade (senha recusada,
        /// lockfile velho...).
        client_not_running: bool,
        /// Mensagem do erro, para log ou tela de diagnóstico.
        error: String,
    },
}

/// Mensagem da UI para o agente: um comando com um `id` escolhido pela UI.
/// O agente responde com um [`ServerMessage::Response`] com o mesmo `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClientMessage {
    pub id: u32,
    pub command: Command,
}

/// O que a UI pode pedir ao agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    // Canal e LoL
    /// Não faz nada; responde `ok`. Serve para testar o canal.
    Ping,
    /// Pede um [`ServerMessage::Hello`] novo com o estado inteiro, que chega
    /// antes da resposta.
    GetState,
    /// Abre o LoL pelo Riot Client ([`crate::launcher`]). Responde `ok` assim
    /// que o pedido sai, ou se o LoL já estava aberto; a subida do LoL chega
    /// pelas mensagens `lcu`. Erro `launchFailed` quando não dá para abrir
    /// (Riot Client não encontrado, falha ao chamar o executável).
    LaunchLeague,
    /// Reabre a UI oficial do LoL (`POST /riotclient/launch-ux`), a saída de
    /// emergência (#13). Responde `notConnected` com o LoL fechado e `lcu` se
    /// a LCU recusar. O agente não fecha a UI de novo sozinho até a próxima
    /// conexão com o LoL.
    LaunchUx,

    // Lobby, fila e partida encontrada
    /// Cria um lobby na fila pedida, ou troca a fila do lobby atual (só o
    /// líder pode). O lobby novo chega numa mensagem `lobby` antes da
    /// resposta. Responde `notConnected` com o LoL fechado e `lcu` se a LCU
    /// recusar (fila fechada, jogador não é o líder, já está na fila...).
    CreateLobby { queue: Queue },
    /// Sai do lobby. A mensagem `lobby` com `null` chega antes da resposta.
    /// Responde `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    LeaveLobby,
    /// Volta ao lobby da última partida no pós-jogo, com o mesmo grupo e a
    /// mesma fila, como o "Jogar novamente" do client oficial (#180). O lobby
    /// chega numa mensagem `lobby` antes da resposta. Responde `notConnected`
    /// com o LoL fechado e `lcu` se a LCU recusar (fora do pós-jogo, por
    /// exemplo).
    PlayAgain,
    /// Escolhe o cargo primário e o secundário do jogador logado (#18). Só
    /// vale num lobby com `showPositionSelector`. O par segue as regras do
    /// client oficial: cargos diferentes, e `second` em `unselected` quando
    /// `first` é `fill` ou `unselected`; fora disso responde
    /// `invalidCommand` sem falar com a LCU. O lobby com os cargos novos
    /// chega numa mensagem `lobby` antes da resposta. Responde `notConnected`
    /// com o LoL fechado e `lcu` se a LCU recusar (fora de lobby, fila sem
    /// cargos...).
    SetPositions { first: Position, second: Position },
    /// Salva a ordem das 5 posições do jogador logado no Rift Clássico
    /// (#230). Só vale num lobby com `orderedPositions`. `positions` traz
    /// as 5 rotas (`top`, `jungle`, `middle`, `bottom`, `utility`) da 1ª à
    /// 5ª preferência, cada uma uma vez, ou só `["fill"]` para "Qualquer
    /// posição"; fora disso responde `invalidCommand` sem falar com a LCU,
    /// e também fora de lobby ou num lobby sem `orderedPositions`. O lobby
    /// com a ordem nova chega numa mensagem `lobby` antes da resposta.
    /// Responde `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    SetPositionOrder { positions: Vec<Position> },
    /// Entra na fila com o lobby atual (só o líder pode). A mensagem
    /// `matchmaking` com a busca chega antes da resposta. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar (fora de
    /// lobby, não é o líder, punição de fila...).
    StartQueue,
    /// Sai da fila. A mensagem `matchmaking` com `null` chega antes da
    /// resposta. Responde `notConnected` com o LoL fechado e `lcu` se a LCU
    /// recusar.
    StopQueue,
    /// Como `createLobby`, com o `queueId` de uma das filas da lista
    /// `queues` (#146): serve também para as filas que não são uma das
    /// [`Queue`] fixas (ARAM: Desordem, Jade, contra IA...). Responde
    /// `invalidCommand` sem falar com a LCU se a fila não está na lista, e
    /// os mesmos erros do `createLobby` no resto. Também responde
    /// `invalidCommand` numa fila da lista com `available: false` (#209).
    /// Uma fila fixa da tela de escolha de modo que ainda não está na lista
    /// segue para a LCU, que decide (#210).
    CreateLobbyById { queue_id: u32 },
    /// Aceita a partida encontrada (`POST
    /// /lol-matchmaking/v1/ready-check/accept`, #20). Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar (por
    /// exemplo, sem ready check aberto). A mudança chega depois como
    /// `readyCheck` com `playerResponse: "Accepted"`.
    AcceptReadyCheck,
    /// Recusa a partida encontrada (`POST
    /// /lol-matchmaking/v1/ready-check/decline`, #20). Mesmos erros do
    /// `acceptReadyCheck`.
    DeclineReadyCheck,
    /// Inicia uma parte do Tutorial (filas 2000, 2010 e 2020, #209 e #228):
    /// cria o lobby na fila da parte e entra na busca. Com `queueId`, a parte
    /// escolhida (fora dessas três filas responde `invalidCommand` sem falar
    /// com a LCU); sem ele, a próxima que o jogador ainda não terminou, pelo
    /// `GET /lol-npe-tutorial-path/v1/tutorials` (sem essa informação, ou com
    /// as três feitas, a Parte 1). O lobby e a busca chegam nas mensagens
    /// `lobby` e `matchmaking` antes da resposta. Responde `notConnected` com
    /// o LoL fechado e `lcu` se a LCU recusar (parte ainda com cadeado, por
    /// exemplo).
    StartTutorial {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        queue_id: Option<u32>,
    },
    /// Abre o Modo de Treino (#209 e #229): cria uma personalizada
    /// `PRACTICETOOL` no Summoner's Rift, com senha, e deixa o jogador no
    /// lobby dela, com as equipes em `lobby.custom`. A partida começa com
    /// `startCustomGame`. Responde `notConnected` com o LoL fechado e `lcu` se
    /// a LCU recusar criar.
    StartPracticeTool,
    /// Cria uma partida personalizada com as opções do formulário (#262): o
    /// mesmo `POST /lol-lobby/v2/lobby` do Modo de Treino, com
    /// `customGameLobby`, e deixa o jogador no lobby dela, com as equipes e as
    /// opções em `lobby.custom`. O Rift Clássico (#270) só aceita às cegas e
    /// alternada; os outros mapas aceitam qualquer tipo de seleção. Nome vazio
    /// ou com mais de 64 caracteres, tipo de seleção que o mapa não aceita ou
    /// equipe fora de 1 a 5 responde `invalidCommand` sem falar com a LCU. A partida começa com
    /// `startCustomGame`. Responde `notConnected` com o LoL fechado e `lcu` se
    /// a LCU recusar criar.
    CreateCustomGame { options: CustomGameOptions },
    /// Escolhe as duas posições e os dois campeões do jogador logado no Jogo
    /// Dinâmico (#218), num lobby com `showQuickPlaySlots`: `PUT
    /// /lol-lobby/v1/lobby/members/localMember/player-slots`. Sempre os dois
    /// slots, na ordem. Cada slot tem uma das 5 posições (sem `fill`), e as
    /// posições e os campeões são diferentes entre os slots. Feitiços e
    /// página de runas em `null` mantêm os do slot quando o campeão e a
    /// posição não mudam e senão usam os que a LCU recomenda. Fora das regras
    /// responde `invalidCommand` sem falar com a LCU. O lobby novo chega numa
    /// mensagem `lobby` antes da resposta. Responde `notConnected` com o LoL
    /// fechado e `lcu` se a LCU recusar (fora de lobby, fila sem slots,
    /// campeão que o jogador não tem...).
    SetPlayerSlots { slots: Vec<PlayerSlotInput> },
    /// Abre (`open: true`) ou fecha o lobby (#225): com o lobby aberto,
    /// amigos entram sem convite (`PUT /lol-lobby/v2/lobby/partyType`). Só
    /// o líder pode: fora de lobby ou sem ser o líder responde
    /// `invalidCommand` sem falar com a LCU. O lobby novo, com `isOpen`,
    /// chega numa mensagem `lobby` antes da resposta. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    SetLobbyOpen { open: bool },
    /// Remove o jogador `summoner_id` do grupo (#267): `POST
    /// /lol-lobby/v2/lobby/members/{summonerId}/kick`. Só o líder pode, e
    /// só com outro jogador do lobby (vale também nas equipes e entre os
    /// espectadores da personalizada): fora de lobby, sem ser o líder, com
    /// o próprio jogador ou com quem não está no lobby responde
    /// `invalidCommand` sem falar com a LCU. O lobby novo chega numa
    /// mensagem `lobby` antes da resposta. Responde `notConnected` com o LoL
    /// fechado e `lcu` se a LCU recusar.
    KickLobbyMember {
        #[ts(type = "number")]
        summoner_id: u64,
    },
    /// Passa a liderança do grupo para o jogador `summoner_id` (#267):
    /// `POST /lol-lobby/v2/lobby/members/{summonerId}/promote`. Mesmas
    /// regras e respostas de `kickLobbyMember`. Depois disso o jogador
    /// logado deixa de ser o líder (`isLeader` falso no lobby novo).
    PromoteLobbyMember {
        #[ts(type = "number")]
        summoner_id: u64,
    },
    /// Libera (`allowed` verdadeiro) ou tira a permissão de convidar do
    /// jogador `summoner_id` (#412): `POST
    /// /lol-lobby/v2/lobby/members/{summonerId}/grant-invite` ou
    /// `/revoke-invite`. Mesmas regras e respostas de `kickLobbyMember`. O
    /// estado de cada membro vem em `LobbyMember.allowedInviteOthers`.
    SetLobbyMemberInvite {
        #[ts(type = "number")]
        summoner_id: u64,
        allowed: bool,
    },
    /// Gera o link para outra pessoa entrar no lobby (#226), o do botão
    /// "copiar link" do client oficial: `GET
    /// /lol-lobby/v2/ags/agsActivityId` e o código em `/lol-lobby/v2/ags/
    /// {id}/joinCode` (reaproveita o ativo que ainda vale por mais de um
    /// minuto, senão `POST` gera um novo). O link chega numa mensagem
    /// `lobbyLink` antes da resposta. Fora de lobby responde
    /// `invalidCommand` sem falar com a LCU; `notConnected` com o LoL
    /// fechado e `lcu` se a LCU recusar.
    CreateLobbyLink,
    /// Põe o jogador logado na equipe `team`, vaga `position`, do lobby da
    /// Arena (#224): `PUT /lol-lobby/v2/lobby/subteamData`. Os índices são
    /// os de [`crate::lobby::SubteamSlot`], como a LCU manda. Fora de lobby
    /// ou num lobby sem `subteams` responde `invalidCommand` sem falar com a
    /// LCU. O lobby novo chega numa mensagem `lobby` antes da resposta.
    /// Responde `notConnected` com o LoL fechado e `lcu` se a LCU recusar
    /// (vaga que não existe, ocupada...).
    SetSubteamSlot { team: u32, position: u32 },
    /// Adiciona um bot na equipe `team` da personalizada (#229): `POST
    /// /lol-lobby/v1/lobby/custom/bots`. O campeão é um de `availableBots`,
    /// a posição uma das 5 rotas (sem `fill` nem `unselected`) e a
    /// dificuldade não pode ser `unknown`. Só o líder pode, fora de
    /// personalizada ou com a equipe cheia responde `invalidCommand` sem
    /// falar com a LCU. O lobby novo chega numa mensagem `lobby` antes da
    /// resposta. Responde `notConnected` com o LoL fechado e `lcu` se a LCU
    /// recusar.
    AddBot {
        team: CustomTeam,
        champion_id: u32,
        difficulty: BotDifficulty,
        position: Position,
    },
    /// Muda o campeão, a dificuldade e a posição do bot `botUuid` (#229).
    /// Mesmas regras e erros do `addBot`; um bot que não está no lobby
    /// responde `invalidCommand`.
    UpdateBot {
        bot_uuid: String,
        champion_id: u32,
        difficulty: BotDifficulty,
        position: Position,
    },
    /// Tira o bot `botUuid` da personalizada (#229). Mesmos erros do
    /// `updateBot`.
    RemoveBot { bot_uuid: String },
    /// Leva o jogador logado para a equipe `team` da personalizada (#229),
    /// o "Juntar-se": `POST /lol-lobby/v2/lobby/team/{team}`. Qualquer
    /// membro pode. Fora de personalizada, já na equipe ou com ela cheia
    /// responde `invalidCommand` sem falar com a LCU. O lobby novo chega numa
    /// mensagem `lobby` antes da resposta.
    SwitchTeam { team: CustomTeam },
    /// Leva o jogador logado para os espectadores da personalizada (#263):
    /// `POST /lol-lobby/v2/lobby/team/spectator`. Para voltar, `switchTeam`.
    /// Qualquer membro pode. Fora de personalizada, com a personalizada sem
    /// espectadores (`settings.spectators` `none`), já assistindo ou com as
    /// vagas cheias (`maxSpectators`) responde `invalidCommand` sem falar com
    /// a LCU. O lobby novo chega numa mensagem `lobby` antes da resposta.
    /// Responde `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    Spectate,
    /// Inicia a partida da personalizada (#229): `POST
    /// /lol-lobby/v1/lobby/custom/start-champ-select`. Só o líder; fora de
    /// personalizada responde `invalidCommand`. A seleção de campeões chega
    /// pelas mensagens de sempre. Responde `notConnected` com o LoL fechado e
    /// `lcu` se a LCU recusar.
    StartCustomGame,
    /// Pede a lista de personalizadas abertas (#264): `POST
    /// /lol-lobby/v1/custom-games/refresh` e `GET /lol-lobby/v1/custom-games`.
    /// A lista chega numa mensagem `customGames` antes da resposta (e de novo
    /// se a LCU trouxer a lista nova depois). Responde `notConnected` com o
    /// LoL fechado e `lcu` se a LCU recusar.
    RefreshCustomGames,
    /// Entra na personalizada `partyId` da lista (#264): `POST
    /// /lol-lobby/v2/party/{partyId}/join` com a senha (`lobbyPassword`). O
    /// lobby dela chega numa mensagem `lobby` antes da resposta. `partyId`
    /// vazio ou com algo além de letras, números e hífens, sem senha numa
    /// partida que a lista diz ter senha, ou com senha de mais de 64
    /// caracteres responde `invalidCommand` sem falar com a LCU. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar (senha
    /// errada, partida cheia ou que já fechou).
    JoinCustomGame {
        party_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        password: Option<String>,
    },

    // Seleção de campeões
    /// Marca o campeão `championId` na ação `actionId` da seleção (#23), sem
    /// confirmar: o hover de um ban ou de um pick na vez do jogador, ou a
    /// intenção de pick antes da vez dele (etapa `PLANNING` ou durante os
    /// bans). `actionId` é o `id` de uma ação do jogador logado em
    /// `champSelect.actions`, em geral o de `champSelect.pendingAction`.
    /// Responde `invalidCommand` sem falar com a LCU fora da seleção, com
    /// `championId` 0, com ação de outro jogador, já confirmada, que não é
    /// ban nem pick, ou com ban fora da vez. A seleção nova chega numa
    /// mensagem `champSelect` antes da resposta. Responde `notConnected` com
    /// o LoL fechado e `lcu` se a LCU recusar (campeão que o jogador não
    /// tem, já banido ou escolhido...).
    HoverChampion { action_id: u32, champion_id: u32 },
    /// Confirma o ban ou o pick do campeão `championId` na ação `actionId`
    /// (#23): marca o campeão e confirma, como o botão do client oficial.
    /// Só na vez da ação (`isInProgress`). Mesmos erros do `hoverChampion`.
    ConfirmAction { action_id: u32, champion_id: u32 },
    /// Troca os feitiços de invocador do jogador logado na seleção (#24):
    /// `spell1Id` é o da tecla D e `spell2Id` o da tecla F, como em
    /// `champSelect.myTeam`. Para inverter os dois, mande os mesmos ids
    /// trocados. Vale da etapa `PLANNING` até a `FINALIZATION`. Os ids vêm de
    /// `summonerSpellCatalog`. Responde `invalidCommand` sem falar com a LCU
    /// fora da seleção (ou com a partida abrindo), com um feitiço `0`, com os
    /// dois iguais, com feitiço que não existe, que não vale na fila
    /// (`queues` do feitiço) ou acima do nível do invocador
    /// (`summonerLevel`). A seleção nova chega numa mensagem `champSelect`
    /// antes da resposta. Responde `notConnected` com o LoL fechado e `lcu`
    /// se a LCU recusar.
    SetSummonerSpells { spell1_id: u32, spell2_id: u32 },
    /// Pede uma troca a um aliado na seleção (#25): de campeão
    /// (`kind: "champion"`), de vez no draft (`"pickOrder"`) ou de cargo
    /// (`"position"`). `swapId` é o `id` de uma entrada `AVAILABLE` em
    /// `champSelect.trades`, `pickOrderSwaps` ou `positionSwaps`, conforme o
    /// `kind`. Responde `invalidCommand` sem falar com a LCU fora da
    /// seleção, com troca que não existe nesse tipo ou fora do estado
    /// `AVAILABLE`. A seleção nova (com a troca em `SENT`) chega numa
    /// mensagem `champSelect` antes da resposta. Responde `notConnected`
    /// com o LoL fechado e `lcu` se a LCU recusar.
    RequestSwap {
        kind: SwapKind,
        #[ts(type = "number")]
        swap_id: u64,
    },
    /// Aceita a troca que o aliado pediu (entrada em `RECEIVED`), #25.
    /// Mesmos erros do `requestSwap`.
    AcceptSwap {
        kind: SwapKind,
        #[ts(type = "number")]
        swap_id: u64,
    },
    /// Recusa a troca que o aliado pediu (entrada em `RECEIVED`), #25.
    /// Mesmos erros do `requestSwap`.
    DeclineSwap {
        kind: SwapKind,
        #[ts(type = "number")]
        swap_id: u64,
    },
    /// Cancela a troca que o jogador pediu (entrada em `SENT`), #25.
    /// Mesmos erros do `requestSwap`.
    CancelSwap {
        kind: SwapKind,
        #[ts(type = "number")]
        swap_id: u64,
    },
    /// Rola de novo o campeão do jogador no ARAM (#201): `POST
    /// /lol-champ-select/v1/session/my-selection/reroll`. A LCU sorteia
    /// outro campeão e o antigo vai para o banco. Responde `invalidCommand`
    /// sem falar com a LCU fora da seleção (ou com a partida abrindo), em
    /// fila sem reroll (`champSelect.allowRerolling`) ou sem reroll sobrando
    /// (`champSelect.rerollsRemaining`). A seleção nova chega numa mensagem
    /// `champSelect` antes da resposta. Responde `notConnected` com o LoL
    /// fechado e `lcu` se a LCU recusar.
    RerollChampion,
    /// Troca o campeão do jogador pelo `championId` do banco do ARAM
    /// (#201): `POST /lol-champ-select/v1/session/bench/swap/{championId}`.
    /// O campeão do jogador vai para o banco. Responde `invalidCommand` sem
    /// falar com a LCU fora da seleção (ou com a partida abrindo), sem banco
    /// (`champSelect.benchEnabled`) ou com campeão fora de
    /// `champSelect.benchChampions`. A seleção nova chega numa mensagem
    /// `champSelect` antes da resposta. Responde `notConnected` com o LoL
    /// fechado e `lcu` se a LCU recusar (por exemplo, na espera entre duas
    /// trocas).
    SwapBenchChampion { champion_id: u32 },
    /// Marca o aviso de autofill como visto (#360): `champSelect.
    /// autofillNotice` fica falso até o fim desta seleção (a mensagem
    /// `champSelect` chega antes da resposta) e o agente avisa a LCU com `PUT
    /// /lol-lobby/v1/autofill-displayed`. Responde `invalidCommand` fora da
    /// seleção, `notConnected` com o LoL fechado e `lcu` se a LCU recusar (o
    /// aviso continua escondido).
    AckAutofill,
    /// Manda `body` na sala de chat do time na seleção de campeões (#27).
    /// Não precisa dizer a sala: vai para a que está aberta. A mensagem
    /// chega numa `champSelectChat` antes da resposta quando a LCU devolve a
    /// mensagem, e depois pelo WAMP nos outros casos. Responde
    /// `invalidCommand` para texto vazio (só espaços) ou sem sala aberta,
    /// sem falar com a LCU, `notConnected` com o LoL fechado e `lcu` se a
    /// LCU recusar.
    SendChampSelectMessage { body: String },
    /// Manda `body` no chat do grupo do lobby (#206). Vai para a sala aberta.
    /// A mensagem chega numa `lobbyChat` antes da resposta quando a LCU
    /// devolve a mensagem, e depois pelo WAMP nos outros casos. Responde
    /// `invalidCommand` para texto vazio ou sem sala aberta, sem falar com a
    /// LCU, `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    SendLobbyMessage { body: String },

    // Runas
    /// Aplica a página de runas `pageId` (#26). As páginas com o
    /// `currentPageId` novo chegam numa mensagem `runePages` antes da
    /// resposta. Responde `notConnected` com o LoL fechado e `lcu` se a LCU
    /// recusar (página que não existe...).
    SelectRunePage {
        #[ts(type = "number")]
        page_id: u64,
    },
    /// Cria uma página de runas e já a aplica (#26). A página é conferida
    /// com as árvores do `runeCatalog` antes de ir para a LCU: nome vazio,
    /// árvores iguais ou runa fora da linha voltam `invalidCommand` (veja
    /// `selectedPerkIds` em `RunePage` para a ordem das runas). A lista nova
    /// chega numa mensagem `runePages` antes da resposta. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar, como sem
    /// espaço para página nova (`canCreatePage: false`).
    CreateRunePage { page: RunePageInput },
    /// Troca nome, árvores e runas da página `pageId` (#26), com a mesma
    /// conferência do `createRunePage`. Não muda qual página está em uso.
    /// Mesmos erros do `createRunePage`; `lcu` também quando a página não
    /// existe ou não é editável (`isEditable: false`).
    UpdateRunePage {
        #[ts(type = "number")]
        page_id: u64,
        page: RunePageInput,
    },
    /// Apaga a página `pageId` (#26). A lista nova chega numa mensagem
    /// `runePages` antes da resposta. Responde `notConnected` com o LoL
    /// fechado e `lcu` se a LCU recusar (página que não existe ou não pode
    /// ser apagada, `isDeletable: false`).
    DeleteRunePage {
        #[ts(type = "number")]
        page_id: u64,
    },

    // Social: amigos, convites e conversas
    /// Aceita um convite de lobby recebido (#32) e entra no lobby de quem
    /// convidou. O lobby novo e a lista de convites sem o aceito chegam nas
    /// mensagens `lobby` e `receivedInvitations` antes da resposta (o lobby
    /// pode chegar um pouco depois, pelo WAMP). Responde `invalidCommand`
    /// sem falar com a LCU se o id tiver caracteres que a LCU não usa,
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar (convite
    /// expirado, jogador na fila...).
    AcceptInvitation { invitation_id: String },
    /// Recusa um convite de lobby recebido (#32). A lista sem ele chega numa
    /// mensagem `receivedInvitations` antes da resposta. Mesmos erros do
    /// `acceptInvitation`.
    DeclineInvitation { invitation_id: String },
    /// Convida jogadores para o lobby atual (#32), com o `summonerId` e o
    /// `puuid` da lista de amigos. Só vale com `canInvite` no lobby. Os
    /// convites aparecem em `invitations` do lobby, que chega numa mensagem
    /// `lobby` antes da resposta. Responde `invalidCommand` sem falar com a
    /// LCU com a lista vazia ou um convidado sem `summonerId` nem `puuid`,
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar (fora de
    /// lobby, sem permissão, lobby cheio...).
    InviteToLobby { invitees: Vec<Invitee> },
    /// Convida para o lobby atual o jogador com o Riot ID
    /// `gameName#tagLine` (#206), amigo ou não, como o "Convidar por nome"
    /// do client oficial. Mesma resposta do `inviteToLobby`; responde
    /// `invalidCommand` sem falar com a LCU sem nome ou sem tag, e
    /// `notFound` quando a LCU não acha o jogador.
    InviteByRiotId { game_name: String, tag_line: String },
    /// Relê os jogadores das partidas recentes (#206), para a seção
    /// "Recentes" do convite (a UI pede ao abrir o pop-up). A lista chega
    /// numa mensagem `recentPlayers` antes da resposta. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    LoadRecentPlayers,
    /// Relê os jogadores sugeridos (#377), para a seção "Sugeridos" do
    /// convite (a UI pede ao abrir o pop-up): `GET
    /// /lol-suggested-players/v1/suggested-players`. A lista chega numa
    /// mensagem `suggestedPlayers` antes da resposta. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    LoadSuggestedPlayers,
    /// Dispensa o sugerido `summoner_id` (#377): `DELETE
    /// /lol-suggested-players/v1/suggested-players/{summonerId}`. A lista sem
    /// ele chega numa mensagem `suggestedPlayers` antes da resposta. Responde
    /// `invalidCommand` sem falar com a LCU se ele não está na lista,
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    DismissSuggestedPlayer {
        #[ts(type = "number")]
        summoner_id: u64,
    },
    /// Manda `body` na conversa 1:1 `conversationId` (#31): o `id` de uma
    /// [`Conversation`], que é o `pid` do amigo no chat. Se a conversa ainda
    /// não estava aberta, a LCU abre e ela chega numa mensagem
    /// `conversation`. A mensagem mandada chega numa `conversation` antes da
    /// resposta quando a LCU devolve a mensagem, e depois pelo WAMP nos
    /// outros casos. Responde `invalidCommand` para texto vazio (só espaços)
    /// sem falar com a LCU, `notConnected` com o LoL fechado e `lcu` se a
    /// LCU recusar.
    SendChatMessage {
        conversation_id: String,
        body: String,
    },
    /// Diz qual conversa 1:1 a UI está mostrando (#31), ou `null` para
    /// nenhuma, como a UI oficial faz ao abrir e fechar a janela da conversa.
    /// A LCU zera as não lidas da conversa ativa; a mudança chega numa
    /// mensagem `conversation`. Responde `notConnected` com o LoL fechado e
    /// `lcu` se a LCU recusar.
    SetActiveConversation { conversation_id: Option<String> },

    // Perfil
    /// Carrega a próxima página do histórico de partidas (#172), depois das
    /// que já estão em `matchHistory`. O histórico com as partidas novas
    /// chega numa mensagem `matchHistory` antes da resposta. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    LoadMoreMatches,
    /// Busca o placar com os 10 jogadores da partida `gameId` (#174). O
    /// placar chega numa mensagem `matchDetails` antes da resposta. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar (partida
    /// que não existe, histórico fora do ar).
    OpenMatchDetails {
        #[ts(type = "number")]
        game_id: u64,
    },
    /// Busca o perfil do jogador `puuid` (#255), por exemplo de um amigo:
    /// invocador, ranks, maestria e a primeira página do histórico. O perfil
    /// chega numa mensagem `playerProfile` antes da resposta. Uma parte que
    /// a LCU não manda (histórico de perfil privado, por exemplo) fica vazia.
    /// O placar de uma partida dele sai do `openMatchDetails`. Responde
    /// `invalidCommand` para um `puuid` vazio ou com caracteres fora de
    /// letras, números e `-`, `notConnected` com o LoL fechado, `notFound`
    /// se a LCU não conhece o jogador e `lcu` se a LCU recusar.
    OpenPlayerProfile { puuid: String },
    /// Carrega a próxima página do histórico do perfil aberto com
    /// `openPlayerProfile` (#255). O perfil com as partidas novas chega numa
    /// mensagem `playerProfile` antes da resposta. Responde `invalidCommand`
    /// sem perfil aberto, `notConnected` com o LoL fechado e `lcu` se a LCU
    /// recusar.
    LoadMorePlayerMatches,

    // Coleção
    /// Lê a coleção de campeões da conta (#369): todos os campeões do jogo,
    /// os que a conta tem e os que não tem, com as contagens de skins. A
    /// coleção chega numa mensagem `collection` antes da resposta. Responde
    /// `notConnected` com o LoL fechado ou antes do invocador carregar e
    /// `lcu` se a LCU recusar.
    OpenCollection,
    /// Lê as skins do campeão `championId` (#369), com os cromas de cada
    /// uma e o que a conta tem. As skins chegam numa mensagem
    /// `championSkins` antes da resposta. Se outro campeão for pedido antes
    /// destas chegarem, estas são descartadas. Responde `invalidCommand`
    /// para `championId` `0`, `notConnected` com o LoL fechado ou antes do
    /// invocador carregar e `lcu` se a LCU recusar.
    OpenChampionSkins { champion_id: u32 },

    // Início
    /// Abre a página de `patchNotes.url` (#301) no navegador padrão. Responde
    /// `notFound` sem notas no estado e `launchFailed` se não deu para abrir
    /// o navegador.
    OpenPatchNotes,
}

/// Resultado de um comando.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CommandResult {
    Ok,
    Error { code: ErrorCode, message: String },
}

/// Por que um comando falhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// O comando precisa do LoL e o agente não está conectado.
    NotConnected,
    /// A LCU respondeu com erro ou não respondeu.
    Lcu,
    /// Não foi possível abrir o LoL pelo Riot Client.
    LaunchFailed,
    /// O comando chegou bem formado mas com valores que não fazem sentido
    /// (como o mesmo cargo de primário e secundário). Nada foi mandado para
    /// a LCU.
    InvalidCommand,
    /// O que o comando procura não existe (jogador com esse Riot ID, #206).
    NotFound,
}

impl CommandResult {
    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        CommandResult::Error {
            code,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_game::{AramMap, CustomMap, CustomPickType, SpectatorPolicy};
    use serde_json::json;

    #[test]
    fn formato_das_mensagens_do_agente() {
        let hello = ServerMessage::Hello {
            protocol: PROTOCOL_VERSION,
            agent_version: "0.1.0".into(),
            state: Box::default(),
        };
        assert_eq!(
            serde_json::to_value(&hello).unwrap(),
            json!({
                "type": "hello",
                "protocol": 1,
                "agentVersion": "0.1.0",
                "state": {
                    "lcu": {"state": "starting"},
                    "summoner": null,
                    "gameflowPhase": null,
                    "readyCheck": null,
                    "lobby": null,
                    "lobbyLink": null,
                    "matchmaking": null,
                    "queues": [],
                    "playableChampions": null,
                    "tutorial": null,
                    "availableBots": null,
                    "lobbyChat": null,
                    "customGames": null,
                    "champSelect": null,
                    "receivedInvitations": [],
                    "recentPlayers": [],
                    "suggestedPlayers": [],
                    "conversations": [],
                    "champSelectChat": null,
                    "runePages": null,
                    "runeCatalog": null,
                    "summonerSpellCatalog": null,
                    "championCatalog": null,
                    "champSelectChampions": {"pickable": null, "bannable": null, "subset": null},
                    "friends": [],
                    "friendGroups": [],
                    "ranked": null,
                    "championMasteries": [],
                    "matchHistory": null,
                    "matchDetails": null,
                    "playerProfile": null,
                    "liveGame": null,
                    "playerLiveGame": null,
                    "collection": null,
                    "championSkins": null,
                    "patchNotes": null,
                    "wallet": null,
                },
            })
        );

        let retrying = ServerMessage::Lcu {
            status: LcuStatus::Retrying {
                attempt: 2,
                retry_in_ms: 1000,
                client_not_running: true,
                error: "o LoL não está aberto".into(),
            },
        };
        assert_eq!(
            serde_json::to_value(&retrying).unwrap(),
            json!({
                "type": "lcu",
                "status": {
                    "state": "retrying",
                    "attempt": 2,
                    "retryInMs": 1000,
                    "clientNotRunning": true,
                    "error": "o LoL não está aberto",
                },
            })
        );

        let phase = ServerMessage::GameflowPhase {
            phase: Some(GameflowPhase::Lobby),
        };
        assert_eq!(
            serde_json::to_value(&phase).unwrap(),
            json!({"type": "gameflowPhase", "phase": "Lobby"})
        );

        let ready_check = ServerMessage::ReadyCheck {
            ready_check: ReadyCheck::from_json(&json!({
                "state": "InProgress",
                "playerResponse": "None",
                "timer": 3.0,
            })),
        };
        assert_eq!(
            serde_json::to_value(&ready_check).unwrap(),
            json!({
                "type": "readyCheck",
                "readyCheck": {
                    "state": "InProgress",
                    "playerResponse": "None",
                    "timer": 3.0,
                    "duration": 13.0,
                },
            })
        );

        let erro = ServerMessage::Response {
            id: 3,
            result: CommandResult::error(ErrorCode::NotConnected, "o LoL não está aberto"),
        };
        assert_eq!(
            serde_json::to_value(&erro).unwrap(),
            json!({
                "type": "response",
                "id": 3,
                "result": {
                    "status": "error",
                    "code": "notConnected",
                    "message": "o LoL não está aberto",
                },
            })
        );
    }

    #[test]
    fn le_comandos_da_ui() {
        let msg: ClientMessage =
            serde_json::from_value(json!({"id": 7, "command": {"type": "getState"}})).unwrap();
        assert_eq!(
            msg,
            ClientMessage {
                id: 7,
                command: Command::GetState
            }
        );
        for (name, command) in [
            ("acceptReadyCheck", Command::AcceptReadyCheck),
            ("playAgain", Command::PlayAgain),
            ("declineReadyCheck", Command::DeclineReadyCheck),
            ("startTutorial", Command::StartTutorial { queue_id: None }),
            ("startPracticeTool", Command::StartPracticeTool),
            ("startCustomGame", Command::StartCustomGame),
            ("spectate", Command::Spectate),
        ] {
            let msg: ClientMessage =
                serde_json::from_value(json!({"id": 9, "command": {"type": name}})).unwrap();
            assert_eq!(msg.command, command);
        }
        let msg: ClientMessage = serde_json::from_value(
            json!({"id": 9, "command": {"type": "startTutorial", "queueId": 2010}}),
        )
        .unwrap();
        assert_eq!(
            msg.command,
            Command::StartTutorial {
                queue_id: Some(2010)
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({"id": 9, "command": {
            "type": "createCustomGame", "options": {
                "name": "Partida de Fulano", "password": "", "map": "summonersRift",
                "pickType": "draftMode", "aramMap": "howlingAbyss", "teamSize": 5, "spectators": "lobbyOnly",
                "hideFromPublicList": false, "spectatorDelay": true,
            },
        }}))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::CreateCustomGame {
                options: CustomGameOptions {
                    name: "Partida de Fulano".into(),
                    password: String::new(),
                    map: CustomMap::SummonersRift,
                    pick_type: CustomPickType::DraftMode,
                    aram_map: AramMap::HowlingAbyss,
                    team_size: 5,
                    spectators: SpectatorPolicy::LobbyOnly,
                    hide_from_public_list: false,
                    spectator_delay: true,
                }
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({"id": 9, "command": {
            "type": "addBot", "team": "team2", "championId": 22,
            "difficulty": "beginner", "position": "bottom",
        }}))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::AddBot {
                team: CustomTeam::Team2,
                champion_id: 22,
                difficulty: BotDifficulty::Beginner,
                position: Position::Bottom,
            }
        );
        let msg: ClientMessage = serde_json::from_value(
            json!({"id": 9, "command": {"type": "removeBot", "botUuid": "b"}}),
        )
        .unwrap();
        assert_eq!(
            msg.command,
            Command::RemoveBot {
                bot_uuid: "b".into()
            }
        );
        let msg: ClientMessage = serde_json::from_value(
            json!({"id": 9, "command": {"type": "setLobbyOpen", "open": true}}),
        )
        .unwrap();
        assert_eq!(msg.command, Command::SetLobbyOpen { open: true });
        for (json, command) in [
            (
                json!({"type": "kickLobbyMember", "summonerId": 42}),
                Command::KickLobbyMember { summoner_id: 42 },
            ),
            (
                json!({"type": "promoteLobbyMember", "summonerId": 42}),
                Command::PromoteLobbyMember { summoner_id: 42 },
            ),
            (
                json!({"type": "setLobbyMemberInvite", "summonerId": 42, "allowed": true}),
                Command::SetLobbyMemberInvite {
                    summoner_id: 42,
                    allowed: true,
                },
            ),
            (
                json!({"type": "loadSuggestedPlayers"}),
                Command::LoadSuggestedPlayers,
            ),
            (
                json!({"type": "dismissSuggestedPlayer", "summonerId": 42}),
                Command::DismissSuggestedPlayer { summoner_id: 42 },
            ),
            (
                json!({"type": "refreshCustomGames"}),
                Command::RefreshCustomGames,
            ),
            (
                json!({"type": "joinCustomGame", "partyId": "06645d51-8137"}),
                Command::JoinCustomGame {
                    party_id: "06645d51-8137".into(),
                    password: None,
                },
            ),
            (
                json!({"type": "joinCustomGame", "partyId": "p", "password": "segredo"}),
                Command::JoinCustomGame {
                    party_id: "p".into(),
                    password: Some("segredo".into()),
                },
            ),
        ] {
            assert_eq!(serde_json::from_value::<Command>(json).unwrap(), command);
        }
        let msg: ClientMessage = serde_json::from_value(json!(
            {"id": 9, "command": {"type": "setSubteamSlot", "team": 2, "position": 1}}
        ))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::SetSubteamSlot {
                team: 2,
                position: 1
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!(
            {"id": 10, "command": {"type": "createLobbyLink"}}
        ))
        .unwrap();
        assert_eq!(msg.command, Command::CreateLobbyLink);
        let msg: ClientMessage =
            serde_json::from_value(json!({"id": 8, "command": {"type": "launchLeague"}})).unwrap();
        assert_eq!(msg.command, Command::LaunchLeague);
        assert_eq!(
            serde_json::to_value(ErrorCode::LaunchFailed).unwrap(),
            json!("launchFailed")
        );
        let msg: ClientMessage =
            serde_json::from_value(json!({"id": 8, "command": {"type": "launchUx"}})).unwrap();
        assert_eq!(msg.command, Command::LaunchUx);
        let msg: ClientMessage = serde_json::from_value(
            json!({"id": 8, "command": {"type": "createLobby", "queue": "rankedFlex"}}),
        )
        .unwrap();
        assert_eq!(
            msg.command,
            Command::CreateLobby {
                queue: Queue::RankedFlex
            }
        );
        let msg: ClientMessage =
            serde_json::from_value(json!({"id": 9, "command": {"type": "leaveLobby"}})).unwrap();
        assert_eq!(msg.command, Command::LeaveLobby);
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 10,
            "command": {"type": "setPositions", "first": "middle", "second": "jungle"},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::SetPositions {
                first: Position::Middle,
                second: Position::Jungle,
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 10,
            "command": {"type": "setPositionOrder", "positions": ["middle", "jungle", "top", "utility", "bottom"]},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::SetPositionOrder {
                positions: vec![
                    Position::Middle,
                    Position::Jungle,
                    Position::Top,
                    Position::Utility,
                    Position::Bottom,
                ],
            }
        );
        let msg: ClientMessage =
            serde_json::from_value(json!({"id": 10, "command": {"type": "startQueue"}})).unwrap();
        assert_eq!(msg.command, Command::StartQueue);
        let msg: ClientMessage =
            serde_json::from_value(json!({"id": 11, "command": {"type": "stopQueue"}})).unwrap();
        assert_eq!(msg.command, Command::StopQueue);
        let msg: ClientMessage = serde_json::from_value(
            json!({"id": 11, "command": {"type": "createLobbyById", "queueId": 2400}}),
        )
        .unwrap();
        assert_eq!(msg.command, Command::CreateLobbyById { queue_id: 2400 });
        let msg: ClientMessage = serde_json::from_value(
            json!({"id": 12, "command": {"type": "selectRunePage", "pageId": 994528921}}),
        )
        .unwrap();
        assert_eq!(
            msg.command,
            Command::SelectRunePage {
                page_id: 994_528_921
            }
        );
        let page = json!({
            "name": "Nova",
            "primaryStyleId": 8000,
            "subStyleId": 8300,
            "selectedPerkIds": [8005, 9111, 9104, 8017, 8313, 8321, 5005, 5008, 5011],
        });
        let input = RunePageInput {
            name: "Nova".into(),
            primary_style_id: 8000,
            sub_style_id: 8300,
            selected_perk_ids: vec![8005, 9111, 9104, 8017, 8313, 8321, 5005, 5008, 5011],
        };
        let msg: ClientMessage = serde_json::from_value(
            json!({"id": 13, "command": {"type": "createRunePage", "page": page}}),
        )
        .unwrap();
        assert_eq!(
            msg.command,
            Command::CreateRunePage {
                page: input.clone()
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 14,
            "command": {"type": "updateRunePage", "pageId": 7, "page": page},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::UpdateRunePage {
                page_id: 7,
                page: input
            }
        );
        let msg: ClientMessage = serde_json::from_value(
            json!({"id": 15, "command": {"type": "deleteRunePage", "pageId": 7}}),
        )
        .unwrap();
        assert_eq!(msg.command, Command::DeleteRunePage { page_id: 7 });
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 12,
            "command": {"type": "hoverChampion", "actionId": 13, "championId": 121},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::HoverChampion {
                action_id: 13,
                champion_id: 121,
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 13,
            "command": {"type": "confirmAction", "actionId": 13, "championId": 121},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::ConfirmAction {
                action_id: 13,
                champion_id: 121,
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 14,
            "command": {"type": "setSummonerSpells", "spell1Id": 4, "spell2Id": 14},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::SetSummonerSpells {
                spell1_id: 4,
                spell2_id: 14
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 15,
            "command": {"type": "requestSwap", "kind": "pickOrder", "swapId": 28},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::RequestSwap {
                kind: SwapKind::PickOrder,
                swap_id: 28
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 16,
            "command": {"type": "rerollChampion"},
        }))
        .unwrap();
        assert_eq!(msg.command, Command::RerollChampion);
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 18,
            "command": {"type": "ackAutofill"},
        }))
        .unwrap();
        assert_eq!(msg.command, Command::AckAutofill);
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 17,
            "command": {"type": "swapBenchChampion", "championId": 266},
        }))
        .unwrap();
        assert_eq!(msg.command, Command::SwapBenchChampion { champion_id: 266 });
        for (name, kind) in [
            ("acceptSwap", SwapKind::Champion),
            ("declineSwap", SwapKind::Position),
            ("cancelSwap", SwapKind::Champion),
        ] {
            let msg: ClientMessage = serde_json::from_value(json!({
                "id": 15,
                "command": {"type": name, "kind": kind, "swapId": 47},
            }))
            .unwrap();
            let expected = match name {
                "acceptSwap" => Command::AcceptSwap { kind, swap_id: 47 },
                "declineSwap" => Command::DeclineSwap { kind, swap_id: 47 },
                _ => Command::CancelSwap { kind, swap_id: 47 },
            };
            assert_eq!(msg.command, expected);
        }
        // Só os tipos de troca que existem.
        assert!(serde_json::from_value::<ClientMessage>(
            json!({"id": 1, "command": {"type": "requestSwap", "kind": "skin", "swapId": 1}})
        )
        .is_err());
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 12,
            "command": {"type": "sendChatMessage", "conversationId": "a@br1.pvp.net", "body": "oi"},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::SendChatMessage {
                conversation_id: "a@br1.pvp.net".into(),
                body: "oi".into(),
            }
        );
        for (value, expected) in [
            (json!("a@br1.pvp.net"), Some("a@br1.pvp.net".to_owned())),
            (json!(null), None),
        ] {
            let msg: ClientMessage = serde_json::from_value(json!({
                "id": 13,
                "command": {"type": "setActiveConversation", "conversationId": value},
            }))
            .unwrap();
            assert_eq!(
                msg.command,
                Command::SetActiveConversation {
                    conversation_id: expected
                }
            );
        }
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 14,
            "command": {"type": "sendChampSelectMessage", "body": "vou mid"},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::SendChampSelectMessage {
                body: "vou mid".into()
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 12,
            "command": {"type": "acceptInvitation", "invitationId": "abc"},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::AcceptInvitation {
                invitation_id: "abc".into()
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 13,
            "command": {"type": "declineInvitation", "invitationId": "abc"},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::DeclineInvitation {
                invitation_id: "abc".into()
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 15,
            "command": {"type": "openMatchDetails", "gameId": 1234567890123u64},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::OpenMatchDetails {
                game_id: 1_234_567_890_123
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 16,
            "command": {"type": "loadMoreMatches"},
        }))
        .unwrap();
        assert_eq!(msg.command, Command::LoadMoreMatches);
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 16,
            "command": {"type": "openPlayerProfile", "puuid": "fake-puuid-0002"},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::OpenPlayerProfile {
                puuid: "fake-puuid-0002".into()
            }
        );
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 16,
            "command": {"type": "loadMorePlayerMatches"},
        }))
        .unwrap();
        assert_eq!(msg.command, Command::LoadMorePlayerMatches);
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 17,
            "command": {"type": "openPatchNotes"},
        }))
        .unwrap();
        assert_eq!(msg.command, Command::OpenPatchNotes);
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 18,
            "command": {"type": "openCollection"},
        }))
        .unwrap();
        assert_eq!(msg.command, Command::OpenCollection);
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 19,
            "command": {"type": "openChampionSkins", "championId": 266},
        }))
        .unwrap();
        assert_eq!(msg.command, Command::OpenChampionSkins { champion_id: 266 });
        let msg: ClientMessage = serde_json::from_value(json!({
            "id": 14,
            "command": {"type": "inviteToLobby", "invitees": [{"summonerId": 1003, "puuid": "p"}]},
        }))
        .unwrap();
        assert_eq!(
            msg.command,
            Command::InviteToLobby {
                invitees: vec![Invitee {
                    summoner_id: 1003,
                    puuid: "p".into()
                }]
            }
        );
        // Só as filas que o easylobby oferece.
        assert!(serde_json::from_value::<ClientMessage>(
            json!({"id": 1, "command": {"type": "createLobby", "queue": "tft"}})
        )
        .is_err());
        assert!(serde_json::from_value::<ClientMessage>(json!({"id": 1})).is_err());
        assert!(serde_json::from_value::<ClientMessage>(
            json!({"id": 1, "command": {"type": "comprarSkin"}})
        )
        .is_err());
    }
}
