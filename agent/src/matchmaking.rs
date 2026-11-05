//! Entrar e sair da fila (#19), com o tempo na fila.
//!
//! A busca de partida do lobby fica em dois lugares da LCU:
//!
//! - `POST /lol-lobby/v2/lobby/matchmaking/search` entra na fila com o lobby
//!   atual (só o líder pode) e `DELETE` no mesmo caminho sai dela. Os dois
//!   respondem `204` sem corpo.
//! - `GET /lol-matchmaking/v1/search` devolve a busca em andamento, ou `404`
//!   fora da fila. Durante a busca, a LCU manda um evento WAMP nesse URI a
//!   cada segundo, com `timeInQueue` subindo; quando a partida é encontrada,
//!   `searchState` vira `Found` (o ready check, #20, segue em
//!   `/lol-matchmaking/v1/ready-check`). A busca é apagada (`Delete`) quando
//!   o jogador sai da fila ou a partida começa.
//!
//! O [`Hub`](crate::ws::Hub) lê a busca pelo REST a cada conexão e depois
//! segue os eventos; a UI recebe o [`Matchmaking`] pelo `/ws` e pede as
//! mudanças com os comandos `startQueue` e `stopQueue`.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) da busca de partida.
pub const SEARCH_PATH: &str = "/lol-matchmaking/v1/search";

/// Entra (`POST`) e sai (`DELETE`) da fila com o lobby atual.
pub const LOBBY_SEARCH_PATH: &str = "/lol-lobby/v2/lobby/matchmaking/search";

/// A busca de partida em andamento: o que a tela de fila precisa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Matchmaking {
    pub search_state: SearchState,
    /// `queueId` da Riot da fila em que o jogador está.
    pub queue_id: u32,
    /// Segundos na fila, como a LCU conta. Chega de segundo em segundo; a UI
    /// pode contar sozinha entre um valor e outro.
    pub time_in_queue: f64,
    /// Tempo estimado de fila, em segundos.
    pub estimated_queue_time: f64,
    /// Segundos de fila de baixa prioridade que ainda faltam (punição por
    /// abandonar partidas). `0` sem punição.
    pub low_priority_time_remaining: f64,
    /// Por que a busca não anda (por exemplo, punição por sair de um ready
    /// check). Vazio quando está tudo certo.
    pub errors: Vec<MatchmakingError>,
}

/// Estado da busca, com os nomes da LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum SearchState {
    /// Procurando partida.
    Searching,
    /// Partida encontrada: o ready check está aberto ou já foi aceito.
    Found,
    /// A busca parou por um erro (veja [`Matchmaking::errors`]). Os nomes
    /// `ServiceError` e `ServiceShutdown` da LCU também viram este.
    Error,
    /// O jogador saiu da fila de baixa prioridade antes do fim da punição.
    AbandonedLowPriorityQueue,
    /// Estado que o agente não conhece.
    Unknown,
}

impl SearchState {
    /// Lê o `searchState` da LCU. `None` para os estados sem busca
    /// (`Invalid`, `Canceled`).
    fn from_lcu(state: &str) -> Option<SearchState> {
        Some(match state {
            "Invalid" | "Canceled" => return None,
            "Searching" => SearchState::Searching,
            "Found" => SearchState::Found,
            "Error" | "ServiceError" | "ServiceShutdown" => SearchState::Error,
            "AbandonedLowPriorityQueue" => SearchState::AbandonedLowPriorityQueue,
            _ => SearchState::Unknown,
        })
    }
}

/// Um erro da busca, como a LCU manda em `errors`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MatchmakingError {
    /// Código da Riot, como `QUEUE_DODGER` ou `LEAVER_BUSTED`.
    pub error_type: String,
    /// Texto da Riot; muitas vezes vazio.
    pub message: String,
    /// Segundos de punição que faltam, ou `0`.
    pub penalty_time_remaining: f64,
}

impl Matchmaking {
    /// Lê a busca do JSON da LCU (resposta do `GET` ou `data` de um evento).
    /// `None` quando não há busca (`searchState` `Invalid` ou `Canceled`) ou
    /// quando o valor não tem o formato esperado.
    pub fn from_json(value: &Value) -> Option<Matchmaking> {
        let raw = RawSearch::deserialize(value).ok()?;
        let search_state = SearchState::from_lcu(&raw.search_state)?;
        Some(Matchmaking {
            search_state,
            queue_id: raw.queue_id,
            time_in_queue: raw.time_in_queue,
            estimated_queue_time: raw.estimated_queue_time,
            low_priority_time_remaining: raw
                .low_priority_data
                .map_or(0.0, |d| d.penalty_time_remaining),
            errors: raw
                .errors
                .into_iter()
                .map(|e| MatchmakingError {
                    error_type: e.error_type,
                    message: e.message,
                    penalty_time_remaining: e.penalty_time_remaining,
                })
                .collect(),
        })
    }
}

/// A busca como a LCU manda, só com os campos usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSearch {
    search_state: String,
    #[serde(default)]
    queue_id: u32,
    #[serde(default)]
    time_in_queue: f64,
    #[serde(default)]
    estimated_queue_time: f64,
    #[serde(default)]
    low_priority_data: Option<RawLowPriority>,
    #[serde(default)]
    errors: Vec<RawError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLowPriority {
    #[serde(default)]
    penalty_time_remaining: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawError {
    #[serde(default)]
    error_type: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    penalty_time_remaining: f64,
}

/// Busca de partida atual. Fora da fila a LCU responde `404`, que volta como
/// `Ok(None)`.
pub async fn current_search(lcu: &LcuClient) -> Result<Option<Matchmaking>, LcuError> {
    match lcu.get::<Value>(SEARCH_PATH).await {
        Ok(body) => Ok(Matchmaking::from_json(&body)),
        Err(error) if error.status() == Some(404) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Entra na fila com o lobby atual.
pub async fn start_search(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.send::<Option<Value>>(Method::POST, LOBBY_SEARCH_PATH, None)
        .await
        .map(|_| ())
}

/// Sai da fila.
pub async fn stop_search(lcu: &LcuClient) -> Result<(), LcuError> {
    lcu.delete::<Option<Value>>(LOBBY_SEARCH_PATH)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_a_busca_gravada() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/16.22/rest.json");
        let rest: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let body = rest
            .iter()
            .find(|r| r["path"] == SEARCH_PATH && r["status"] == 200)
            .map(|r| &r["body"])
            .unwrap();
        let search = Matchmaking::from_json(body).unwrap();
        assert_eq!(search.search_state, SearchState::Searching);
        assert_eq!(search.queue_id, 420);
        assert_eq!(search.time_in_queue, 0.0);
        assert!((search.estimated_queue_time - 94.663).abs() < 0.001);
        assert_eq!(search.low_priority_time_remaining, 0.0);
        assert!(search.errors.is_empty());
    }

    #[test]
    fn estados_da_lcu() {
        let search = |state: &str| {
            Matchmaking::from_json(&json!({"searchState": state})).map(|s| s.search_state)
        };
        assert_eq!(search("Searching"), Some(SearchState::Searching));
        assert_eq!(search("Found"), Some(SearchState::Found));
        assert_eq!(search("ServiceError"), Some(SearchState::Error));
        assert_eq!(
            search("AbandonedLowPriorityQueue"),
            Some(SearchState::AbandonedLowPriorityQueue)
        );
        assert_eq!(search("EstadoNovo"), Some(SearchState::Unknown));
        assert_eq!(search("Invalid"), None);
        assert_eq!(search("Canceled"), None);
        assert_eq!(Matchmaking::from_json(&json!(null)), None);
        assert_eq!(
            Matchmaking::from_json(&json!({"errorCode": "RPC_ERROR"})),
            None
        );
    }

    #[test]
    fn erros_e_baixa_prioridade() {
        let search = Matchmaking::from_json(&json!({
            "searchState": "Error",
            "queueId": 400,
            "lowPriorityData": {"penaltyTimeRemaining": 120.0},
            "errors": [{
                "errorType": "QUEUE_DODGER",
                "id": 1,
                "message": "",
                "penalizedSummonerId": 1,
                "penaltyTimeRemaining": 300.5,
            }],
        }))
        .unwrap();
        assert_eq!(search.search_state, SearchState::Error);
        assert_eq!(search.low_priority_time_remaining, 120.0);
        assert_eq!(
            search.errors,
            [MatchmakingError {
                error_type: "QUEUE_DODGER".into(),
                message: String::new(),
                penalty_time_remaining: 300.5,
            }]
        );
    }

    #[test]
    fn formato_para_a_ui() {
        let search = Matchmaking {
            search_state: SearchState::Searching,
            queue_id: 420,
            time_in_queue: 12.0,
            estimated_queue_time: 90.0,
            low_priority_time_remaining: 0.0,
            errors: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(&search).unwrap(),
            json!({
                "searchState": "Searching",
                "queueId": 420,
                "timeInQueue": 12.0,
                "estimatedQueueTime": 90.0,
                "lowPriorityTimeRemaining": 0.0,
                "errors": [],
            })
        );
    }
}
