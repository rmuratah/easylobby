//! Saldo de Essência Azul, Essência Laranja e RP no cabeçalho (#370).
//!
//! O agente lê as três moedas ao conectar, no login e no fim de cada
//! partida, e aplica os eventos da carteira e do item da Essência Laranja
//! que o WAMP mandar. Uma releitura que cruza com um evento pode trazer um
//! saldo um instante mais velho; o próximo evento ou releitura corrige.

use super::Hub;
use crate::wallet::{self, Wallet, WalletChange};
use crate::ws::protocol::ServerMessage;

impl Hub {
    pub fn set_wallet(&self, wallet: Option<Wallet>) {
        self.update(|shared| {
            if shared.state.wallet == wallet {
                return None;
            }
            shared.state.wallet = wallet.clone();
            Some(ServerMessage::Wallet { wallet })
        });
    }

    /// Aplica as mudanças de um evento da LCU, mantendo as outras moedas.
    pub(super) fn apply_wallet_changes(&self, changes: Vec<WalletChange>) {
        if changes.is_empty() {
            return;
        }
        self.update(|shared| {
            let mut next = shared.state.wallet.clone().unwrap_or_default();
            for change in changes {
                next.apply(change);
            }
            if shared.state.wallet.as_ref() == Some(&next) {
                return None;
            }
            shared.state.wallet = Some(next.clone());
            Some(ServerMessage::Wallet { wallet: Some(next) })
        });
    }

    /// Relê as três moedas numa tarefa separada. Se nenhuma responder (LoL
    /// subindo, antes do login), o que já estava na tela fica.
    pub(super) fn spawn_refresh_wallet(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            let Some(read) = wallet::current(&client).await else {
                return;
            };
            if hub.lock().session != session {
                return;
            }
            // Uma moeda que não respondeu desta vez fica com o valor de antes.
            let changes = [
                read.blue_essence.map(WalletChange::BlueEssence),
                read.orange_essence.map(WalletChange::OrangeEssence),
                read.rp.map(WalletChange::Rp),
            ];
            hub.apply_wallet_changes(changes.into_iter().flatten().collect());
        });
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::connection::ConnectionEvent;
    use crate::wamp::{EventType, LcuEvent};

    fn event(uri: &str, data: serde_json::Value) -> ConnectionEvent {
        ConnectionEvent::Event(LcuEvent {
            uri: uri.into(),
            event_type: EventType::Update,
            data,
        })
    }

    #[tokio::test]
    async fn eventos_da_carteira_mudam_so_a_sua_moeda() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.set_wallet(Some(Wallet {
            blue_essence: Some(100),
            orange_essence: Some(200),
            rp: Some(300),
        }));
        rx.try_recv().unwrap();

        hub.apply(event("/lol-inventory/v1/wallet/RP", json!({"RP": 50})))
            .await;
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::Wallet {
                wallet: Some(Wallet {
                    blue_essence: Some(100),
                    orange_essence: Some(200),
                    rp: Some(50),
                })
            }
        );

        hub.apply(event(
            wallet::ORANGE_ESSENCE_PATH,
            json!({"lootId": "CURRENCY_cosmetic", "count": 1250}),
        ))
        .await;
        assert_eq!(hub.state().wallet.unwrap().orange_essence, Some(1250));
        rx.try_recv().unwrap();

        // O mesmo saldo de novo e moedas que o app não mostra não viram
        // mensagem.
        hub.apply(event("/lol-inventory/v1/wallet/RP", json!({"RP": 50})))
            .await;
        hub.apply(event(
            "/lol-inventory/v1/wallet/tft_star_fragments",
            json!({"tft_star_fragments": 3}),
        ))
        .await;
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn evento_sem_saldo_anterior_cria_a_carteira() {
        let hub = Hub::new();
        hub.apply(event(
            "/lol-inventory/v1/wallet/lol_blue_essence",
            json!({"lol_blue_essence": 750}),
        ))
        .await;
        assert_eq!(
            hub.state().wallet,
            Some(Wallet {
                blue_essence: Some(750),
                orange_essence: None,
                rp: None,
            })
        );
    }
}
