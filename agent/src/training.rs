//! Aba Treinamento da tela de escolha de modo (#209): Tutorial e Modo de
//! Treino.
//!
//! - **Tutorial:** são três filas escondidas (`isVisible: false`), uma por
//!   parte: 2000, 2010 e 2020. O agente lê o progresso do jogador em
//!   [`TUTORIALS_PATH`] (o caminho de iniciante do client oficial), cria o
//!   lobby na fila da próxima parte que falta e entra na busca, como o client
//!   faz. Com a escolha de parte (#228), a UI mostra as três partes em
//!   cards ([`TutorialPart`]) e o comando de iniciar recebe a fila da parte.
//! - **Modo de Treino:** é uma personalizada (`PRACTICETOOL`, fila 3140 na
//!   lista da LCU, com `isCustom`). O agente cria a personalizada no
//!   Summoner's Rift, com uma senha para ninguém de fora entrar, e deixa o
//!   jogador no lobby da personalizada, com as equipes e os bots (#229, veja
//!   [`crate::custom_lobby`]). A partida começa pelo comando de iniciar.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::lobby::{Lobby, LOBBY_PATH};
use crate::rest::{LcuClient, LcuError};

/// Filas das Partes 1, 2 e 3 do Tutorial, em ordem.
pub const TUTORIAL_QUEUES: [u32; 3] = [2000, 2010, 2020];

/// Progresso do Tutorial (caminho de iniciante do client oficial).
pub const TUTORIALS_PATH: &str = "/lol-npe-tutorial-path/v1/tutorials";

/// Inicia a seleção de campeões de uma personalizada.
pub const CUSTOM_START_CHAMP_SELECT_PATH: &str = "/lol-lobby/v1/lobby/custom/start-champ-select";

/// Modo de jogo do Modo de Treino.
pub const PRACTICE_TOOL_GAME_MODE: &str = "PRACTICETOOL";

/// Uma parte do Tutorial, como os cards do client oficial mostram (#228).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TutorialPart {
    /// Fila da parte: 2000, 2010 ou 2020.
    pub queue_id: u32,
    /// Número da parte: 1, 2 ou 3.
    pub part: u32,
    /// Se o jogador já terminou a parte. Dá para jogar de novo.
    pub completed: bool,
    /// Se dá para jogar a parte: a Parte 1 sempre, e as outras depois que a
    /// anterior foi concluída (o cadeado do client oficial).
    pub available: bool,
}

/// Se a fila `queue_id` está `COMPLETED` na resposta de [`TUTORIALS_PATH`].
///
/// Cada item da lista traz a fila em `queueId` (número ou texto) e o
/// progresso em `status`; o agente aceita os dois formatos porque o
/// endpoint não está documentado.
fn is_completed(items: &[Value], queue_id: u32) -> bool {
    items.iter().any(|item| {
        let id = match item.get("queueId") {
            Some(Value::Number(n)) => n.as_u64(),
            Some(Value::String(s)) => s.trim().parse().ok(),
            _ => None,
        };
        let done = item
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|s| s.eq_ignore_ascii_case("COMPLETED"));
        id == Some(u64::from(queue_id)) && done
    })
}

/// As três partes do Tutorial a partir da resposta de [`TUTORIALS_PATH`].
/// `None` quando a resposta não é uma lista: sem saber o progresso, a UI
/// deixa escolher qualquer parte e a LCU decide.
pub fn tutorial_parts(value: &Value) -> Option<Vec<TutorialPart>> {
    let items = value.as_array()?;
    let mut previous_done = true;
    Some(
        TUTORIAL_QUEUES
            .into_iter()
            .zip(1..)
            .map(|(queue_id, part)| {
                let completed = is_completed(items, queue_id);
                let available = previous_done;
                previous_done = completed;
                TutorialPart {
                    queue_id,
                    part,
                    completed,
                    available,
                }
            })
            .collect(),
    )
}

/// Lê o progresso do Tutorial (#228). `None` se a LCU não responder ou
/// responder num formato desconhecido.
pub async fn tutorial_progress(lcu: &LcuClient) -> Option<Vec<TutorialPart>> {
    let body = lcu.get::<Value>(TUTORIALS_PATH).await.ok()?;
    tutorial_parts(&body)
}

/// Fila da próxima parte do Tutorial, a partir da resposta de
/// [`TUTORIALS_PATH`]: a primeira parte de [`TUTORIAL_QUEUES`] que não está
/// `COMPLETED`. Sem informação que dê para usar, ou com as três feitas, a
/// Parte 1.
///
/// Cada item da lista traz a fila em `queueId` (número ou texto) e o
/// progresso em `status`; o agente aceita os dois formatos porque o
/// endpoint não está documentado.
pub fn next_tutorial_queue(value: &Value) -> u32 {
    let Some(items) = value.as_array() else {
        return TUTORIAL_QUEUES[0];
    };
    TUTORIAL_QUEUES
        .into_iter()
        .find(|&queue_id| !is_completed(items, queue_id))
        .unwrap_or(TUTORIAL_QUEUES[0])
}

/// Busca a fila da próxima parte do Tutorial. Se a LCU não responder o
/// progresso, a Parte 1.
pub async fn next_tutorial(lcu: &LcuClient) -> u32 {
    match lcu.get::<Value>(TUTORIALS_PATH).await {
        Ok(body) => next_tutorial_queue(&body),
        Err(_) => TUTORIAL_QUEUES[0],
    }
}

/// Corpo do `POST /lol-lobby/v2/lobby` que cria a personalizada do Modo de
/// Treino, com a senha `password`.
pub fn practice_tool_body(password: &str) -> Value {
    json!({
        "customGameLobby": {
            "configuration": {
                "gameMode": PRACTICE_TOOL_GAME_MODE,
                "gameMutator": "",
                "gameServerRegion": "",
                "mapId": 11,
                "mutators": { "id": 1 },
                "spectatorPolicy": "NotAllowed",
                "teamSize": 5
            },
            "lobbyName": "Modo de Treino",
            "lobbyPassword": password
        },
        "isCustom": true
    })
}

/// Senha da personalizada do Modo de Treino. Não protege nada sensível, só
/// evita que alguém da lista de personalizadas entre.
fn practice_tool_password() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("el{:x}", nanos ^ (u128::from(std::process::id()) << 64))
}

/// Cria a personalizada do Modo de Treino. Devolve o lobby que a LCU
/// respondeu.
pub async fn create_practice_tool(lcu: &LcuClient) -> Result<Option<Lobby>, LcuError> {
    let body: Value = lcu
        .post(LOBBY_PATH, &practice_tool_body(&practice_tool_password()))
        .await?;
    Ok(Lobby::from_json(&body))
}

/// Inicia a seleção de campeões da personalizada atual.
pub async fn start_custom_champ_select(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.post::<Value, Option<Value>>(CUSTOM_START_CHAMP_SELECT_PATH, &json!({}))
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxima_parte_e_a_primeira_nao_feita() {
        let progress = json!([
            {"queueId": 2000, "status": "COMPLETED"},
            {"queueId": "2010", "status": "UNLOCKED"},
            {"queueId": 2020, "status": "LOCKED"},
        ]);
        assert_eq!(next_tutorial_queue(&progress), 2010);
        let two_done = json!([
            {"queueId": "2000", "status": "completed"},
            {"queueId": 2010, "status": "COMPLETED"},
        ]);
        assert_eq!(next_tutorial_queue(&two_done), 2020);
    }

    #[test]
    fn sem_informacao_ou_tudo_feito_comeca_da_parte_1() {
        assert_eq!(next_tutorial_queue(&json!(null)), 2000);
        assert_eq!(next_tutorial_queue(&json!({"erro": 1})), 2000);
        assert_eq!(next_tutorial_queue(&json!([])), 2000);
        let all = json!(TUTORIAL_QUEUES
            .iter()
            .map(|id| json!({"queueId": id, "status": "COMPLETED"}))
            .collect::<Vec<_>>());
        assert_eq!(next_tutorial_queue(&all), 2000);
    }

    #[test]
    fn partes_com_cadeado_ate_a_anterior_ser_concluida() {
        let progress = json!([
            {"queueId": 2000, "status": "COMPLETED"},
            {"queueId": "2010", "status": "UNLOCKED"},
            {"queueId": 2020, "status": "LOCKED"},
        ]);
        let parts = tutorial_parts(&progress).unwrap();
        let seen: Vec<(u32, u32, bool, bool)> = parts
            .iter()
            .map(|p| (p.queue_id, p.part, p.completed, p.available))
            .collect();
        assert_eq!(
            seen,
            [
                (2000, 1, true, true),
                (2010, 2, false, true),
                (2020, 3, false, false)
            ]
        );
        let none = tutorial_parts(&json!([])).unwrap();
        assert_eq!(
            none.iter().map(|p| p.available).collect::<Vec<_>>(),
            [true, false, false]
        );
        let all = json!(TUTORIAL_QUEUES
            .iter()
            .map(|id| json!({"queueId": id, "status": "COMPLETED"}))
            .collect::<Vec<_>>());
        assert!(tutorial_parts(&all)
            .unwrap()
            .iter()
            .all(|p| p.completed && p.available));
        assert_eq!(tutorial_parts(&json!({"erro": 1})), None);
        assert_eq!(
            serde_json::to_value(parts[0]).unwrap(),
            json!({"queueId": 2000, "part": 1, "completed": true, "available": true})
        );
    }

    #[test]
    fn personalizada_de_treino_no_summoners_rift_com_senha() {
        let body = practice_tool_body("segredo");
        let config = &body["customGameLobby"]["configuration"];
        assert_eq!(config["gameMode"], "PRACTICETOOL");
        assert_eq!(config["mapId"], 11);
        assert_eq!(body["customGameLobby"]["lobbyPassword"], "segredo");
        assert_eq!(body["isCustom"], true);
        assert!(practice_tool_password().len() > 2);
    }
}
