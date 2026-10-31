//! Raccourcis globaux. Règle GGG : une pression de touche ne déclenche
//! jamais plus d'une action, et Augur n'envoie aucune touche au jeu.
//!
//! Les raccourcis de l'overlay (Échap, Alt+U) ne sont enregistrés que
//! pendant son affichage, pour ne pas confisquer ces touches au jeu.

use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::overlay;

pub const EVENT_UPGRADE_SEARCH: &str = "augur://upgrade-search-requested";

pub fn close_overlay() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

pub fn upgrade_search() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::KeyU)
}

/// Plugin avec le gestionnaire unique de tous les raccourcis d'Augur.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if *shortcut == close_overlay() {
                overlay::hide(app);
            } else if *shortcut == upgrade_search() {
                // Une seule action : demander à l'interface de préparer la
                // recherche. La requête de trade part ensuite du client, une fois.
                let _ = app.emit_to(overlay::LABEL, EVENT_UPGRADE_SEARCH, ());
            }
        })
        .build()
}

pub fn register_overlay(app: &AppHandle) {
    for s in [close_overlay(), upgrade_search()] {
        if let Err(err) = app.global_shortcut().register(s) {
            log::warn!("raccourci {s:?} indisponible : {err}");
        }
    }
}

pub fn unregister_overlay(app: &AppHandle) {
    for s in [close_overlay(), upgrade_search()] {
        let _ = app.global_shortcut().unregister(s);
    }
}
