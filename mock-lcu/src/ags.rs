//! Link para entrar no lobby (#226): a "atividade" (AGS) do lobby e o código
//! de convite dela.
//!
//! - `GET` [`ACTIVITY_ID`] devolve o id da atividade do lobby atual, uma
//!   string tirada do `partyId`; fora de lobby, 404.
//! - `POST` em `/lol-lobby/v2/ags/{id}/joinCode` gera o código, no formato
//!   `LolLobbyAgsSharedJoinCodeDto` (`joinCode`, `smartUrl`, `expiresAt`,
//!   `isActive`), válido por uma hora.
//! - `GET` no mesmo caminho responde 404, como se ainda não houvesse código:
//!   o mock não guarda o código gerado. `DELETE` responde 204.
//!
//! A gravação não tem essas rotas. O formato do link e os campos vêm da
//! investigação no LoL real (#226); o id da atividade, o código e a unidade
//! do `expiresAt` (milissegundos aqui) são suposições do mock.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::lobby::LOBBY;
use crate::state::{LcuState, Response};

/// Id da atividade do lobby atual.
pub const ACTIVITY_ID: &str = "/lol-lobby/v2/ags/agsActivityId";

/// Começo dos caminhos de código de convite.
const PREFIX: &str = "/lol-lobby/v2/ags/";

/// Fim dos caminhos de código de convite.
const SUFFIX: &str = "/joinCode";

/// Validade do código gerado.
const VALIDITY_MS: u64 = 60 * 60 * 1000;

/// Se `path` é o caminho do código de convite de alguma atividade.
pub fn is_join_code_path(path: &str) -> bool {
    path.strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
        .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}

/// Id da atividade do lobby atual, ou `None` fora de lobby.
fn current_activity(state: &LcuState) -> Option<String> {
    let lobby = state.get(LOBBY);
    if lobby.status != 200 {
        return None;
    }
    let party = lobby.body["partyId"].as_str().unwrap_or("party");
    Some(format!("mock-ags-{party}"))
}

/// `GET` [`ACTIVITY_ID`].
pub fn activity_id(state: &LcuState) -> Response {
    match current_activity(state) {
        Some(id) => Response {
            status: 200,
            body: Value::String(id),
        },
        None => Response::error(404, "mock-lcu: LOBBY_NOT_FOUND"),
    }
}

/// `GET`, `POST` ou `DELETE` no código de convite de `path`. A atividade
/// precisa ser a do lobby atual.
pub fn join_code(state: &LcuState, method: &str, path: &str) -> Response {
    let Some(activity) = current_activity(state) else {
        return Response::error(404, "mock-lcu: LOBBY_NOT_FOUND");
    };
    if path != format!("{PREFIX}{activity}{SUFFIX}") {
        return Response::error(404, "mock-lcu: atividade desconhecida");
    }
    match method {
        "POST" => {
            let code = code_for(&activity);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
            Response {
                status: 200,
                body: json!({
                    "joinCode": code,
                    "smartUrl": format!("https://gg.riotgames.com/LOL?joinCode={code}"),
                    "expiresAt": now + VALIDITY_MS,
                    "isActive": true,
                }),
            }
        }
        "DELETE" => Response {
            status: 204,
            body: Value::Null,
        },
        _ => Response::error(404, "mock-lcu: sem código de convite"),
    }
}

/// Código fixo por atividade, no formato `XXXX-XXXX-XXXX`.
fn code_for(activity: &str) -> String {
    // FNV-1a, só para o código não depender da ordem dos pedidos.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in activity.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let chars: Vec<char> = (0..12)
        .map(|i| char::from(ALPHABET[((hash >> (i * 5)) & 31) as usize]))
        .collect();
    format!(
        "{}-{}-{}",
        chars[0..4].iter().collect::<String>(),
        chars[4..8].iter().collect::<String>(),
        chars[8..12].iter().collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_o_caminho_do_codigo() {
        assert!(is_join_code_path("/lol-lobby/v2/ags/abc/joinCode"));
        assert!(!is_join_code_path("/lol-lobby/v2/ags//joinCode"));
        assert!(!is_join_code_path("/lol-lobby/v2/ags/a/b/joinCode"));
        assert!(!is_join_code_path(ACTIVITY_ID));
    }

    #[test]
    fn codigo_no_formato_do_client() {
        let code = code_for("mock-ags-1");
        assert_eq!(code.len(), 14);
        assert_eq!(code.matches('-').count(), 2);
        assert_eq!(code, code_for("mock-ags-1"));
    }
}
