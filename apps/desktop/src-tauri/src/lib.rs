//! Application Windows Augur : coquille Tauri 2 de l'overlay en jeu
//! (Upgrade Copilot) et de la fenêtre principale.
//!
//! Sources d'entrée, toutes tolérées par GGG : presse-papier (Ctrl+C fait
//! par le joueur), raccourcis globaux (une action par pression, aucune
//! touche envoyée au jeu), fichier `Client.txt`, titre de la fenêtre au
//! premier plan. Aucune lecture mémoire, aucune modification des fichiers
//! du jeu, jamais de `POESESSID`.

use std::sync::Mutex;

use tauri::{AppHandle, Manager, WindowEvent};

#[cfg(test)]
mod acl_tests;
mod bug_report;
mod character;
mod client_log;
mod clipboard;
mod commands;
pub mod contracts;
mod diag_log;
mod evaluation;
mod forge;
mod forge_poe1;
mod forge_prices;
mod game;
mod game_window;
mod gap;
pub mod item_text;
mod ledger;
mod ledger_banner;
pub mod ledger_store;
mod log_locator;
pub mod measures;
mod overlay;
pub mod placement;
mod poe1_calc;
mod poe1_items;
mod poe1_stash;
mod poe2_price;
mod secret;
mod settings;
mod shortcuts;
mod time;
mod trade_price;
mod tray;
mod updates;
mod upgrade_search;

pub const MAIN_WINDOW: &str = "main";

/// État partagé de l'application.
#[derive(Default)]
pub struct AppState {
    last_text: Mutex<Option<String>>,
}

impl AppState {
    pub fn set_last_text(&self, text: String) {
        *self.last_text.lock().unwrap_or_else(|e| e.into_inner()) = Some(text);
    }

    pub fn last_text(&self) -> Option<String> {
        self.last_text
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    diag_log::init();
    tauri::Builder::default()
        // En premier : une seconde instance réveille la première au lieu de démarrer.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app)
        }))
        .plugin(shortcuts::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .manage(overlay::OverlayCtl::default())
        .manage(ledger_banner::BannerCtl::default())
        .manage(updates::PendingUpdate::default())
        .manage(client_log::ClientLogState::default())
        .manage(measures::MeasureState::default())
        .manage(poe1_items::Poe1Ninja::default())
        .manage(poe2_price::Poe2Ninja::default())
        .manage(trade_price::TradeBudget::default())
        .manage(poe1_calc::Poe1Calc::default())
        .invoke_handler(tauri::generate_handler![
            commands::parse_clipboard_item,
            commands::get_overlay_state,
            commands::hide_overlay,
            commands::fit_overlay,
            commands::set_overlay_display,
            commands::set_overlay_pinned,
            commands::set_ledger_banner,
            commands::open_main_window,
            commands::check_for_update,
            commands::install_update,
            commands::get_settings,
            commands::set_game,
            commands::get_poe1_item_info,
            commands::evaluate_poe1_item,
            commands::open_poe1_upgrade_search,
            commands::open_poe1_trade_search,
            commands::get_poe1_stash,
            commands::connect_poe1_stash,
            commands::forget_poe1_stash_session,
            commands::list_poe1_stash_tabs,
            commands::set_poe1_stash_tabs,
            commands::take_poe1_stash_snapshot,
            commands::get_leagues,
            commands::set_league,
            commands::set_upgrade_budget,
            commands::open_upgrade_search,
            commands::set_shortcuts,
            commands::set_update_channel,
            commands::get_client_log_status,
            commands::set_client_log_path,
            commands::get_measure_status,
            commands::start_measure_session,
            commands::stop_measure_session,
            commands::add_measure_note,
            commands::copy_measure_export,
            commands::get_ledger_status,
            commands::start_ledger_session,
            commands::stop_ledger_session,
            commands::set_ledger_strategy,
            commands::list_ledger_sessions,
            commands::get_ledger_session,
            commands::delete_ledger_session,
            commands::get_character,
            commands::evaluate_item,
            commands::estimate_price,
            commands::trade_price_check,
            commands::open_trade_listings,
            commands::import_pob_code,
            commands::clear_character,
            commands::get_target,
            commands::import_target_code,
            commands::clear_target,
            commands::get_gap,
            commands::get_target_progress,
            commands::open_target_search,
            commands::set_upgrade_search_target,
            commands::get_forge_prices,
            commands::set_forge_prices,
            commands::reset_forge_prices,
            commands::refresh_forge_prices,
            commands::get_forge_bases,
            commands::get_forge_poe1_bases,
            commands::get_forge_poe1_affixes,
            commands::plan_forge_poe1,
            commands::get_forge_poe1_advice,
            commands::get_forge_poe1_history,
            commands::delete_forge_poe1_history_entry,
            commands::follow_forge_poe1_entry,
            commands::open_forge_poe1_buy_search,
            commands::analyze_forge_poe1_item,
            commands::get_forge_poe1_prices,
            commands::get_forge_affixes,
            commands::analyze_forge_item,
            commands::open_forge_buy_search,
            commands::get_forge_catalogue,
            commands::plan_forge_craft,
            commands::get_forge_advice,
            commands::get_forge_tracking,
            commands::stop_forge_tracking,
            commands::get_forge_history,
            commands::delete_forge_history_entry,
            commands::get_diagnostic,
            commands::copy_diagnostic,
            commands::open_bug_report,
            commands::open_idea,
        ])
        .setup(|app| {
            let handle = app.handle();
            if let Ok(dir) = handle.path().app_log_dir() {
                diag_log::attach_file(&dir);
            }
            log::info!("démarrage d'Augur v{}", handle.package_info().version);
            app.manage(settings::SettingsStore::load(handle));
            // Avant le Ledger et le suivi de Client.txt, qui dépendent du jeu actif.
            game::set_active(handle.state::<settings::SettingsStore>().get().game);
            app.manage(character::CharacterState::load(handle));
            // PoB PoE 1 met quelques secondes à démarrer : prêt avant le premier Ctrl+C.
            if game::active() == game::Game::Poe1 {
                poe1_calc::prechauffer(handle);
            }
            app.manage(ledger::LedgerState::open(handle));
            app.manage(poe1_stash::Poe1Stash::load(handle));
            app.manage(forge_prices::ForgePrices::load(handle));
            app.manage(forge::ForgePlanner::load(handle));
            app.manage(forge_poe1::ForgePoe1State::load(handle));
            // plan suivi avant la fermeture : recalculé en arrière-plan, aux prix du moment
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                let prix = h.state::<forge_prices::ForgePrices>().get();
                h.state::<forge::ForgePlanner>().restaurer(&h, prix).await;
            });
            overlay::prepare(handle)?;
            ledger_banner::prepare(handle)?;
            tray::create(handle)?;
            clipboard::spawn_watcher(handle.clone());
            client_log::spawn(handle.clone());
            updates::spawn_periodic_check(handle.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fermer la fenêtre principale la cache : l'overlay continue de tourner.
            if window.label() == MAIN_WINDOW
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("erreur au lancement d'Augur");
}
