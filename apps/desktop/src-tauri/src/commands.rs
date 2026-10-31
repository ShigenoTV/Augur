//! Commandes Tauri exposées à l'interface. Chaque commande est soumise aux
//! permissions déclarées dans `build.rs` et `capabilities/`.

use tauri::{AppHandle, State};

use crate::contracts::{AppError, OverlayState, ParsedItem};
use crate::{AppState, game_window, item_text, overlay};

/// Version du contrat application ↔ interface (voir crates/augur-contracts).
pub const CONTRACT_VERSION: u32 = 1;

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
        AppError::new(
            crate::contracts::ErrorCode::Internal,
            format!("Réglage non enregistré ({e})."),
        )
    })
}
