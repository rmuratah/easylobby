//! Modo de compatibilidade: checagem de saúde dos endpoints da LCU (#36).
//!
//! Quando o LoL atualiza, a Riot às vezes tira, renomeia ou muda o formato
//! de um endpoint da LCU. Sem checagem, a função que usa esse endpoint falha
//! sem explicação (um botão que não faz nada, uma tela que nunca carrega).
//!
//! Depois de conectar e logar, o agente faz um `GET` em cada endpoint de
//! [`PROBES`]. Cada um pertence a uma [`Feature`]. Se algum falhar, só a
//! função dele é desligada: o agente recusa os comandos dela com
//! `featureUnavailable` e a UI troca a tela dela por um aviso com o botão de
//! abrir o client oficial. O resto do app continua funcionando.
//!
//! Só conta como falha o que mostra que o endpoint mudou:
//!
//! - [`EndpointProblem::Missing`]: a LCU responde 404 com `errorCode`
//!   `RESOURCE_NOT_FOUND` ("Invalid URI format"), que é como ela responde a
//!   um caminho que não existe. Um 404 com `RPC_ERROR` ("LOBBY_NOT_FOUND",
//!   "No active delegate"...) só quer dizer que o recurso não existe agora
//!   (fora de lobby, fora da seleção) e conta como saudável. As duas formas
//!   estão nas gravações do 16.22 em `agent/tests/fixtures/`.
//! - [`EndpointProblem::UnexpectedFormat`]: a LCU responde 200, mas com um
//!   JSON de outro formato (uma lista no lugar de um objeto, por exemplo).
//!
//! Erro de rede, timeout, 5xx e os outros status não desligam nada: podem
//! ser passageiros, e o comando que falhar mostra o erro dele.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Uma função do app que depende de endpoints da LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Feature {
    /// Lobby, fila, convites e personalizadas.
    Lobby,
    /// Partida encontrada: aceitar e recusar.
    ReadyCheck,
    /// Seleção de campeões: bans, picks, feitiços, skins, trocas e chat.
    ChampSelect,
    /// Páginas de runas.
    Runes,
    /// Amigos e conversas.
    Social,
    /// Perfil: ranks, maestria e histórico.
    Profile,
    /// Pós-jogo: honra e resultado da partida.
    PostGame,
}

/// O que se espera do JSON de um `200`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Object,
    Array,
    /// Qualquer JSON.
    Any,
}

impl Shape {
    fn matches(self, value: &Value) -> bool {
        match self {
            Shape::Object => value.is_object(),
            Shape::Array => value.is_array(),
            Shape::Any => true,
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Shape::Object => "um objeto",
            Shape::Array => "uma lista",
            Shape::Any => "JSON",
        }
    }
}

/// Um endpoint checado e a função que depende dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    pub feature: Feature,
    pub path: &'static str,
    pub shape: Shape,
}

const fn probe(feature: Feature, path: &'static str, shape: Shape) -> Probe {
    Probe {
        feature,
        path,
        shape,
    }
}

/// Os endpoints checados ao conectar. Só `GET`s que o agente já faz e que a
/// LCU responde em qualquer fase (200 ou 404 com `RPC_ERROR`). Ao usar um
/// endpoint novo numa função, acrescente-o aqui.
pub const PROBES: &[Probe] = &[
    probe(Feature::Lobby, crate::lobby::LOBBY_PATH, Shape::Object),
    probe(
        Feature::Lobby,
        crate::queues::GAME_QUEUES_PATH,
        Shape::Array,
    ),
    probe(
        Feature::Lobby,
        crate::matchmaking::SEARCH_PATH,
        Shape::Object,
    ),
    probe(
        Feature::Lobby,
        crate::invitations::RECEIVED_INVITATIONS_PATH,
        Shape::Array,
    ),
    probe(
        Feature::ReadyCheck,
        crate::ready_check::READY_CHECK_PATH,
        Shape::Object,
    ),
    probe(
        Feature::ChampSelect,
        crate::champ_select::CHAMP_SELECT_SESSION_PATH,
        Shape::Object,
    ),
    probe(
        Feature::ChampSelect,
        crate::champions::CHAMPION_SUMMARY_PATH,
        Shape::Array,
    ),
    probe(
        Feature::ChampSelect,
        crate::summoner_spells::SUMMONER_SPELLS_PATH,
        Shape::Array,
    ),
    probe(Feature::Runes, crate::runes::PAGES_PATH, Shape::Array),
    probe(
        Feature::Runes,
        crate::runes::PERK_STYLES_PATH,
        Shape::Object,
    ),
    probe(Feature::Runes, crate::runes::PERKS_PATH, Shape::Array),
    probe(Feature::Social, crate::friends::FRIENDS_PATH, Shape::Array),
    probe(
        Feature::Social,
        crate::chat::CONVERSATIONS_PATH,
        Shape::Array,
    ),
    probe(
        Feature::Profile,
        crate::ranked::RANKED_STATS_PATH,
        Shape::Object,
    ),
    probe(
        Feature::Profile,
        crate::mastery::CHAMPION_MASTERY_PATH,
        Shape::Array,
    ),
    probe(Feature::PostGame, crate::honor::BALLOT_PATH, Shape::Object),
    probe(
        Feature::PostGame,
        crate::end_of_game::EOG_STATS_PATH,
        Shape::Object,
    ),
];

/// Como um endpoint falhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum EndpointProblem {
    /// A LCU não conhece mais o caminho.
    Missing,
    /// A LCU respondeu, mas num formato que o agente não entende.
    UnexpectedFormat,
}

/// Uma função desligada porque um endpoint dela falhou na checagem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UnavailableFeature {
    pub feature: Feature,
    /// O primeiro endpoint da função que falhou.
    pub path: String,
    pub problem: EndpointProblem,
    /// Detalhe técnico do erro, para o log e para quem reportar o problema.
    pub detail: String,
}

/// Classifica a resposta de um endpoint. `None` é saudável (ou falha que
/// não diz nada sobre o endpoint, como erro de rede).
pub fn classify(
    result: &Result<Value, LcuError>,
    shape: Shape,
) -> Option<(EndpointProblem, String)> {
    match result {
        Ok(value) if shape.matches(value) => None,
        Ok(value) => Some((
            EndpointProblem::UnexpectedFormat,
            format!("esperava {}, veio {}", shape.describe(), json_kind(value)),
        )),
        Err(LcuError::Status {
            status: 404, body, ..
        }) if body.get("errorCode").and_then(Value::as_str) == Some("RESOURCE_NOT_FOUND") => {
            Some((
                EndpointProblem::Missing,
                result.as_ref().unwrap_err().to_string(),
            ))
        }
        Err(LcuError::Decode { .. }) => Some((
            EndpointProblem::UnexpectedFormat,
            result.as_ref().unwrap_err().to_string(),
        )),
        Err(_) => None,
    }
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "vazio",
        Value::Bool(_) => "um booleano",
        Value::Number(_) => "um número",
        Value::String(_) => "um texto",
        Value::Array(_) => "uma lista",
        Value::Object(_) => "um objeto",
    }
}

/// Checa `probes` ao mesmo tempo e devolve as funções com algum endpoint
/// quebrado, uma vez cada, na ordem de `probes`.
pub async fn check(client: &LcuClient, probes: &[Probe]) -> Vec<UnavailableFeature> {
    let results = futures_util::future::join_all(
        probes
            .iter()
            .map(|probe| async move { (probe, client.get::<Value>(probe.path).await) }),
    )
    .await;
    let mut unavailable: Vec<UnavailableFeature> = Vec::new();
    for (probe, result) in results {
        let Some((problem, detail)) = classify(&result, probe.shape) else {
            continue;
        };
        if unavailable.iter().any(|u| u.feature == probe.feature) {
            continue;
        }
        unavailable.push(UnavailableFeature {
            feature: probe.feature,
            path: probe.path.to_owned(),
            problem,
            detail,
        });
    }
    unavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Method;
    use serde_json::json;

    fn status(status: u16, body: Value) -> Result<Value, LcuError> {
        Err(LcuError::Status {
            method: Method::GET,
            path: "/x".into(),
            status,
            message: body
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned),
            body,
        })
    }

    #[test]
    fn caminho_que_nao_existe_desliga() {
        // Como a LCU do 16.22 responde a um caminho que ela não conhece.
        let result = status(
            404,
            json!({"errorCode": "RESOURCE_NOT_FOUND", "httpStatus": 404, "message": "Invalid URI format"}),
        );
        let (problem, detail) = classify(&result, Shape::Object).unwrap();
        assert_eq!(problem, EndpointProblem::Missing);
        assert!(detail.contains("Invalid URI format"), "{detail}");
    }

    #[test]
    fn recurso_que_nao_existe_agora_e_saudavel() {
        for message in ["LOBBY_NOT_FOUND", "No active delegate"] {
            let result = status(
                404,
                json!({"errorCode": "RPC_ERROR", "httpStatus": 404, "message": message}),
            );
            assert_eq!(classify(&result, Shape::Object), None, "{message}");
        }
    }

    #[test]
    fn outros_erros_nao_desligam() {
        assert_eq!(classify(&status(500, json!({})), Shape::Object), None);
        assert_eq!(classify(&status(401, json!({})), Shape::Object), None);
    }

    #[test]
    fn formato_diferente_desliga() {
        assert_eq!(classify(&Ok(json!({})), Shape::Object), None);
        assert_eq!(classify(&Ok(json!([])), Shape::Array), None);
        assert_eq!(classify(&Ok(json!("x")), Shape::Any), None);
        let (problem, detail) = classify(&Ok(json!({"a": 1})), Shape::Array).unwrap();
        assert_eq!(problem, EndpointProblem::UnexpectedFormat);
        assert_eq!(detail, "esperava uma lista, veio um objeto");
    }

    #[test]
    fn toda_funcao_tem_endpoint() {
        for feature in [
            Feature::Lobby,
            Feature::ReadyCheck,
            Feature::ChampSelect,
            Feature::Runes,
            Feature::Social,
            Feature::Profile,
            Feature::PostGame,
        ] {
            assert!(
                PROBES.iter().any(|p| p.feature == feature),
                "{feature:?} sem endpoint"
            );
        }
    }
}
