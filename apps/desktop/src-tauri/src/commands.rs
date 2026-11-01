//! Commandes Tauri exposées à l'interface. Chaque commande est soumise aux
//! permissions déclarées dans `build.rs` et `capabilities/`.

use tauri::{AppHandle, State};

use crate::contracts::{AppError, CONTRACT_VERSION, OverlayState, ParsedItem, app_error};
use crate::{AppState, game_window, item_text, overlay};

#[tauri::command]
pub fn parse_clipboard_item(text: String) -> Result<ParsedItem, AppError> {
    item_text::parse(&text)
}

#[tauri::command]
pub fn get_overlay_state(app: AppHandle, state: State<'_, AppState>) -> OverlayState {
    let last_item = state.last_text().and_then(|t| item_text::parse(&t).ok());
    OverlayState {
        contract_version: CONTRACT_VERSION,
        app_version: app.package_info().version.to_string(),
        game_focused: game_window::game_is_foreground(),
        character: None,
        last_item,
        data_patch: None,
    }
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    overlay::hide(&app);
}

#[tauri::command]
pub fn fit_overlay(app: AppHandle, width: f64, height: f64) {
    overlay::fit(&app, width, height);
}

#[tauri::command]
pub fn open_main_window(app: AppHandle) {
    crate::show_main_window(&app);
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
) -> Result<Option<crate::updates::UpdateInfo>, AppError> {
    crate::updates::check(&app).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), AppError> {
    crate::updates::install(&app).await
}

#[tauri::command]
pub fn get_settings(store: State<'_, crate::settings::SettingsStore>) -> crate::settings::Settings {
    store.get()
}

#[tauri::command]
pub fn set_update_channel(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    beta: bool,
) -> Result<crate::settings::Settings, AppError> {
    store.update(&app, |s| s.beta_channel = beta).map_err(|e| {
        app_error(
            crate::contracts::ErrorCode::Internal,
            format!("Réglage non enregistré ({e})."),
        )
    })
}

#[tauri::command]
pub fn get_client_log_status(
    state: State<'_, crate::client_log::ClientLogState>,
) -> crate::client_log::ClientLogStatus {
    state.get()
}

/// Enregistre le chemin de `Client.txt` choisi par le joueur (fichier, dossier
/// `logs` ou dossier du jeu). `None` revient à la détection automatique.
#[tauri::command]
pub fn set_client_log_path(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    path: Option<String>,
) -> Result<crate::settings::Settings, AppError> {
    let path = path.filter(|p| !p.trim().is_empty());
    if let Some(p) = &path {
        let file = crate::log_locator::normalize_user_path(p);
        if !file.is_file() {
            return Err(app_error(
                crate::contracts::ErrorCode::Internal,
                format!(
                    "Aucun fichier Client.txt à cet endroit\u{a0}: {}",
                    file.display()
                ),
            ));
        }
    }
    store
        .update(&app, |s| s.client_log_path = path)
        .map_err(|e| {
            app_error(
                crate::contracts::ErrorCode::Internal,
                format!("Réglage non enregistré ({e})."),
            )
        })
}
