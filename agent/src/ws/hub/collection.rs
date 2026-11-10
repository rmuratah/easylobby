//! Coleção de campeões e skins da conta (#369), lida quando a UI pede.
//!
//! Nada aqui é lido ao conectar: a aba Coleção pede a coleção ao abrir
//! (`openCollection`) e as skins de um campeão ao abrir o campeão
//! (`openChampionSkins`). O resultado fica no estado até ser pedido de novo
//! e some quando o LoL fecha.

use super::{not_connected, Hub, Shared};
use crate::collection::{self, ChampionSkins, Collection};
use crate::rest::LcuClient;
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    pub fn set_collection(&self, collection: Option<Collection>) {
        self.update(|shared| write_collection(shared, collection));
    }

    pub fn set_champion_skins(&self, skins: Option<ChampionSkins>) {
        self.update(|shared| {
            if skins.is_none() {
                shared.champion_skins_request = None;
            }
            write_champion_skins(shared, skins)
        });
    }

    /// Cliente, conexão e `summonerId` do jogador logado. `None` com o LoL
    /// fechado ou antes do invocador carregar.
    fn inventory_client(&self) -> Option<(LcuClient, u64, u64)> {
        let shared = self.lock();
        let client = shared.client.clone()?;
        let summoner_id = shared
            .state
            .summoner
            .as_ref()
            .map(|s| s.summoner_id)
            .filter(|&id| id != 0)?;
        Some((client, shared.session, summoner_id))
    }

    /// Lê a coleção da conta (comando `openCollection`). A coleção chega
    /// numa mensagem `collection` antes da resposta.
    pub async fn open_collection(&self) -> CommandResult {
        let Some((client, session, summoner_id)) = self.inventory_client() else {
            return not_connected();
        };
        let catalog = self.lock().state.champion_catalog.clone();
        match collection::load(&client, summoner_id, catalog.as_ref()).await {
            Ok(Some(collection)) => {
                self.update_in_session(session, |shared| {
                    write_collection(shared, Some(collection))
                });
                CommandResult::Ok
            }
            Ok(None) => CommandResult::error(ErrorCode::Lcu, "resposta inesperada da LCU"),
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Lê as skins do campeão `champion_id` (comando `openChampionSkins`).
    /// As skins chegam numa mensagem `championSkins` antes da resposta. Se
    /// outro campeão for pedido antes destas chegarem, estas são
    /// descartadas.
    pub async fn open_champion_skins(&self, champion_id: u32) -> CommandResult {
        if champion_id == 0 {
            return CommandResult::error(ErrorCode::InvalidCommand, "campeão inválido");
        }
        let Some((client, session, summoner_id)) = self.inventory_client() else {
            return not_connected();
        };
        self.lock().champion_skins_request = Some(champion_id);
        match collection::champion_skins(&client, summoner_id, champion_id).await {
            Ok(Some(skins)) => {
                self.update_in_session(session, |shared| {
                    if shared.champion_skins_request != Some(champion_id) {
                        return None;
                    }
                    write_champion_skins(shared, Some(skins))
                });
                CommandResult::Ok
            }
            Ok(None) => CommandResult::error(ErrorCode::Lcu, "resposta inesperada da LCU"),
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }
}

fn write_collection(shared: &mut Shared, collection: Option<Collection>) -> Option<ServerMessage> {
    if shared.state.collection == collection {
        return None;
    }
    shared.state.collection = collection.clone();
    Some(ServerMessage::Collection { collection })
}

fn write_champion_skins(
    shared: &mut Shared,
    skins: Option<ChampionSkins>,
) -> Option<ServerMessage> {
    if shared.state.champion_skins == skins {
        return None;
    }
    shared.state.champion_skins = skins.clone();
    Some(ServerMessage::ChampionSkins {
        champion_skins: skins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sem_lol_os_comandos_respondem_not_connected() {
        let hub = Hub::new();
        assert!(matches!(
            hub.open_collection().await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
        assert!(matches!(
            hub.open_champion_skins(266).await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn campeao_zero_e_invalido() {
        let hub = Hub::new();
        assert!(matches!(
            hub.open_champion_skins(0).await,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        ));
    }

    #[test]
    fn colecao_e_skins_novas_viram_mensagem_e_repetidas_nao() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let collection = Collection::default();
        hub.set_collection(Some(collection.clone()));
        hub.set_collection(Some(collection.clone()));
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::Collection {
                collection: Some(collection)
            }
        );
        assert!(rx.try_recv().is_err());

        let skins = ChampionSkins {
            champion_id: 266,
            skins: Vec::new(),
        };
        hub.set_champion_skins(Some(skins.clone()));
        hub.set_champion_skins(Some(skins.clone()));
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::ChampionSkins {
                champion_skins: Some(skins)
            }
        );
        assert!(rx.try_recv().is_err());

        hub.set_collection(None);
        hub.set_champion_skins(None);
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::Collection { collection: None }
        );
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::ChampionSkins {
                champion_skins: None
            }
        );
        assert_eq!(hub.state().collection, None);
    }
}
