//! Commandes Tauri exposées à l'interface. Chaque commande est soumise aux
//! permissions déclarées dans `build.rs` et `capabilities/`.

use tauri::{AppHandle, Emitter, Manager, State};

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
        data_patch_poe1: poe2_core::poe1::forge::donnees()
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

/// Version compacte et taille de l'overlay (Réglages).
#[tauri::command]
pub fn set_overlay_display(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    compact: bool,
    scale: u16,
) -> Result<crate::settings::Settings, AppError> {
    let scale = scale.clamp(
        crate::settings::OVERLAY_SCALE_MIN,
        crate::settings::OVERLAY_SCALE_MAX,
    );
    let settings = store
        .update(&app, |s| {
            s.overlay.compact = compact;
            s.overlay.scale = scale;
        })
        .map_err(|e| {
            app_error(
                crate::contracts::ErrorCode::Internal,
                format!("Réglage non enregistré ({e})."),
            )
        })?;
    overlay::emit_settings(&app, &settings);
    Ok(settings)
}

/// Épingle ou libère l'overlay (Réglages ou bouton de l'overlay).
#[tauri::command]
pub fn set_overlay_pinned(
    app: AppHandle,
    pinned: bool,
) -> Result<crate::settings::Settings, AppError> {
    overlay::set_pinned(&app, pinned).map_err(|e| {
        app_error(
            crate::contracts::ErrorCode::Internal,
            format!("Réglage non enregistré ({e})."),
        )
    })
}

/// Active ou coupe le bandeau Atlas Ledger en jeu (Réglages).
#[tauri::command]
pub fn set_ledger_banner(
    app: AppHandle,
    enabled: bool,
) -> Result<crate::settings::Settings, AppError> {
    crate::ledger_banner::set_enabled(&app, enabled).map_err(|e| {
        app_error(
            crate::contracts::ErrorCode::Internal,
            format!("Réglage non enregistré ({e})."),
        )
    })
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
    crate::forge_prices::leagues(
        &app.package_info().version.to_string(),
        crate::game::active(),
    )
    .await
}

/// Change le jeu actif (sélecteur « POE 1 / POE 2 »). Refusé pendant une
/// session Atlas Ledger. Le suivi de `Client.txt` passe de lui-même au
/// journal du nouveau jeu.
#[tauri::command]
pub fn set_game(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    ledger: State<'_, crate::ledger::LedgerState>,
    game: crate::game::Game,
) -> Result<crate::settings::Settings, AppError> {
    if store.get().game == game && crate::game::active() == game {
        return Ok(store.get());
    }
    ledger
        .switch_game(&app, game)
        .map_err(|m| app_error(crate::contracts::ErrorCode::Internal, m))?;
    let settings = store.update(&app, |s| s.game = game).map_err(|e| {
        app_error(
            crate::contracts::ErrorCode::Internal,
            format!("Réglage non enregistré ({e})."),
        )
    })?;
    crate::game::set_active(game);
    log::info!("jeu actif : {}", game.label());
    if game == crate::game::Game::Poe1 {
        crate::poe1_calc::prechauffer(&app);
    }
    let _ = app.emit(crate::game::EVENT_CHANGED, game);
    Ok(settings)
}

/// PoE 1 : objet copié en dernier et sa cote poe.ninja (overlay).
#[tauri::command]
pub async fn get_poe1_item_info(
    app: AppHandle,
) -> Result<crate::poe1_items::Poe1ItemInfo, AppError> {
    crate::poe1_items::info(&app).await
}

/// PoE 1 : ouvre la recherche de l'objet copié sur le site de trade (Alt+U).
/// PoE 1 : l'objet copié comparé à celui porté (Path of Building embarqué).
#[tauri::command]
pub async fn evaluate_poe1_item(
    app: AppHandle,
) -> Result<crate::poe1_calc::Poe1Evaluation, AppError> {
    tauri::async_runtime::spawn_blocking(move || crate::poe1_calc::evaluer(&app))
        .await
        .map_err(|e| {
            crate::contracts::app_error(crate::contracts::ErrorCode::Internal, e.to_string())
        })?
}

#[tauri::command]
pub fn open_poe1_trade_search(app: AppHandle) -> Result<String, AppError> {
    crate::poe1_items::open_trade(&app)
}

/// PoE 1, Alt+U : recherche de trade pondérée par Path of Building pour
/// l'emplacement de l'objet copié (sinon, la recherche de l'objet lui-même).
#[tauri::command]
pub async fn open_poe1_upgrade_search(
    app: AppHandle,
) -> Result<crate::upgrade_search::UpgradeSearch, AppError> {
    tauri::async_runtime::spawn_blocking(move || crate::poe1_calc::rechercher_upgrade(&app))
        .await
        .map_err(|e| {
            crate::contracts::app_error(crate::contracts::ErrorCode::Internal, e.to_string())
        })?
}

/// PoE 1 : état de la carte « Coffres » (compte, onglets suivis, deux derniers relevés).
#[tauri::command]
pub fn get_poe1_stash(app: AppHandle) -> crate::poe1_stash::StashStatus {
    crate::poe1_stash::status(&app)
}

/// PoE 1 : enregistre le compte et la session (`POESESSID`, facultative si déjà
/// enregistrée), puis vérifie l'accès en listant les onglets.
#[tauri::command]
pub async fn connect_poe1_stash(
    app: AppHandle,
    account: String,
    session: Option<String>,
) -> Result<Vec<poe2_core::poe1::coffre::Onglet>, AppError> {
    crate::poe1_stash::connecter(&app, account, session).await
}

/// PoE 1 : efface la session enregistrée.
#[tauri::command]
pub fn forget_poe1_stash_session(app: AppHandle) -> crate::poe1_stash::StashStatus {
    crate::poe1_stash::oublier(&app);
    crate::poe1_stash::status(&app)
}

/// PoE 1 : onglets du compte (une lecture sur le site).
#[tauri::command]
pub async fn list_poe1_stash_tabs(
    app: AppHandle,
) -> Result<Vec<poe2_core::poe1::coffre::Onglet>, AppError> {
    crate::poe1_stash::onglets(&app).await
}

/// PoE 1 : onglets suivis.
#[tauri::command]
pub fn set_poe1_stash_tabs(
    app: AppHandle,
    tabs: Vec<u32>,
) -> Result<crate::poe1_stash::StashStatus, AppError> {
    crate::poe1_stash::choisir(&app, tabs)
}

/// PoE 1 : relève les onglets suivis et leur valeur.
#[tauri::command]
pub async fn take_poe1_stash_snapshot(
    app: AppHandle,
) -> Result<crate::poe1_stash::StashStatus, AppError> {
    crate::poe1_stash::relever(&app).await
}

/// PoE 1 : intervalle du relevé automatique pendant une session Ledger (0 : coupé).
#[tauri::command]
pub fn set_poe1_stash_auto(
    app: AppHandle,
    minutes: u32,
) -> Result<crate::poe1_stash::StashStatus, AppError> {
    crate::poe1_stash::set_auto(&app, minutes)
}

/// Ligue poe.ninja du jeu actif. `None` ou vide : ligue par défaut.
#[tauri::command]
pub fn set_league(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    league: Option<String>,
) -> Result<crate::settings::Settings, AppError> {
    let league = league
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty());
    let game = crate::game::active();
    store
        .update(&app, |s| s.set_league_for(game, league))
        .map_err(|e| {
            app_error(
                crate::contracts::ErrorCode::Internal,
                format!("Réglage non enregistré ({e})."),
            )
        })
}

/// Budget de la recherche d'upgrades du jeu actif. `None` ou 0 : sans limite.
/// PoE 2 : en exalts. PoE 1 : en chaos ou en divines (`currency`).
#[tauri::command]
pub fn set_upgrade_budget(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    budget: Option<u32>,
    currency: Option<String>,
) -> Result<crate::settings::Settings, AppError> {
    let budget = budget.filter(|b| *b > 0);
    let devise = currency.filter(|c| c == "chaos" || c == "divine");
    store
        .update(&app, |s| match crate::game::active() {
            crate::game::Game::Poe2 => s.upgrade_budget = budget,
            crate::game::Game::Poe1 => {
                s.poe1.upgrade_budget = budget;
                s.poe1.upgrade_budget_currency = devise;
            }
        })
        .map_err(|e| {
            app_error(
                crate::contracts::ErrorCode::Internal,
                format!("Réglage non enregistré ({e})."),
            )
        })
}

/// Recherche d'upgrades (Alt+U) : ouvre dans le navigateur une recherche de
/// trade pondérée pour l'emplacement de l'objet copié. Aucune requête n'est
/// envoyée par Augur.
#[tauri::command]
pub fn open_upgrade_search(
    character: State<'_, crate::character::CharacterState>,
    store: State<'_, crate::settings::SettingsStore>,
    item: ParsedItem,
) -> Result<crate::upgrade_search::UpgradeSearch, AppError> {
    let settings = store.get();
    let preparer = |b: Option<&poe2_core::pob::BuildPob>| {
        crate::upgrade_search::preparer(
            b,
            &item.item_class,
            settings.league(),
            settings.upgrade_budget,
        )
    };
    let recherche = character
        .with_build(|b| preparer(Some(b)))
        .unwrap_or_else(|| preparer(None))?;
    crate::upgrade_search::ouvrir(&recherche.url)?;
    Ok(recherche)
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
        price_check: shortcuts.price_check.trim().to_string(),
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
        .update(&app, |s| {
            s.set_client_log_path_for(crate::game::active(), path)
        })
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
    let info = crate::character::import(&app, &code).map_err(internal)?;
    if crate::game::active() == crate::game::Game::Poe1 {
        crate::poe1_calc::prechauffer(&app);
    }
    Ok(info)
}

#[tauri::command]
pub fn clear_character(app: AppHandle) -> Result<(), AppError> {
    crate::character::clear(&app).map_err(internal)
}

/// Personnage cible (build visé) du jeu actif.
#[tauri::command]
pub fn get_target(
    state: State<'_, crate::character::CharacterState>,
) -> Option<crate::character::CharacterInfo> {
    state.info_role(crate::character::Role::Cible)
}

/// Importe le personnage cible d'un code d'export Path of Building, comme
/// `import_pob_code`. Il ne sert qu'à l'écart : l'overlay compare toujours
/// au personnage actuel.
#[tauri::command]
pub fn import_target_code(
    app: AppHandle,
    code: String,
) -> Result<crate::character::CharacterInfo, AppError> {
    crate::character::import_role(&app, &code, crate::character::Role::Cible).map_err(internal)
}

#[tauri::command]
pub fn clear_target(app: AppHandle) -> Result<(), AppError> {
    crate::character::clear_role(&app, crate::character::Role::Cible).map_err(internal)
}

/// Écart entre le personnage actuel et la cible du jeu actif (`None` s'il
/// manque l'un des deux).
#[tauri::command]
pub fn get_gap(
    state: State<'_, crate::character::CharacterState>,
) -> Option<crate::gap::GapReport> {
    let game = crate::game::active();
    state.with_pair(game, |a, c| crate::gap::report(game, a, c))
}

/// L'objet copié en dernier face à l'objet de la cible pour son emplacement
/// (ligne « vers la cible » de l'overlay). `None` sans cible ou sans objet cible.
#[tauri::command]
pub fn get_target_progress(
    app: AppHandle,
    state: State<'_, crate::character::CharacterState>,
) -> Option<crate::gap::TargetProgress> {
    let texte = app.state::<crate::AppState>().last_text()?;
    state
        .with_target(crate::game::active(), |a, c| {
            crate::gap::progress(a, c, &texte)
        })
        .flatten()
}

/// Alt+U « vers la cible » : ouvre la recherche de l'objet de la cible pour
/// l'emplacement de l'objet copié (ou `slot`, s'il est donné).
#[tauri::command]
pub fn open_target_search(
    app: AppHandle,
    state: State<'_, crate::character::CharacterState>,
    store: State<'_, crate::settings::SettingsStore>,
    slot: Option<String>,
) -> Result<crate::gap::TargetSearch, AppError> {
    let game = crate::game::active();
    let settings = store.get();
    let texte = app.state::<crate::AppState>().last_text();
    let r = state
        .with_target(game, |a, c| {
            let slot = slot
                .clone()
                .or_else(|| {
                    texte
                        .as_deref()
                        .and_then(|t| crate::gap::progress(a, c, t))
                        .map(|p| p.slot)
                })
                .ok_or_else(|| {
                    app_error(
                        crate::contracts::ErrorCode::Unsupported,
                        "Ta cible n'a pas d'objet pour cet emplacement.",
                    )
                })?;
            crate::gap::target_search(game, c, &slot, &settings)
        })
        .unwrap_or_else(|| {
            Err(app_error(
                crate::contracts::ErrorCode::NoCharacter,
                "Importe un personnage cible dans Copilot.",
            ))
        })?;
    crate::upgrade_search::ouvrir(&r.url)?;
    Ok(r)
}

/// Alt+U : objet meilleur que celui porté (`false`) ou objet de la cible (`true`).
#[tauri::command]
pub fn set_upgrade_search_target(
    app: AppHandle,
    store: State<'_, crate::settings::SettingsStore>,
    enabled: bool,
) -> Result<crate::settings::Settings, AppError> {
    store
        .update(&app, |s| s.upgrade_search_target = enabled)
        .map_err(|e| {
            app_error(
                crate::contracts::ErrorCode::Internal,
                format!("Réglage non enregistré ({e})."),
            )
        })
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

/// Prix estimé de l'objet copié (cote poe.ninja des objets empilables).
#[tauri::command]
pub async fn estimate_price(
    app: AppHandle,
    item: ParsedItem,
) -> Result<crate::contracts::PriceEstimate, AppError> {
    crate::poe2_price::estimer(&app, &item).await
}

/// Cote trade d'un unique ou d'un rare, sur le raccourci du joueur.
#[tauri::command]
pub async fn trade_price_check(
    app: AppHandle,
    item: ParsedItem,
) -> Result<crate::trade_price::TradePriceCheck, AppError> {
    crate::trade_price::coter(&app, &item).await
}

/// Ouvre les annonces d'une cote trade dans le navigateur.
#[tauri::command]
pub fn open_trade_listings(url: String) -> Result<(), AppError> {
    crate::trade_price::ouvrir(&url)
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

/// Forge PoE 1 : bases craftables (équipement), avec leurs implicites et influences possibles.
#[tauri::command]
pub fn get_forge_poe1_bases() -> Result<Vec<poe2_core::poe1::forge::BaseVue>, AppError> {
    crate::forge_poe1::bases()
}

/// Forge PoE 1 : affixes d'une base à un niveau d'objet, influence comprise.
#[tauri::command]
pub fn get_forge_poe1_affixes(
    base_id: String,
    ilvl: u32,
    influence: Option<String>,
) -> Result<poe2_core::poe1::forge::AffixesPoe1, AppError> {
    crate::forge_poe1::affixes(&base_id, ilvl, influence.as_deref())
}

/// Forge PoE 1 : méthodes de spam et chance de réussite d'un essai.
#[tauri::command]
pub async fn plan_forge_poe1(
    app: AppHandle,
    demande: poe2_core::poe1::forge::DemandePoe1,
) -> Result<poe2_core::poe1::forge::ResultatPoe1, AppError> {
    crate::forge_poe1::planifier(&app, demande).await
}

/// Forge PoE 1 : conseil pour l'objet copié en jeu (plan suivi par l'overlay).
#[tauri::command]
pub fn get_forge_poe1_advice(
    state: State<'_, crate::AppState>,
    forge: State<'_, crate::forge_poe1::ForgePoe1State>,
) -> Result<Option<poe2_core::poe1::forge::suivi::ConseilPoe1>, AppError> {
    match state.last_text() {
        Some(text) => forge.conseil(&text),
        None => Ok(None),
    }
}

/// Forge PoE 1 : plans calculés et plan suivi.
#[tauri::command]
pub fn get_forge_poe1_history(
    forge: State<'_, crate::forge_poe1::ForgePoe1State>,
) -> crate::forge_poe1::HistoriquePoe1 {
    forge.historique()
}

#[tauri::command]
pub fn delete_forge_poe1_history_entry(
    app: AppHandle,
    forge: State<'_, crate::forge_poe1::ForgePoe1State>,
    id: u64,
) -> crate::forge_poe1::HistoriquePoe1 {
    forge.supprimer(&app, id)
}

/// Forge PoE 1 : suit un plan de l'historique (`None` : arrête le suivi).
#[tauri::command]
pub fn follow_forge_poe1_entry(
    app: AppHandle,
    forge: State<'_, crate::forge_poe1::ForgePoe1State>,
    id: Option<u64>,
) -> crate::forge_poe1::HistoriquePoe1 {
    forge.suivre(&app, id)
}

/// Forge PoE 1, « crafter ou acheter » : recherche de l'objet visé sur le site de trade de PoE 1.
#[tauri::command]
pub fn open_forge_poe1_buy_search(
    store: State<'_, crate::settings::SettingsStore>,
    demande: poe2_core::poe1::forge::DemandePoe1,
) -> Result<crate::upgrade_search::ForgeBuySearch, AppError> {
    let r = poe2_core::poe1::forge::achat::recherche(&demande)
        .map_err(|e| app_error(crate::contracts::ErrorCode::Unsupported, e.to_string()))?;
    let url = crate::upgrade_search::adresse_requete_poe1(
        store.get().league_for(crate::game::Game::Poe1),
        &r.requete,
    );
    crate::upgrade_search::ouvrir(&url)?;
    Ok(crate::upgrade_search::ForgeBuySearch {
        url,
        filtres: r.filtres.len(),
        ignorees: r.ignorees,
    })
}

/// Forge PoE 1 : objet de départ collé (Ctrl+Alt+C en jeu) : base, niveau, influence et mods lus.
#[tauri::command]
pub fn analyze_forge_poe1_item(
    texte: String,
) -> Result<poe2_core::poe1::forge::ObjetDepartPoe1, AppError> {
    crate::forge_poe1::analyser(&texte)
}

/// Forge PoE 1 : prix poe.ninja (Chaos) des monnaies, Essences, fossiles et résonateurs.
#[tauri::command]
pub async fn get_forge_poe1_prices(
    app: AppHandle,
) -> Result<crate::forge_poe1::PrixPoe1, AppError> {
    crate::forge_poe1::prix(&app).await
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

/// Objet de départ collé dans le panneau Forge (Ctrl+Alt+C en jeu) : base, niveau et mods lus.
#[tauri::command]
pub fn analyze_forge_item(
    texte: String,
) -> Result<poe2_core::forge::atelier::ObjetDepart, AppError> {
    poe2_core::forge::atelier::analyser_objet(&texte).map_err(crate::forge::error)
}

/// « Crafter ou acheter » : ouvre dans le navigateur la recherche de l'objet visé par le plan (même
/// base, chaque mod voulu au tier visé ou mieux, du moins cher au plus cher), dans la ligue des
/// Réglages. Aucune requête n'est envoyée par Augur : le joueur reporte le prix qu'il voit.
#[tauri::command]
pub fn open_forge_buy_search(
    store: State<'_, crate::settings::SettingsStore>,
    demande: poe2_core::forge::atelier::DemandePlan,
) -> Result<crate::upgrade_search::ForgeBuySearch, AppError> {
    let r = poe2_core::forge::achat::recherche(&demande).map_err(crate::forge::error)?;
    let url = crate::upgrade_search::adresse_requete(store.get().league(), &r.requete);
    crate::upgrade_search::ouvrir(&url)?;
    Ok(crate::upgrade_search::ForgeBuySearch {
        url,
        filtres: r.filtres.len(),
        ignorees: r.ignorees,
    })
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

/// Diagnostic affiché dans Réglages : résumé technique et journal d'Augur.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub resume: String,
    pub journal: Vec<String>,
}

#[tauri::command]
pub fn get_diagnostic(app: AppHandle) -> Diagnostic {
    Diagnostic {
        resume: crate::bug_report::diagnostic(&app),
        journal: crate::diag_log::lines(),
    }
}

/// Copie le résumé et le journal complet dans le presse-papier.
#[tauri::command]
pub fn copy_diagnostic(app: AppHandle) -> Result<(), AppError> {
    let mut text = crate::bug_report::diagnostic(&app);
    let journal = crate::diag_log::lines();
    text.push_str("\n\nJournal :\n");
    if journal.is_empty() {
        text.push_str("(vide)");
    } else {
        text.push_str(&journal.join("\n"));
    }
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text))
        .map_err(|e| {
            app_error(
                crate::contracts::ErrorCode::Internal,
                format!("Presse-papier indisponible ({e})."),
            )
        })
}

/// Ouvre dans le navigateur le ticket GitHub prérempli (le joueur le relit et l'envoie lui-même).
#[tauri::command]
pub fn open_bug_report(
    report: crate::bug_report::BugReport,
) -> Result<crate::bug_report::BugReportOpened, AppError> {
    let (url, truncated) = crate::bug_report::issue_url(&report)?;
    // Copie d'abord le rapport complet : si le navigateur n'applique pas le
    // modèle ou si un champ a été raccourci, le joueur n'a qu'à coller.
    let rapport = crate::bug_report::rapport_complet(&report);
    let copied = arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(rapport))
        .map_err(|e| log::warn!("rapport de bug non copié : {e}"))
        .is_ok();
    crate::bug_report::open_in_browser(&url)?;
    log::info!(
        "ticket de bug préparé dans le navigateur ({} caractères)",
        url.len()
    );
    Ok(crate::bug_report::BugReportOpened { truncated, copied })
}

/// Ouvre dans le navigateur le ticket GitHub d'idée prérempli (le joueur le relit et l'envoie lui-même).
#[tauri::command]
pub fn open_idea(
    report: crate::bug_report::IdeaReport,
) -> Result<crate::bug_report::BugReportOpened, AppError> {
    let (url, truncated) = crate::bug_report::idea_url(&report)?;
    let texte = crate::bug_report::idee_complete(&report);
    let copied = arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(texte))
        .map_err(|e| log::warn!("idée non copiée : {e}"))
        .is_ok();
    crate::bug_report::open_in_browser(&url)?;
    log::info!("ticket d'idée préparé dans le navigateur");
    Ok(crate::bug_report::BugReportOpened { truncated, copied })
}
