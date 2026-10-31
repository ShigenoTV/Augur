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

mod client_log;
mod clipboard;
mod commands;
pub mod contracts;
mod game_window;
pub mod item_text;
mod overlay;
pub mod placement;
mod settings;
mod shortcuts;
mod time;
mod tray;
mod updates;

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
    tauri::Builder::default()
        // En premier : une seconde instance réveille la première au lieu de démarrer.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app)
        }))
        .plugin(shortcuts::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .manage(overlay::OverlayCtl::default())
        .manage(updates::PendingUpdate::default())
        .invoke_handler(tauri::generate_handler![
            commands::parse_clipboard_item,
            commands::get_overlay_state,
            commands::hide_overlay,
            commands::fit_overlay,
            commands::open_main_window,
            commands::check_for_update,
            commands::install_update,
            commands::get_settings,
            commands::set_update_channel,
        ])
        .setup(|app| {
            let handle = app.handle();
            app.manage(settings::SettingsStore::load(handle));
            overlay::prepare(handle)?;
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
