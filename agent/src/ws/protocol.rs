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
//! {"type":"lobby","lobby":{"queueId":420,"queue":"rankedSolo",...}}
//! {"type":"response","id":7,"result":{"status":"ok"}}
//! ```
//!
//! ```json
//! {"id":7,"command":{"type":"ping"}}
//! {"id":8,"command":{"type":"createLobby","queue":"rankedSolo"}}
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

use crate::lobby::{Lobby, LobbyMember, Queue};
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
        state: AgentState,
    },
    /// O estado da conexão com o LoL mudou.
    Lcu { status: LcuStatus },
    /// O invocador logado mudou. `null` quando o LoL fecha ou antes do login.
    Summoner { summoner: Option<Summoner> },
    /// A fase do gameflow mudou (`None`, `Lobby`, `Matchmaking`,
    /// `ReadyCheck`, `ChampSelect`, `InProgress`...). `null` com o LoL
    /// fechado.
    GameflowPhase { phase: Option<String> },
    /// O lobby mudou (criado, fila trocada, alguém entrou ou saiu). `null`
    /// fora de lobby e com o LoL fechado.
    Lobby { lobby: Option<Lobby> },
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
    pub gameflow_phase: Option<String>,
    pub lobby: Option<Lobby>,
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
        file::<Lobby>(&cfg)?,
        file::<LobbyMember>(&cfg)?,
        file::<Queue>(&cfg)?,
        file::<ClientMessage>(&cfg)?,
        file::<Command>(&cfg)?,
        file::<CommandResult>(&cfg)?,
        file::<ErrorCode>(&cfg)?,
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
            state: AgentState::default(),
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
                    "lobby": null,
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
            phase: Some("Lobby".into()),
        };
        assert_eq!(
            serde_json::to_value(&phase).unwrap(),
            json!({"type": "gameflowPhase", "phase": "Lobby"})
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
