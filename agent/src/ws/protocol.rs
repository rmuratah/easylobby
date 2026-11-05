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
//! {"type":"readyCheck","readyCheck":{"state":"InProgress","playerResponse":"None","timer":3.0,"duration":12.0}}
//! {"type":"lobby","lobby":{"queueId":420,"queue":"rankedSolo",...}}
//! {"type":"champSelect","champSelect":{"timer":{"phase":"BAN_PICK",...},"myTeam":[...],...}}
//! {"type":"receivedInvitations","invitations":[{"invitationId":"...","fromSummonerId":1,...}]}
//! {"type":"response","id":7,"result":{"status":"ok"}}
//! ```
//!
//! ```json
//! {"id":7,"command":{"type":"ping"}}
//! {"id":8,"command":{"type":"createLobby","queue":"rankedSolo"}}
//! {"id":9,"command":{"type":"confirmAction","actionId":13,"championId":121}}
//! ```
//!
//! Os tipos TypeScript em `ui/src/bindings/` saem destes tipos com `ts-rs`.
//! Depois de mudar qualquer tipo daqui, rode
//! `EASYLOBBY_UPDATE_BINDINGS=1 cargo test -p easylobby-agent bindings` e
//! comite os arquivos gerados. O teste `bindings_estao_atualizados` falha no
//! CI se alguém esquecer.
//!
//! Para mudar o formato de forma incompatível (tirar ou renomear um campo ou
//! uma mensagem), suba [`PROTOCOL_VERSION`]. Campo ou mensagem nova não
//! precisa: a UI ignora o que não conhece.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::assets::AssetKind;
use crate::champ_select::{
    ChampSelect, ChampSelectAction, ChampSelectActionType, ChampSelectBans, ChampSelectPhase,
    ChampSelectPlayer, ChampSelectTimer,
};
use crate::gameflow::GameflowPhase;
use crate::invitations::{Invitee, ReceivedInvitation};
use crate::lobby::{InvitationState, Lobby, LobbyInvitation, LobbyMember, Position, Queue};
use crate::matchmaking::{Matchmaking, MatchmakingError, SearchState};
use crate::ready_check::{ReadyCheck, ReadyCheckResponse, ReadyCheckState};
use crate::summoner::Summoner;

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
    /// O ready check (partida encontrada, #20) abriu, andou ou fechou. Chega
    /// mais ou menos a cada segundo enquanto ele está aberto, com o `timer`
    /// novo. `null` quando não há ready check (fechou, ou o LoL fechou).
    ReadyCheck { ready_check: Option<ReadyCheck> },
    /// O lobby mudou (criado, fila trocada, alguém entrou ou saiu). `null`
    /// fora de lobby e com o LoL fechado.
    Lobby { lobby: Option<Lobby> },
    /// A busca de partida mudou: entrou ou saiu da fila, mais um segundo na
    /// fila (chega a cada segundo enquanto procura), partida encontrada.
    /// `null` fora da fila e com o LoL fechado.
    Matchmaking { matchmaking: Option<Matchmaking> },
    /// A seleção de campeões (#22) abriu ou mudou: hover, ban, pick, troca
    /// de feitiço, troca de etapa ou de vez. Sempre o estado inteiro da
    /// seleção, não só o que mudou. `null` quando a seleção fecha (partida
    /// abrindo, dodge) e com o LoL fechado.
    ChampSelect { champ_select: Option<ChampSelect> },
    /// Os convites de lobby recebidos mudaram (#32): chegou um, ou um foi
    /// aceito, recusado ou cancelado. Sempre a lista inteira dos pendentes;
    /// vazia quando não há nenhum e com o LoL fechado.
    ReceivedInvitations {
        invitations: Vec<ReceivedInvitation>,
    },
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
    pub lcu: LcuStatus,
    pub summoner: Option<Summoner>,
    pub gameflow_phase: Option<GameflowPhase>,
    /// Ready check aberto (partida encontrada, #20), ou `null`.
    pub ready_check: Option<ReadyCheck>,
    pub lobby: Option<Lobby>,
    pub matchmaking: Option<Matchmaking>,
    /// Seleção de campeões em andamento (#22), ou `null`.
    pub champ_select: Option<ChampSelect>,
    /// Convites de lobby recebidos e ainda pendentes (#32).
    pub received_invitations: Vec<ReceivedInvitation>,
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
    /// Não faz nada; responde `ok`. Serve para testar o canal.
    Ping,
    /// Pede um [`ServerMessage::Hello`] novo com o estado inteiro, que chega
    /// antes da resposta.
    GetState,
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
    /// Cria um lobby na fila pedida, ou troca a fila do lobby atual (só o
    /// líder pode). O lobby novo chega numa mensagem `lobby` antes da
    /// resposta. Responde `notConnected` com o LoL fechado e `lcu` se a LCU
    /// recusar (fila fechada, jogador não é o líder, já está na fila...).
    CreateLobby { queue: Queue },
    /// Sai do lobby. A mensagem `lobby` com `null` chega antes da resposta.
    /// Responde `notConnected` com o LoL fechado e `lcu` se a LCU recusar.
    LeaveLobby,
    /// Escolhe o cargo primário e o secundário do jogador logado (#18). Só
    /// vale num lobby com `showPositionSelector`. O par segue as regras do
    /// client oficial: cargos diferentes, e `second` em `unselected` quando
    /// `first` é `fill` ou `unselected`; fora disso responde
    /// `invalidCommand` sem falar com a LCU. O lobby com os cargos novos
    /// chega numa mensagem `lobby` antes da resposta. Responde `notConnected`
    /// com o LoL fechado e `lcu` se a LCU recusar (fora de lobby, fila sem
    /// cargos...).
    SetPositions { first: Position, second: Position },
    /// Entra na fila com o lobby atual (só o líder pode). A mensagem
    /// `matchmaking` com a busca chega antes da resposta. Responde
    /// `notConnected` com o LoL fechado e `lcu` se a LCU recusar (fora de
    /// lobby, não é o líder, punição de fila...).
    StartQueue,
    /// Sai da fila. A mensagem `matchmaking` com `null` chega antes da
    /// resposta. Responde `notConnected` com o LoL fechado e `lcu` se a LCU
    /// recusar.
    StopQueue,
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
}

impl CommandResult {
    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        CommandResult::Error {
            code,
            message: message.into(),
        }
    }
}

/// Gera os arquivos `.ts` de todos os tipos do protocolo: `(nome do arquivo,
/// conteúdo)`. Os tipos que entram por dependência (como [`Summoner`]) estão
/// na lista também.
pub fn typescript_bindings() -> Result<Vec<(String, String)>, ts_rs::ExportError> {
    fn file<T: TS + 'static>(cfg: &ts_rs::Config) -> Result<(String, String), ts_rs::ExportError> {
        let path = T::output_path()
            .ok_or_else(|| ts_rs::ExportError::CannotBeExported(std::any::type_name::<T>()))?;
        let name = path.to_string_lossy().replace('\\', "/");
        Ok((name, T::export_to_string(cfg)?))
    }
    let cfg = ts_rs::Config::default();
    let mut files = vec![
        file::<ServerMessage>(&cfg)?,
        file::<AgentState>(&cfg)?,
        file::<LcuStatus>(&cfg)?,
        file::<Summoner>(&cfg)?,
        file::<GameflowPhase>(&cfg)?,
        file::<ReadyCheck>(&cfg)?,
        file::<ReadyCheckState>(&cfg)?,
        file::<ReadyCheckResponse>(&cfg)?,
        file::<Lobby>(&cfg)?,
        file::<LobbyInvitation>(&cfg)?,
        file::<InvitationState>(&cfg)?,
        file::<ReceivedInvitation>(&cfg)?,
        file::<Invitee>(&cfg)?,
        file::<LobbyMember>(&cfg)?,
        file::<Position>(&cfg)?,
        file::<Queue>(&cfg)?,
        file::<Matchmaking>(&cfg)?,
        file::<MatchmakingError>(&cfg)?,
        file::<SearchState>(&cfg)?,
        file::<ChampSelect>(&cfg)?,
        file::<ChampSelectTimer>(&cfg)?,
        file::<ChampSelectPhase>(&cfg)?,
        file::<ChampSelectPlayer>(&cfg)?,
        file::<ChampSelectBans>(&cfg)?,
        file::<ChampSelectAction>(&cfg)?,
        file::<ChampSelectActionType>(&cfg)?,
        file::<ClientMessage>(&cfg)?,
        file::<Command>(&cfg)?,
        file::<CommandResult>(&cfg)?,
        file::<ErrorCode>(&cfg)?,
        file::<AssetKind>(&cfg)?,
    ];
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    "matchmaking": null,
                    "champSelect": null,
                    "receivedInvitations": [],
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
                    "duration": 12.0,
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
            ("declineReadyCheck", Command::DeclineReadyCheck),
        ] {
            let msg: ClientMessage =
                serde_json::from_value(json!({"id": 9, "command": {"type": name}})).unwrap();
            assert_eq!(msg.command, command);
        }
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
        let msg: ClientMessage =
            serde_json::from_value(json!({"id": 10, "command": {"type": "startQueue"}})).unwrap();
        assert_eq!(msg.command, Command::StartQueue);
        let msg: ClientMessage =
            serde_json::from_value(json!({"id": 11, "command": {"type": "stopQueue"}})).unwrap();
        assert_eq!(msg.command, Command::StopQueue);
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

    /// Compara os tipos gerados com os arquivos em `ui/src/bindings/`. Com
    /// `EASYLOBBY_UPDATE_BINDINGS=1`, regrava os arquivos em vez de comparar.
    #[test]
    fn bindings_estao_atualizados() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/bindings");
        let files = typescript_bindings().unwrap();
        if std::env::var_os("EASYLOBBY_UPDATE_BINDINGS").is_some() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|e| e == "ts") {
                    std::fs::remove_file(path).unwrap();
                }
            }
            for (name, content) in &files {
                std::fs::write(dir.join(name), content).unwrap();
            }
            return;
        }

        let hint = "rode `EASYLOBBY_UPDATE_BINDINGS=1 cargo test -p easylobby-agent bindings` \
                    e comite ui/src/bindings/";
        let mut committed: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".ts"))
            .collect();
        committed.sort();
        let expected: Vec<String> = files.iter().map(|(name, _)| name.clone()).collect();
        assert_eq!(
            committed, expected,
            "arquivos diferentes em ui/src/bindings: {hint}"
        );
        for (name, content) in &files {
            // O checkout no Windows pode trocar \n por \r\n.
            let on_disk = std::fs::read_to_string(dir.join(name))
                .unwrap()
                .replace("\r\n", "\n");
            assert_eq!(&on_disk, content, "{name} está desatualizado: {hint}");
        }
    }
}
