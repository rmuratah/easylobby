//! Link para outra pessoa entrar no lobby (#226), o mesmo do botão "copiar
//! link" do client oficial.
//!
//! O link é um código de convite com validade, não o `partyId`. A LCU guarda
//! o código na "atividade" (AGS) do lobby:
//!
//! - `GET /lol-lobby/v2/ags/agsActivityId` devolve o id da atividade (uma
//!   string) do lobby atual;
//! - `GET /lol-lobby/v2/ags/{id}/joinCode` devolve o código atual;
//! - `POST /lol-lobby/v2/ags/{id}/joinCode` gera um código novo.
//!
//! As duas rotas de código respondem `LolLobbyAgsSharedJoinCodeDto`:
//! `joinCode`, `smartUrl` (o link pronto, como
//! `https://gg.riotgames.com/LOL?joinCode=XXXX-XXXX-XXXX`), `expiresAt` e
//! `isActive`. A unidade do `expiresAt` ainda não foi conferida no LoL real;
//! [`expires_at_ms`] aceita segundos ou milissegundos.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Id da atividade (AGS) do lobby atual.
pub const AGS_ACTIVITY_ID_PATH: &str = "/lol-lobby/v2/ags/agsActivityId";

/// Endereço base do link, para montar quando a LCU não manda `smartUrl`.
pub const SMART_URL_BASE: &str = "https://gg.riotgames.com/LOL?joinCode=";

/// Um código ativo que vence em menos que isso é trocado por um novo, para
/// a pessoa que recebe o link ter tempo de abrir.
const MIN_REMAINING_MS: u64 = 60_000;

/// O link para entrar no lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LobbyLink {
    /// O link para copiar (`smartUrl`).
    pub url: String,
    /// O código do convite (`joinCode`), já dentro do `url`.
    pub join_code: String,
    /// Quando o código vence, em milissegundos desde 1970 (UTC). `null` se
    /// a LCU não disse.
    #[ts(type = "number | null")]
    pub expires_at: Option<u64>,
}

/// Caminho do código de convite da atividade `activity_id`.
pub fn join_code_path(activity_id: &str) -> String {
    format!("/lol-lobby/v2/ags/{}/joinCode", encode_segment(activity_id))
}

/// O link do lobby atual: reaproveita o código ativo, ou gera um novo se não
/// há código, ele foi desativado ou está para vencer. `Ok(None)` quando a
/// LCU respondeu sem atividade ou sem código (fora de lobby, por exemplo).
pub async fn create(lcu: &LcuClient) -> Result<Option<LobbyLink>, LcuError> {
    let activity: Value = lcu.get(AGS_ACTIVITY_ID_PATH).await?;
    let Some(activity_id) = activity.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let path = join_code_path(activity_id);
    let now = now_ms();
    // Sem código ainda, a LCU responde erro: aí só falta gerar.
    if let Ok(current) = lcu.get::<Value>(&path).await {
        if let Some(link) = usable(&current, now) {
            return Ok(Some(link));
        }
    }
    let created: Value = lcu.post(&path, &json!({})).await?;
    Ok(from_json(&created))
}

/// O código de `body` se ele ainda vale por pelo menos [`MIN_REMAINING_MS`].
fn usable(body: &Value, now: u64) -> Option<LobbyLink> {
    if body.get("isActive").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let link = from_json(body)?;
    match link.expires_at {
        Some(at) if at < now.saturating_add(MIN_REMAINING_MS) => None,
        _ => Some(link),
    }
}

/// Lê um `LolLobbyAgsSharedJoinCodeDto`. Sem `joinCode` não há link.
pub fn from_json(body: &Value) -> Option<LobbyLink> {
    let join_code = body
        .get("joinCode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_owned();
    let url = body
        .get("smartUrl")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(|| format!("{SMART_URL_BASE}{join_code}"), str::to_owned);
    let expires_at = body
        .get("expiresAt")
        .and_then(Value::as_u64)
        .and_then(expires_at_ms);
    Some(LobbyLink {
        url,
        join_code,
        expires_at,
    })
}

/// `expiresAt` em milissegundos. Um número pequeno demais para ser
/// milissegundos de agora em diante (antes de 2001) é lido como segundos.
/// `0` é "sem validade".
pub fn expires_at_ms(raw: u64) -> Option<u64> {
    match raw {
        0 => None,
        r if r < 1_000_000_000_000 => r.checked_mul(1000),
        r => Some(r),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Escapa `segment` para ir num pedaço do caminho: só letras, números e
/// `-._~` passam como estão.
fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_codigo_da_lcu() {
        let link = from_json(&json!({
            "joinCode": "ABCD-EFGH-IJKL",
            "smartUrl": "https://gg.riotgames.com/LOL?joinCode=ABCD-EFGH-IJKL",
            "expiresAt": 1_900_000_000_000u64,
            "isActive": true,
        }))
        .unwrap();
        assert_eq!(
            link,
            LobbyLink {
                url: "https://gg.riotgames.com/LOL?joinCode=ABCD-EFGH-IJKL".into(),
                join_code: "ABCD-EFGH-IJKL".into(),
                expires_at: Some(1_900_000_000_000),
            }
        );
    }

    #[test]
    fn sem_smart_url_monta_o_link() {
        let link = from_json(&json!({"joinCode": "ABCD", "smartUrl": ""})).unwrap();
        assert_eq!(link.url, "https://gg.riotgames.com/LOL?joinCode=ABCD");
        assert_eq!(link.expires_at, None);
    }

    #[test]
    fn sem_codigo_nao_ha_link() {
        assert_eq!(from_json(&json!({"joinCode": ""})), None);
        assert_eq!(from_json(&json!({})), None);
        assert_eq!(from_json(&Value::Null), None);
    }

    #[test]
    fn validade_em_segundos_ou_milissegundos() {
        assert_eq!(expires_at_ms(0), None);
        assert_eq!(expires_at_ms(1_900_000_000), Some(1_900_000_000_000));
        assert_eq!(expires_at_ms(1_900_000_000_000), Some(1_900_000_000_000));
    }

    #[test]
    fn so_reaproveita_codigo_ativo_e_longe_de_vencer() {
        let now = 1_800_000_000_000u64;
        let body = |active: bool, expires: u64| json!({"joinCode": "X", "isActive": active, "expiresAt": expires});
        assert!(usable(&body(true, now + 3_600_000), now).is_some());
        assert!(usable(&body(false, now + 3_600_000), now).is_none());
        assert!(usable(&body(true, now + 10_000), now).is_none());
        assert!(usable(&body(true, now - 1), now).is_none());
        // Sem validade, vale.
        assert!(usable(&json!({"joinCode": "X", "isActive": true}), now).is_some());
    }

    #[test]
    fn escapa_o_id_da_atividade() {
        assert_eq!(
            join_code_path("a1b2-c3"),
            "/lol-lobby/v2/ags/a1b2-c3/joinCode"
        );
        assert_eq!(
            join_code_path("a/b c"),
            "/lol-lobby/v2/ags/a%2Fb%20c/joinCode"
        );
    }
}
