//! Atualização automática pelo `tauri-plugin-updater` (#35).
//!
//! O workflow de release publica o instalador assinado e um `latest.json` no
//! GitHub Releases. O app consulta esse arquivo, confere a assinatura com a
//! chave pública do `tauri.conf.json` e, quando o jogador pede, baixa e roda o
//! instalador, que reabre o easylobby na versão nova.
//!
//! Quem decide a hora de instalar é a UI: ela só oferece o botão fora da fila,
//! da seleção de campeões e da partida, porque o app fecha durante a
//! instalação.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Versão nova encontrada pela última consulta, guardada para o
/// [`install_update`] não precisar consultar de novo.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

impl PendingUpdate {
    fn replace(&self, update: Option<Update>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = update;
    }

    fn take(&self) -> Option<Update> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

/// O que a UI mostra de uma versão nova.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// Versão instalada.
    current_version: String,
    /// Versão disponível no GitHub Releases.
    version: String,
    /// Notas da release, quando houver.
    notes: Option<String>,
}

/// Versão instalada, para a seção Atualizações das Configurações.
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Procura uma versão nova. `None` quando o app já está na mais recente.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| {
            tracing::warn!("erro ao procurar atualização: {error}");
            error.to_string()
        })?;
    let info = update.as_ref().map(|update| UpdateInfo {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        notes: update.body.clone().filter(|body| !body.trim().is_empty()),
    });
    app.state::<PendingUpdate>().replace(update);
    Ok(info)
}

/// Baixa e instala a versão encontrada pelo [`check_update`]. No Windows, o
/// instalador fecha o app e o abre de novo na versão nova; nos outros
/// sistemas o app reinicia aqui.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .state::<PendingUpdate>()
        .take()
        .ok_or("nenhuma atualização encontrada; procure de novo")?;
    tracing::info!(
        "instalando a versão {} (instalada: {})",
        update.version,
        update.current_version
    );
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| {
            tracing::error!("erro ao instalar a atualização: {error}");
            error.to_string()
        })?;
    app.restart();
}
