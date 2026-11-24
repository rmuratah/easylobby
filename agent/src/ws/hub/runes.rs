//! Páginas de runas e o catálogo das árvores.

use super::{not_connected, Hub, Resource, Shared, Stamp, REFRESH_ATTEMPTS};
use crate::rest::LcuError;
use crate::runes::{self, RuneCatalog, RunePageError, RunePageInput, RunePages};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    pub fn set_rune_pages(&self, rune_pages: Option<RunePages>) {
        self.update(|shared| write_rune_pages(shared, rune_pages));
    }

    pub(super) fn set_rune_pages_if(&self, stamp: Stamp, rune_pages: Option<RunePages>) -> bool {
        self.update_if(Resource::RunePages, stamp, |shared| {
            write_rune_pages(shared, rune_pages)
        })
    }

    /// Muda uma parte das páginas de runas (lista, página em uso ou
    /// espaço livre) a partir de um evento. Antes de ler as páginas, começa
    /// de uma lista vazia.
    pub(super) fn edit_rune_pages(&self, f: impl FnOnce(&mut RunePages)) {
        self.update(|shared| {
            let mut rune_pages = shared.state.rune_pages.clone().unwrap_or_default();
            f(&mut rune_pages);
            write_rune_pages(shared, Some(rune_pages))
        });
    }

    pub fn set_rune_catalog(&self, rune_catalog: Option<RuneCatalog>) {
        self.update(|shared| {
            if shared.state.rune_catalog == rune_catalog {
                return None;
            }
            shared.state.rune_catalog = rune_catalog.clone();
            Some(ServerMessage::RuneCatalog { rune_catalog })
        });
    }

    /// Aplica a página de runas `id` (comando `selectRunePage`, #26).
    pub async fn select_rune_page(&self, id: u64) -> CommandResult {
        let Some(client) = self.client() else {
            return not_connected();
        };
        let result = runes::select_page(&client, id).await;
        self.after_rune_command(RuneAction::Select, result).await
    }

    /// Cria uma página de runas e a aplica (comando `createRunePage`, #26).
    pub async fn create_rune_page(&self, page: RunePageInput) -> CommandResult {
        let catalog = self.state().rune_catalog;
        // Página inválida não depende do LoL aberto.
        if let Err(invalid) = runes::check_page(&page, catalog.as_ref()) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        let result = runes::create_page(&client, &page, catalog.as_ref()).await;
        self.after_rune_page_command(RuneAction::Save, result).await
    }

    /// Edita a página de runas `id` (comando `updateRunePage`, #26).
    pub async fn update_rune_page(&self, id: u64, page: RunePageInput) -> CommandResult {
        let catalog = self.state().rune_catalog;
        if let Err(invalid) = runes::check_page(&page, catalog.as_ref()) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        let result = runes::update_page(&client, id, &page, catalog.as_ref()).await;
        self.after_rune_page_command(RuneAction::Save, result).await
    }

    /// Apaga a página de runas `id` (comando `deleteRunePage`, #26).
    pub async fn delete_rune_page(&self, id: u64) -> CommandResult {
        let Some(client) = self.client() else {
            return not_connected();
        };
        let result = runes::delete_page(&client, id).await;
        self.after_rune_command(RuneAction::Delete, result).await
    }

    pub(super) async fn after_rune_page_command(
        &self,
        action: RuneAction,
        result: Result<(), RunePageError>,
    ) -> CommandResult {
        match result {
            Ok(()) => self.after_rune_command(action, Ok(())).await,
            Err(RunePageError::Invalid(invalid)) => {
                CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string())
            }
            Err(RunePageError::Lcu(error)) => self.after_rune_command(action, Err(error)).await,
        }
    }

    /// Depois de um comando de runas que deu certo, relê as páginas pelo
    /// REST e publica antes de devolver, sem esperar o evento do WAMP. Se
    /// deu errado, devolve uma mensagem para o jogador e deixa o erro cru
    /// só no log (#655).
    pub(super) async fn after_rune_command(
        &self,
        action: RuneAction,
        result: Result<(), LcuError>,
    ) -> CommandResult {
        match result {
            Ok(()) => {
                self.refresh_rune_pages().await;
                CommandResult::Ok
            }
            Err(error) => {
                tracing::warn!("comando de runas falhou: {error}");
                CommandResult::error(ErrorCode::Lcu, friendly_error(action, &error))
            }
        }
    }

    /// Relê as páginas de runas depois de um comando; como
    /// [`Hub::refresh_lobby`] (#87). Se a releitura falhar, o evento do WAMP
    /// corrige depois.
    pub(super) async fn refresh_rune_pages(&self) {
        for _ in 0..REFRESH_ATTEMPTS {
            let Some((client, stamp)) = self.begin(Resource::RunePages) else {
                return;
            };
            let Ok(rune_pages) = runes::current_pages(&client).await else {
                return;
            };
            if self.set_rune_pages_if(stamp, Some(rune_pages)) {
                return;
            }
        }
    }
}

/// O que o comando de runas tentava fazer, para escolher a mensagem de
/// erro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuneAction {
    /// Aplicar uma página que já existe.
    Select,
    /// Criar ou salvar uma página (também a sugestão de runas).
    Save,
    /// Excluir uma página.
    Delete,
}

/// Mensagem para o jogador quando a LCU recusa ou não responde um comando
/// de runas (#655). Completa a frase "Não foi possível aplicar a página: "
/// da UI, sem rota nem código HTTP.
pub(super) fn friendly_error(action: RuneAction, error: &LcuError) -> String {
    if error.is_timeout() {
        return "o LoL demorou para responder. Tente de novo em alguns segundos.".into();
    }
    if error.is_transport() {
        return "a conexão com o LoL falhou. Confira se ele está aberto e tente de novo.".into();
    }
    match (action, error.status()) {
        (RuneAction::Select, Some(400 | 422)) => "a página está inválida neste patch \
            (runas incompletas ou que mudaram com a atualização). Edite a página, \
            escolha as runas que faltam e salve antes de aplicar."
            .into(),
        (RuneAction::Save, Some(400 | 422)) => "o LoL não aceitou estas runas. Confira se \
            todas estão escolhidas e tente de novo."
            .into(),
        (_, Some(403)) => "o LoL não deixa mudar esta página agora.".into(),
        (_, Some(404)) => "a página não existe mais. Escolha outra na lista.".into(),
        (_, Some(_)) => "o LoL recusou o pedido. Tente de novo.".into(),
        (_, None) => "algo deu errado ao falar com o LoL. Tente de novo.".into(),
    }
}

/// Troca as páginas de runas, contando a escrita mesmo quando nada muda
/// (#87).
pub(super) fn write_rune_pages(
    shared: &mut Shared,
    rune_pages: Option<RunePages>,
) -> Option<ServerMessage> {
    shared.rune_pages_rev += 1;
    if shared.state.rune_pages == rune_pages {
        return None;
    }
    shared.state.rune_pages = rune_pages.clone();
    Some(ServerMessage::RunePages { rune_pages })
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::champ_select::ChampSelect;
    use crate::champ_select::CHAMP_SELECT_SESSION_PATH;
    use crate::connection::ConnectionEvent;
    use crate::runes::CURRENT_PAGE_PATH;
    use crate::runes::INVENTORY_PATH;
    use crate::runes::PAGES_PATH;
    use crate::wamp::EventType;
    use serde_json::json;

    #[tokio::test]
    async fn segue_as_paginas_de_runas() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let page = |id: u64, current: bool| {
            json!({
                "id": id,
                "name": format!("Página {id}"),
                "primaryStyleId": 8000,
                "subStyleId": 8100,
                "selectedPerkIds": [8005, 9111, 9104, 8017, 8126, 8135, 5005, 5008, 5011],
                "current": current,
                "isEditable": true,
                "isDeletable": true,
                "isValid": true,
            })
        };
        hub.apply(event(
            PAGES_PATH,
            EventType::Update,
            json!([page(1, true), page(2, false)]),
        ))
        .await;
        match rx.try_recv().unwrap() {
            ServerMessage::RunePages {
                rune_pages: Some(runes),
            } => {
                assert_eq!(runes.pages.len(), 2);
                assert_eq!(runes.current_page_id, Some(1));
                assert!(runes.can_create_page);
            }
            other => panic!("esperava as páginas: {other:?}"),
        }
        hub.apply(event(CURRENT_PAGE_PATH, EventType::Update, page(2, true)))
            .await;
        hub.apply(event(
            INVENTORY_PATH,
            EventType::Update,
            json!({"canAddCustomPage": false, "ownedPageCount": 2}),
        ))
        .await;
        // Repetido não gera mensagem.
        hub.apply(event(CURRENT_PAGE_PATH, EventType::Update, page(2, true)))
            .await;
        hub.apply(event(PAGES_PATH, EventType::Delete, json!(null)))
            .await;
        let mut seen = Vec::new();
        while let Ok(ServerMessage::RunePages { rune_pages }) = rx.try_recv() {
            seen.push(rune_pages.unwrap());
        }
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].current_page_id, Some(2));
        assert!(!seen[1].can_create_page);
        assert_eq!(hub.state().rune_pages.unwrap().pages.len(), 2);

        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert_eq!(hub.state().rune_pages, None);
    }

    #[test]
    fn erro_da_lcu_vira_mensagem_amigavel() {
        let status = |status: u16, message: &str| LcuError::Status {
            method: reqwest::Method::PUT,
            path: CURRENT_PAGE_PATH.into(),
            status,
            message: Some(message.into()),
            body: json!({"message": message}),
        };
        let invalid = status(422, "Page is invalid and can't be set as active.");
        let text = friendly_error(RuneAction::Select, &invalid);
        assert!(
            text.starts_with("a página está inválida neste patch"),
            "{text}"
        );
        assert!(text.contains("Edite a página"), "{text}");
        for action in [RuneAction::Select, RuneAction::Save, RuneAction::Delete] {
            for code in [400, 403, 404, 422, 500] {
                let text = friendly_error(action, &status(code, "Page is invalid"));
                assert!(!text.contains("lol-perks"), "{text}");
                assert!(!text.contains(&code.to_string()), "{text}");
                assert!(!text.contains("invalid"), "{text}");
            }
        }
        assert_eq!(
            friendly_error(RuneAction::Save, &status(400, "bad")),
            "o LoL não aceitou estas runas. Confira se todas estão escolhidas e tente de novo."
        );
    }

    #[tokio::test]
    async fn comandos_de_runas_sem_lol_ou_pagina_invalida() {
        let hub = Hub::new();
        let page = RunePageInput {
            name: "Nova".into(),
            primary_style_id: 8000,
            sub_style_id: 8000,
            selected_perk_ids: vec![1; 9],
        };
        let code = |result: CommandResult| match result {
            CommandResult::Error { code, .. } => code,
            CommandResult::Ok => panic!("esperava erro"),
        };
        // Página inválida volta `invalidCommand` mesmo com o LoL fechado.
        assert_eq!(
            code(hub.create_rune_page(page.clone()).await),
            ErrorCode::InvalidCommand
        );
        assert_eq!(
            code(hub.update_rune_page(1, page.clone()).await),
            ErrorCode::InvalidCommand
        );
        let valid = RunePageInput {
            sub_style_id: 8100,
            ..page
        };
        assert_eq!(
            code(hub.create_rune_page(valid).await),
            ErrorCode::NotConnected
        );
        assert_eq!(code(hub.select_rune_page(1).await), ErrorCode::NotConnected);
        assert_eq!(code(hub.delete_rune_page(1).await), ErrorCode::NotConnected);
    }

    #[tokio::test]
    async fn selecao_e_runas_velhas_sao_descartadas() {
        let hub = Hub::new();
        let stamp = |resource| hub.lock().stamp(resource);
        let session = || ChampSelect::from_json(&champ_select("BAN_PICK", 86));

        // O `Delete` da seleção chegou durante a releitura: ela não volta.
        let s = stamp(Resource::ChampSelect);
        hub.apply(event(
            CHAMP_SELECT_SESSION_PATH,
            EventType::Delete,
            json!({}),
        ))
        .await;
        assert!(!hub.set_champ_select_if(s, session()));
        assert_eq!(hub.state().champ_select, None);
        let s = stamp(Resource::ChampSelect);
        assert!(hub.set_champ_select_if(s, session()));

        // Um evento parcial das runas, mesmo sem mudar nada, também conta.
        let pages = || Some(RunePages::default());
        let s = stamp(Resource::RunePages);
        hub.apply(event(
            INVENTORY_PATH,
            EventType::Update,
            json!({"canAddCustomPage": false}),
        ))
        .await;
        assert!(!hub.set_rune_pages_if(s, pages()));
        let s = stamp(Resource::RunePages);
        hub.apply(event(
            INVENTORY_PATH,
            EventType::Update,
            json!({"canAddCustomPage": false}),
        ))
        .await;
        assert!(!hub.set_rune_pages_if(s, pages()));

        // A conexão caiu: nada da sessão antiga entra.
        let (cs, runes) = (stamp(Resource::ChampSelect), stamp(Resource::RunePages));
        hub.apply(ConnectionEvent::Disconnected(
            crate::connection::DisconnectReason::Closed,
        ))
        .await;
        assert!(!hub.set_champ_select_if(cs, session()));
        assert!(!hub.set_rune_pages_if(runes, pages()));
        let state = hub.state();
        assert_eq!(state.champ_select, None);
        assert_eq!(state.rune_pages, None);
    }
}
