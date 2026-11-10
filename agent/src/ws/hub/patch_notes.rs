//! Notas da atualização atual para o card do Início (#301).

use super::Hub;
use crate::patch_notes::{self, PatchNotes};
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

/// Site de onde saem o título e o resumo das notas. Nos testes o agente não
/// sai para a internet: as notas ficam só com a versão.
const SITE: Option<&str> = if cfg!(test) {
    None
} else {
    Some(patch_notes::DEFAULT_SITE)
};

impl Hub {
    pub fn set_patch_notes(&self, notes: Option<PatchNotes>) {
        self.update(|shared| {
            if shared.state.patch_notes == notes {
                return None;
            }
            shared.state.patch_notes = notes.clone();
            Some(ServerMessage::PatchNotes { patch_notes: notes })
        });
    }

    /// Lê a versão do jogo e busca as notas numa tarefa separada: o site
    /// pode demorar, e nada mais depende dele.
    pub(super) fn spawn_refresh_patch_notes(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Some(client) = self.client() else {
            return;
        };
        let session = self.lock().session;
        let hub = self.clone();
        runtime.spawn(async move {
            // Sem a versão (LCU subindo), o que já estava na tela fica.
            let Some(notes) = patch_notes::current(&client, SITE).await else {
                return;
            };
            if hub.lock().session == session {
                hub.set_patch_notes(Some(notes));
            }
        });
    }

    /// Abre a página das notas no navegador (comando `openPatchNotes`).
    pub fn open_patch_notes(&self) -> CommandResult {
        let Some(url) = self
            .lock()
            .state
            .patch_notes
            .as_ref()
            .map(|n| n.url.clone())
        else {
            return CommandResult::error(ErrorCode::NotFound, "sem notas da atualização");
        };
        match patch_notes::open_in_browser(&url) {
            Ok(()) => CommandResult::Ok,
            Err(error) => CommandResult::error(ErrorCode::LaunchFailed, error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_notas_o_comando_responde_not_found() {
        let hub = Hub::new();
        assert!(matches!(
            hub.open_patch_notes(),
            CommandResult::Error {
                code: ErrorCode::NotFound,
                ..
            }
        ));
    }

    #[test]
    fn notas_novas_viram_mensagem_e_repetidas_nao() {
        let hub = Hub::new();
        let (_, mut rx) = hub.subscribe();
        let notes = PatchNotes {
            version: "26.22".into(),
            title: None,
            summary: None,
            url: "https://www.leagueoflegends.com/pt-br/news/tags/patch-notes/".into(),
            sections: Vec::new(),
        };
        hub.set_patch_notes(Some(notes.clone()));
        hub.set_patch_notes(Some(notes.clone()));
        assert_eq!(
            rx.try_recv().unwrap(),
            ServerMessage::PatchNotes {
                patch_notes: Some(notes)
            }
        );
        assert!(rx.try_recv().is_err());
    }
}
