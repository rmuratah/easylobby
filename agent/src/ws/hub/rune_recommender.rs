//! Recomendador de runas da seleção (#596): as páginas que o client sugere
//! para o campeão na rota e a aplicação de uma delas.
//!
//! Nada é lido ao conectar: a UI pede as sugestões ao abrir o recomendador
//! (`openRecommendedRunes`). Elas ficam no estado até outra rota ou outro
//! campeão ser pedido e somem quando o LoL fecha.

use super::{not_connected, Hub, Shared};
use crate::lobby::Position;
use crate::rune_recommender::{self, RecommendedRunes};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

impl Hub {
    pub fn set_recommended_runes(&self, recommended: Option<RecommendedRunes>) {
        self.update(|shared| {
            if recommended.is_none() {
                shared.recommended_runes_request = None;
            }
            write_recommended_runes(shared, recommended)
        });
    }

    /// Lê as sugestões para `champion_id` na rota `position` (comando
    /// `openRecommendedRunes`). Chegam numa mensagem `recommendedRunes`
    /// antes da resposta; as de um pedido antigo são descartadas.
    pub async fn open_recommended_runes(
        &self,
        champion_id: u32,
        position: Position,
    ) -> CommandResult {
        if champion_id == 0 {
            return CommandResult::error(ErrorCode::InvalidCommand, "campeão inválido");
        }
        if !rune_recommender::is_lane(position) {
            return CommandResult::error(ErrorCode::InvalidCommand, "rota inválida");
        }
        let (client, session) = {
            let shared = self.lock();
            let Some(client) = shared.client.clone() else {
                return not_connected();
            };
            (client, shared.session)
        };
        let request = (champion_id, position);
        self.lock().recommended_runes_request = Some(request);
        match rune_recommender::recommended(&client, champion_id, position).await {
            Ok(Some(recommended)) => {
                self.update_in_session(session, |shared| {
                    if shared.recommended_runes_request != Some(request) {
                        return None;
                    }
                    write_recommended_runes(shared, Some(recommended))
                });
                CommandResult::Ok
            }
            Ok(None) => CommandResult::error(ErrorCode::Lcu, "resposta inesperada da LCU"),
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Aplica a sugestão `index` como página temporária (comando
    /// `applyRecommendedRunes`).
    pub async fn apply_recommended_runes(&self, index: u32) -> CommandResult {
        let state = self.state();
        let Some((recommended, page)) = state.recommended_runes.as_ref().and_then(|r| {
            let page = r.pages.get(usize::try_from(index).ok()?)?;
            Some((r, page))
        }) else {
            return CommandResult::error(ErrorCode::NotFound, "sugestão de runas não encontrada");
        };
        let catalog = state.rune_catalog.as_ref();
        let name = rune_recommender::page_name(
            recommended.champion_id,
            page,
            state.champion_catalog.as_ref(),
            catalog,
        );
        let input = rune_recommender::page_input(name, page);
        if let Err(invalid) = crate::runes::check_page(&input, catalog) {
            return CommandResult::error(ErrorCode::InvalidCommand, invalid.to_string());
        }
        let Some(client) = self.client() else {
            return not_connected();
        };
        let existing = rune_recommender::temporary_page(state.rune_pages.as_ref());
        let result = rune_recommender::apply(
            &client,
            existing,
            &input,
            recommended.champion_id,
            index as usize,
            page,
            catalog,
        )
        .await;
        self.after_rune_page_command(result).await
    }
}

fn write_recommended_runes(
    shared: &mut Shared,
    recommended: Option<RecommendedRunes>,
) -> Option<ServerMessage> {
    if shared.state.recommended_runes == recommended {
        return None;
    }
    shared.state.recommended_runes = recommended.clone();
    Some(ServerMessage::RecommendedRunes {
        recommended_runes: recommended,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rune_recommender::RecommendedRunePage;

    fn sample() -> RecommendedRunes {
        RecommendedRunes {
            champion_id: 63,
            position: Position::Middle,
            pages: vec![RecommendedRunePage {
                title: "Dano de Habilidade e Mana".into(),
                primary_style_id: 8200,
                sub_style_id: 8300,
                selected_perk_ids: vec![8229, 8226, 8210, 8237, 8304, 8347, 5008, 5008, 5011],
                summoner_spell_ids: vec![4, 14],
                recommendation_id: String::new(),
            }],
        }
    }

    #[tokio::test]
    async fn pedidos_invalidos_e_sem_lol() {
        let hub = Hub::new();
        let code = |result: CommandResult| match result {
            CommandResult::Error { code, .. } => Some(code),
            CommandResult::Ok => None,
        };
        assert_eq!(
            code(hub.open_recommended_runes(0, Position::Middle).await),
            Some(ErrorCode::InvalidCommand)
        );
        assert_eq!(
            code(hub.open_recommended_runes(63, Position::Fill).await),
            Some(ErrorCode::InvalidCommand)
        );
        assert_eq!(
            code(hub.open_recommended_runes(63, Position::Middle).await),
            Some(ErrorCode::NotConnected)
        );
        assert_eq!(
            code(hub.apply_recommended_runes(0).await),
            Some(ErrorCode::NotFound)
        );
        hub.set_recommended_runes(Some(sample()));
        assert_eq!(
            code(hub.apply_recommended_runes(1).await),
            Some(ErrorCode::NotFound)
        );
        assert_eq!(
            code(hub.apply_recommended_runes(0).await),
            Some(ErrorCode::NotConnected)
        );
    }

    #[test]
    fn sugestoes_novas_viram_mensagem_e_repetidas_nao() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        hub.set_recommended_runes(Some(sample()));
        hub.set_recommended_runes(Some(sample()));
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::RecommendedRunes {
                recommended_runes: Some(sample())
            }
        );
        assert!(rx.try_recv().is_err());
        hub.set_recommended_runes(None);
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::RecommendedRunes {
                recommended_runes: None
            }
        );
        assert_eq!(hub.state().recommended_runes, None);
    }
}
