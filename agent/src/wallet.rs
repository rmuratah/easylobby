//! Saldo da conta: Essência Azul, Essência Laranja e RP (#370). Só leitura.
//!
//! A Essência Azul e o RP moram na carteira do inventário:
//! `GET /lol-inventory/v1/wallet/{moeda}` responde `{"<moeda>": saldo}`, e o
//! WAMP avisa no mesmo URI quando o saldo muda. A Essência Laranja não está
//! na carteira: é o item `CURRENCY_cosmetic` do Hextec
//! (`/lol-loot/v1/player-loot/CURRENCY_cosmetic`), com o saldo em `count`.
//! Os nomes foram conferidos nos logs do client oficial (16.22).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Prefixo dos URIs da carteira, para o filtro do WAMP.
pub const WALLET_PREFIX: &str = "/lol-inventory/v1/wallet";
/// Moeda do RP na carteira.
pub const RP: &str = "RP";
/// Moeda da Essência Azul na carteira.
pub const BLUE_ESSENCE: &str = "lol_blue_essence";
/// Item do Hextec com a Essência Laranja (e URI dos eventos dele).
pub const ORANGE_ESSENCE_PATH: &str = "/lol-loot/v1/player-loot/CURRENCY_cosmetic";

/// Saldo da conta. Cada moeda é `null` enquanto a LCU não respondeu.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Wallet {
    pub blue_essence: Option<u32>,
    pub orange_essence: Option<u32>,
    pub rp: Option<u32>,
}

/// Uma mudança no saldo que chegou pelo WAMP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletChange {
    BlueEssence(u32),
    OrangeEssence(u32),
    Rp(u32),
}

impl Wallet {
    pub fn apply(&mut self, change: WalletChange) {
        match change {
            WalletChange::BlueEssence(v) => self.blue_essence = Some(v),
            WalletChange::OrangeEssence(v) => self.orange_essence = Some(v),
            WalletChange::Rp(v) => self.rp = Some(v),
        }
    }
}

/// URI da carteira de uma moeda.
pub fn wallet_path(currency: &str) -> String {
    format!("{WALLET_PREFIX}/{currency}")
}

fn as_u32(value: &Value) -> Option<u32> {
    let n = value.as_u64().or_else(|| {
        // Algumas versões mandam o saldo como número com casa decimal.
        value.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64)
    })?;
    Some(u32::try_from(n).unwrap_or(u32::MAX))
}

/// Saldo de `currency` numa resposta da carteira: `{"RP": 326}`, o número
/// solto ou `{"RP": {"amount": 326}}`.
pub fn currency_from_json(currency: &str, value: &Value) -> Option<u32> {
    if let Some(n) = as_u32(value) {
        return Some(n);
    }
    let inner = value.as_object()?.get(currency)?;
    as_u32(inner).or_else(|| inner.get("amount").and_then(as_u32))
}

/// Essência Laranja no item do Hextec (campo `count`).
pub fn orange_essence_from_json(value: &Value) -> Option<u32> {
    value.get("count").and_then(as_u32)
}

/// Lê um evento da LCU que mexe no saldo. `None` se o URI ou o `data` não
/// forem do saldo. Aceita o URI de uma moeda (`/wallet/RP`) e o da carteira
/// inteira (`/wallet`, com várias moedas), devolvendo cada mudança achada.
pub fn changes_from_event(uri: &str, data: &Value) -> Vec<WalletChange> {
    if uri == ORANGE_ESSENCE_PATH {
        return orange_essence_from_json(data)
            .map(WalletChange::OrangeEssence)
            .into_iter()
            .collect();
    }
    let Some(rest) = uri.strip_prefix(WALLET_PREFIX) else {
        return Vec::new();
    };
    let currencies: &[&str] = match rest.trim_start_matches('/') {
        "" => &[BLUE_ESSENCE, RP],
        BLUE_ESSENCE => &[BLUE_ESSENCE],
        RP => &[RP],
        _ => &[],
    };
    currencies
        .iter()
        .filter_map(|c| {
            // Na carteira inteira o número solto não diz de qual moeda é.
            let value = if rest.is_empty() {
                data.get(*c).and_then(|v| currency_from_json(c, v))
            } else {
                currency_from_json(c, data)
            }?;
            Some(if *c == RP {
                WalletChange::Rp(value)
            } else {
                WalletChange::BlueEssence(value)
            })
        })
        .collect()
}

async fn currency(lcu: &LcuClient, currency: &str) -> Option<u32> {
    let body = lcu.get::<Value>(&wallet_path(currency)).await.ok()?;
    currency_from_json(currency, &body)
}

async fn orange_essence(lcu: &LcuClient) -> Option<u32> {
    match lcu.get::<Value>(ORANGE_ESSENCE_PATH).await {
        Ok(body) => orange_essence_from_json(&body),
        // Sem o item, a conta não tem Essência Laranja.
        Err(LcuError::Status { status: 404, .. }) => Some(0),
        Err(_) => None,
    }
}

/// Lê as três moedas. `None` se nenhuma respondeu (LoL subindo ou antes do
/// login).
pub async fn current(lcu: &LcuClient) -> Option<Wallet> {
    let (blue_essence, orange_essence, rp) = tokio::join!(
        currency(lcu, BLUE_ESSENCE),
        orange_essence(lcu),
        currency(lcu, RP),
    );
    let wallet = Wallet {
        blue_essence,
        orange_essence,
        rp,
    };
    (wallet != Wallet::default()).then_some(wallet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_a_resposta_da_carteira() {
        assert_eq!(currency_from_json(RP, &json!({"RP": 326})), Some(326));
        assert_eq!(
            currency_from_json(BLUE_ESSENCE, &json!({"lol_blue_essence": 297248})),
            Some(297248)
        );
        assert_eq!(currency_from_json(RP, &json!(15)), Some(15));
        assert_eq!(
            currency_from_json(RP, &json!({"RP": {"amount": 40}})),
            Some(40)
        );
        assert_eq!(currency_from_json(RP, &json!({"outra": 1})), None);
        assert_eq!(currency_from_json(RP, &json!(-3)), None);
    }

    #[test]
    fn le_a_essencia_laranja_do_hextec() {
        let item = json!({"lootId": "CURRENCY_cosmetic", "count": 1250, "type": "CURRENCY"});
        assert_eq!(orange_essence_from_json(&item), Some(1250));
        assert_eq!(orange_essence_from_json(&json!({})), None);
    }

    #[test]
    fn eventos_da_carteira_viram_mudancas() {
        assert_eq!(
            changes_from_event("/lol-inventory/v1/wallet/RP", &json!({"RP": 5})),
            vec![WalletChange::Rp(5)]
        );
        assert_eq!(
            changes_from_event(
                "/lol-inventory/v1/wallet/lol_blue_essence",
                &json!({"lol_blue_essence": 750})
            ),
            vec![WalletChange::BlueEssence(750)]
        );
        assert_eq!(
            changes_from_event(
                "/lol-inventory/v1/wallet",
                &json!({"RP": 10, "lol_blue_essence": 20, "tft_star_fragments": 3})
            ),
            vec![WalletChange::BlueEssence(20), WalletChange::Rp(10)]
        );
        assert_eq!(
            changes_from_event(ORANGE_ESSENCE_PATH, &json!({"count": 99})),
            vec![WalletChange::OrangeEssence(99)]
        );
        // Moedas que o app não mostra.
        assert!(changes_from_event(
            "/lol-inventory/v1/wallet/tft_star_fragments",
            &json!({"tft_star_fragments": 300})
        )
        .is_empty());
        assert!(changes_from_event("/lol-inventory/v1/walletx", &json!(1)).is_empty());
    }
}
