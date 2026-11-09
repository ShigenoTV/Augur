//! Commandes Tauri exposées à l'interface. Chaque commande est soumise aux
//! permissions déclarées dans `build.rs` et `capabilities/`.

use tauri::{AppHandle, Manager, State};

use crate::contracts::{AppError, CONTRACT_VERSION, OverlayState, ParsedItem, app_error};
use crate::{AppState, game_window, item_text, overlay};

#[tauri::command]
pub fn parse_clipboard_item(text: String) -> Result<ParsedItem, AppError> {
    item_text::parse(&text)
}

#[tauri::command]
pub fn get_overlay_state(
    app: AppHandle,
    state: State<'_, AppState>,
    character: State<'_, crate::character::CharacterState>,
) -> OverlayState {
    let last_item = state.last_text().and_then(|t| item_text::parse(&t).ok());
    OverlayState {
        contract_version: CONTRACT_VERSION,
        app_version: app.package_info().version.to_string(),
        game_focused: game_window::game_is_foreground(),
        character: character.summary(),
        last_item,
        // Version du jeu des données embarquées (RePoE), vide si elles ne se chargent pas.
        data_patch: poe2_core::forge::atelier::donnees()
            .ok()
            .map(|d| d.repoe_version.clone())
            .filter(|v| !v.is_empty()),
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

/// Ligues proposées par poe.ninja, pour la liste de Réglages.
#[tauri::command]
pub async fn get_leagues(app: AppHandle) -> Result<Vec<String>, AppError> {
    crate::forge_prices::leagues(&app.package_info().version.to_string()).await
}

/// Ligue poe.ninja des prix de la Forge. `None` ou vide : ligue par défaut.
#[tauri::command]
pub fn set_league(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    league: Option<String>,
) -> Result<crate::settings::Settings, AppError> {
    let league = league
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty());
    store.update(&app, |s| s.league = league).map_err(|e| {
        app_error(
            crate::contracts::ErrorCode::Internal,
            format!("Réglage non enregistré ({e})."),
        )
    })
}

/// Raccourcis de l'overlay. Si l'overlay est affiché, les anciennes touches
/// sont libérées et les nouvelles prises tout de suite.
#[tauri::command]
pub fn set_shortcuts(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    shortcuts: crate::settings::ShortcutSettings,
) -> Result<crate::settings::Settings, AppError> {
    let shortcuts = crate::settings::ShortcutSettings {
        close_overlay: shortcuts.close_overlay.trim().to_string(),
        upgrade_search: shortcuts.upgrade_search.trim().to_string(),
    };
    crate::shortcuts::validate(&shortcuts)
        .map_err(|m| app_error(crate::contracts::ErrorCode::Internal, m))?;
    let visible = app.state::<overlay::OverlayCtl>().is_visible();
    if visible {
        crate::shortcuts::unregister_overlay(&app);
    }
    let result = store.update(&app, |s| s.shortcuts = shortcuts);
    if visible {
        crate::shortcuts::register_overlay(&app);
    }
    result.map_err(|e| {
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

#[tauri::command]
pub fn get_measure_status(
    state: State<'_, crate::measures::MeasureState>,
) -> crate::measures::MeasureStatus {
    state.status()
}

#[tauri::command]
pub fn start_measure_session(app: AppHandle) -> crate::measures::MeasureStatus {
    crate::measures::start(&app)
}

/// Arrête la session et écrit `augur-mesures-<date>.json` et `.md`.
#[tauri::command]
pub fn stop_measure_session(app: AppHandle) -> Result<crate::measures::MeasureStatus, AppError> {
    crate::measures::stop(&app).map_err(|e| {
        app_error(
            crate::contracts::ErrorCode::Internal,
            format!("Export des mesures impossible ({e})."),
        )
    })
}

#[tauri::command]
pub fn add_measure_note(app: AppHandle, text: String) -> crate::measures::MeasureStatus {
    crate::measures::add_note(&app, &text);
    app.state::<crate::measures::MeasureState>().status()
}

/// Copie le JSON de la dernière session terminée dans le presse-papier, sur
/// demande explicite du joueur (bouton de la fenêtre principale).
#[tauri::command]
pub fn copy_measure_export(
    state: State<'_, crate::measures::MeasureState>,
) -> Result<(), AppError> {
    let Some(json) = state.finished_json() else {
        return Err(app_error(
            crate::contracts::ErrorCode::Internal,
            "Aucune session terminée à copier.",
        ));
    };
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(json))
        .map_err(|e| {
            app_error(
                crate::contracts::ErrorCode::Internal,
                format!("Presse-papier indisponible ({e})."),
            )
        })
}

fn internal(message: String) -> AppError {
    app_error(crate::contracts::ErrorCode::Internal, message)
}

#[tauri::command]
pub fn get_ledger_status(
    state: State<'_, crate::ledger::LedgerState>,
) -> crate::ledger::LedgerStatus {
    state.status()
}

/// Démarre une session Atlas Ledger (sans effet si une session est en cours).
#[tauri::command]
pub fn start_ledger_session(app: AppHandle) -> crate::ledger::LedgerStatus {
    crate::ledger::start(&app)
}

/// Arrête la session et l'enregistre dans l'historique local.
#[tauri::command]
pub fn stop_ledger_session(app: AppHandle) -> Result<crate::ledger::LedgerStatus, AppError> {
    crate::ledger::stop(&app).map_err(internal)
}

/// Étiquette la session en cours (`id` nul) ou une session de l'historique.
#[tauri::command]
pub fn set_ledger_strategy(
    app: AppHandle,
    id: Option<i64>,
    strategy: crate::ledger_store::Strategy,
) -> Result<crate::ledger::LedgerStatus, AppError> {
    crate::ledger::set_strategy(&app, id, strategy).map_err(internal)
}

#[tauri::command]
pub fn list_ledger_sessions(
    app: AppHandle,
    limit: u32,
) -> Result<Vec<crate::ledger_store::SessionRow>, AppError> {
    crate::ledger::history(&app, limit).map_err(internal)
}

#[tauri::command]
pub fn get_ledger_session(
    app: AppHandle,
    id: i64,
) -> Result<crate::ledger_store::StoredSession, AppError> {
    crate::ledger::session(&app, id).map_err(internal)
}

#[tauri::command]
pub fn delete_ledger_session(app: AppHandle, id: i64) -> Result<(), AppError> {
    crate::ledger::delete(&app, id).map_err(internal)
}

#[tauri::command]
pub fn get_character(
    state: State<'_, crate::character::CharacterState>,
) -> Option<crate::character::CharacterInfo> {
    state.info()
}

/// Importe le personnage d'un code d'export Path of Building PoE 2 collé par
/// le joueur (rien n'est téléchargé : un lien pobb.in est refusé avec un message).
#[tauri::command]
pub fn import_pob_code(
    app: AppHandle,
    code: String,
) -> Result<crate::character::CharacterInfo, AppError> {
    crate::character::import(&app, &code).map_err(internal)
}

#[tauri::command]
pub fn clear_character(app: AppHandle) -> Result<(), AppError> {
    crate::character::clear(&app).map_err(internal)
}

/// Gain de DPS de l'objet pour le personnage importé (calcul local).
#[tauri::command]
pub fn evaluate_item(
    character: State<'_, crate::character::CharacterState>,
    item: ParsedItem,
) -> Result<crate::contracts::ItemDelta, AppError> {
    character
        .with_build(|b| crate::evaluation::evaluate(Some(b), &item))
        .unwrap_or_else(|| crate::evaluation::evaluate(None, &item))
}

/// Prix estimé : pas encore disponible (recherche de trade au sprint 3).
#[tauri::command]
pub fn estimate_price(_item: ParsedItem) -> Result<crate::contracts::PriceEstimate, AppError> {
    Err(app_error(
        crate::contracts::ErrorCode::FeatureDisabled,
        "L'estimation de prix arrive dans une prochaine version.",
    ))
}

#[tauri::command]
pub fn get_forge_prices(
    prices: State<'_, crate::forge_prices::ForgePrices>,
) -> poe2_core::forge::prix::TablePrix {
    prices.get()
}

#[tauri::command]
pub fn set_forge_prices(
    app: AppHandle,
    prices: State<'_, crate::forge_prices::ForgePrices>,
    prix: std::collections::BTreeMap<String, f64>,
) -> Result<poe2_core::forge::prix::TablePrix, AppError> {
    prices.set(&app, &prix)
}

#[tauri::command]
pub fn reset_forge_prices(
    app: AppHandle,
    prices: State<'_, crate::forge_prices::ForgePrices>,
) -> Result<poe2_core::forge::prix::TablePrix, AppError> {
    prices.reset(&app)
}

#[tauri::command]
pub async fn refresh_forge_prices(
    app: AppHandle,
    prices: State<'_, crate::forge_prices::ForgePrices>,
    store: State<'_, crate::settings::SettingsStore>,
    ligue: Option<String>,
) -> Result<poe2_core::forge::prix::TablePrix, AppError> {
    // Sans ligue précisée : celle des Réglages (Forbidden Rites par défaut).
    let ligue = ligue
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| store.get().league().to_string());
    prices.refresh(&app, &ligue).await
}

#[tauri::command]
pub fn get_forge_bases() -> Result<Vec<poe2_core::forge::atelier::BaseForge>, AppError> {
    poe2_core::forge::atelier::bases().map_err(crate::forge::error)
}

#[tauri::command]
pub fn get_forge_affixes(
    base_id: String,
    ilvl: u32,
) -> Result<poe2_core::forge::atelier::AffixesBase, AppError> {
    poe2_core::forge::atelier::affixes(&base_id, ilvl).map_err(crate::forge::error)
}

#[tauri::command]
pub fn get_forge_catalogue() -> Vec<poe2_core::forge::atelier::EntreeCatalogue> {
    poe2_core::forge::atelier::catalogue()
}

#[tauri::command]
pub async fn plan_forge_craft(
    app: AppHandle,
    planner: State<'_, crate::forge::ForgePlanner>,
    prices: State<'_, crate::forge_prices::ForgePrices>,
    demande: poe2_core::forge::atelier::DemandePlan,
) -> Result<poe2_core::forge::atelier::ResultatPlan, AppError> {
    planner.plan(&app, demande, prices.get()).await
}

#[tauri::command]
pub fn get_forge_history(
    planner: State<'_, crate::forge::ForgePlanner>,
) -> poe2_core::forge::historique::Historique {
    planner.historique()
}

#[tauri::command]
pub fn delete_forge_history_entry(
    app: AppHandle,
    planner: State<'_, crate::forge::ForgePlanner>,
    id: u64,
) -> poe2_core::forge::historique::Historique {
    planner.supprimer(&app, id)
}

#[tauri::command]
pub fn get_forge_advice(
    state: State<'_, crate::AppState>,
    planner: State<'_, crate::forge::ForgePlanner>,
) -> Result<Option<poe2_core::forge::suivi::ConseilForge>, AppError> {
    match state.last_text() {
        Some(text) => planner.conseil(&text),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn get_forge_tracking(
    planner: State<'_, crate::forge::ForgePlanner>,
) -> Option<crate::forge::Suivi> {
    planner.suivi()
}

#[tauri::command]
pub fn stop_forge_tracking(app: AppHandle, planner: State<'_, crate::forge::ForgePlanner>) {
    planner.arreter(&app);
}
