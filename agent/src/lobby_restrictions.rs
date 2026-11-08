//! Por que a fila não pode começar (#227).
//!
//! Quando o grupo não pode entrar na fila, a LCU manda em `restrictions` do
//! lobby um item por motivo, com o código em `restrictionCode` e quem está
//! bloqueado em `puuids`. Quem decide as regras de tamanho e elegibilidade é
//! a LCU; o agente não copia essas regras, só repassa o motivo e um texto em
//! português para os códigos conhecidos. Para um código sem texto, a UI
//! mostra uma mensagem genérica.
//!
//! Só `QPInvalidNumberOfPlayerSlotsRestriction` aparece nas gravações
//! (16.22). Os outros códigos de [`message`] são os nomes usados pela LCU que
//! a comunidade conhece; um nome que mudar num patch só perde o texto.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Um motivo para a fila não poder começar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LobbyRestriction {
    /// `restrictionCode` da LCU, por exemplo
    /// `QPInvalidNumberOfPlayerSlotsRestriction`.
    pub code: String,
    /// Texto em português do motivo, ou `null` para um código sem texto
    /// conhecido.
    pub message: Option<String>,
    /// Quem está bloqueado. Vazia quando o motivo é do grupo todo.
    pub puuids: Vec<String>,
}

/// Texto em português de um `restrictionCode` conhecido.
pub fn message(code: &str) -> Option<&'static str> {
    Some(match code {
        "QPInvalidNumberOfPlayerSlotsRestriction" => {
            "Escolha as duas posições e um campeão para cada uma."
        }
        "QPScarcePositionsNotAvailableRestriction" => {
            "Uma das posições prioritárias precisa estar entre as escolhidas."
        }
        "TeamMinSizeRestriction" => "O grupo tem poucos jogadores para esta fila.",
        "TeamMaxSizeRestriction" => "O grupo tem jogadores demais para esta fila.",
        "TeamSizeRestriction" => "Esta fila não aceita grupos deste tamanho.",
        "TeamDivisionRestriction" | "TeamSkillRestriction" => {
            "A diferença de elo entre os jogadores do grupo é grande demais."
        }
        "TeamHighMMRMaxSizeRestriction" => "O grupo é grande demais para o elo dos jogadores.",
        "PlayerLevelRestriction" | "PlayerMinLevelRestriction" => {
            "Algum jogador não tem o nível necessário para esta fila."
        }
        "PlayerMaxLevelRestriction" => "Algum jogador passou do nível máximo desta fila.",
        "PlayerAvailableChampionRestriction" => {
            "Algum jogador não tem campeões suficientes para esta fila."
        }
        "PlayerBannedRestriction" | "PlayerRankedSuspensionRestriction" => {
            "Algum jogador está suspenso desta fila."
        }
        "PlayerLeaverBustedRestriction"
        | "PlayerLeaverQueueLockoutRestriction"
        | "PlayerLeaverTaintedWarningRestriction"
        | "PlayerTimedRestriction" => "Algum jogador está com punição de fila.",
        "PlayerInGameRestriction" => "Algum jogador ainda está em uma partida.",
        "PlayerPreRankedRestriction" | "PrerequisiteQueuesNotPlayedRestriction" => {
            "Algum jogador ainda precisa jogar outras filas antes desta."
        }
        "PlayerMinorRestriction" => "Algum jogador não pode jogar agora pelo controle de horário.",
        "QueueDisabled" | "QueueUnsupported" | "QueueUnavailable" => {
            "Esta fila não está disponível agora."
        }
        "GameVersionMismatch" | "GameVersionMissing" | "GameVersionNotSupported" => {
            "Algum jogador está com o jogo desatualizado."
        }
        _ => return None,
    })
}

/// Lê `restrictions` do lobby da LCU. Um item sem código fica de fora.
pub fn from_json(value: &Value) -> Vec<LobbyRestriction> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let code = item.get("restrictionCode")?.as_str()?.trim();
            if code.is_empty() {
                return None;
            }
            let puuids = item
                .get("puuids")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .filter(|p| !p.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            Some(LobbyRestriction {
                code: code.to_owned(),
                message: message(code).map(str::to_owned),
                puuids,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_a_restricao_gravada() {
        let restrictions = from_json(&json!([{
            "expiredTimestamp": 0,
            "puuids": ["fake-puuid-0001"],
            "puuidsString": "fake-puuid-0001",
            "restrictionArgs": {},
            "restrictionCode": "QPInvalidNumberOfPlayerSlotsRestriction",
            "summonerIds": [31056838],
        }]));
        assert_eq!(
            restrictions,
            [LobbyRestriction {
                code: "QPInvalidNumberOfPlayerSlotsRestriction".into(),
                message: Some("Escolha as duas posições e um campeão para cada uma.".into()),
                puuids: vec!["fake-puuid-0001".into()],
            }]
        );
    }

    #[test]
    fn codigo_desconhecido_vai_sem_texto() {
        let restrictions = from_json(&json!([
            {"restrictionCode": "RestricaoDeUmPatchNovo"},
            {"restrictionCode": ""},
            {"puuids": ["x"]},
        ]));
        assert_eq!(restrictions.len(), 1);
        assert_eq!(restrictions[0].code, "RestricaoDeUmPatchNovo");
        assert_eq!(restrictions[0].message, None);
        assert!(restrictions[0].puuids.is_empty());
        assert!(from_json(&json!(null)).is_empty());
    }

    #[test]
    fn formato_para_a_ui() {
        let restriction = LobbyRestriction {
            code: "TeamMinSizeRestriction".into(),
            message: message("TeamMinSizeRestriction").map(str::to_owned),
            puuids: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(&restriction).unwrap(),
            json!({
                "code": "TeamMinSizeRestriction",
                "message": "O grupo tem poucos jogadores para esta fila.",
                "puuids": [],
            })
        );
    }
}
