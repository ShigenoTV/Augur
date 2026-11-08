//! Surveillance du presse-papier : quand le joueur fait Ctrl+C sur un objet
//! dans le jeu, le texte de l'objet arrive dans le presse-papier. Augur le
//! lit, sans jamais simuler de touche ni écrire dans le presse-papier.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::contracts::events;
use crate::measures::{self, MeasureState};
use crate::{AppState, game_window, item_text, overlay};

const POLL: Duration = Duration::from_millis(60);
/// Événement propre à l'appli (hors contrat) : charge `AppError`, émis quand
/// le texte copié ressemble à un objet mais que le parseur le refuse.
pub const EVENT_ITEM_UNRECOGNIZED: &str = "augur://item-unrecognized";

/// Jeton qui change à chaque modification du presse-papier.
#[cfg(windows)]
fn change_token(_clipboard: &mut Option<arboard::Clipboard>) -> u64 {
    // SAFETY : lecture d'un compteur système, sans effet de bord.
    u64::from(unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() })
}

#[cfg(not(windows))]
fn change_token(clipboard: &mut Option<arboard::Clipboard>) -> u64 {
    use std::hash::{Hash, Hasher};
    let text = clipboard
        .as_mut()
        .and_then(|c| c.get_text().ok())
        .unwrap_or_default();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

pub fn spawn_watcher(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("augur-clipboard".into())
        .spawn(move || {
            let mut clipboard = arboard::Clipboard::new().ok();
            let mut last = change_token(&mut clipboard);
            loop {
                std::thread::sleep(POLL);
                let token = change_token(&mut clipboard);
                if token == last {
                    continue;
                }
                last = token;
                // Pendant une session de mesures, les codes Path of Building
                // copiés depuis PoB (jeu pas au premier plan) sont aussi gardés.
                let measuring = app.state::<MeasureState>().is_active();
                let in_game = game_window::game_is_foreground();
                if !in_game && !measuring {
                    continue;
                }
                if clipboard.is_none() {
                    clipboard = arboard::Clipboard::new().ok();
                }
                let Some(text) = clipboard.as_mut().and_then(|c| c.get_text().ok()) else {
                    continue;
                };
                if measuring && measures::looks_like_pob_code(&text) {
                    measures::record_pob_code(&app, &text);
                    continue;
                }
                if !in_game || !item_text::looks_like_item(&text) {
                    continue;
                }
                // Contrat : `augur://item-copied` porte l'objet déjà parsé.
                let parsed = item_text::parse(&text);
                if measuring {
                    let result = parsed.clone().map_err(|e| e.message.clone());
                    measures::record_item(&app, &text, &result);
                }
                let item = match parsed {
                    Ok(item) => item,
                    Err(err) => {
                        log::warn!("objet copié illisible : {}", err.message);
                        // L'overlay affiche l'état « objet non reconnu » avec la raison.
                        let _ = app.emit_to(overlay::LABEL, EVENT_ITEM_UNRECOGNIZED, err);
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || overlay::show_near_cursor(&handle));
                        continue;
                    }
                };
                app.state::<AppState>().set_last_text(text);
                // La fenêtre principale garde le dernier objet analysé (page Copilot).
                let _ = app.emit_to("main", events::ITEM_COPIED, item.clone());
                let _ = app.emit_to(overlay::LABEL, events::ITEM_COPIED, item);
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || overlay::show_near_cursor(&handle));
            }
        });
    if let Err(err) = spawned {
        log::error!("impossible de surveiller le presse-papier : {err}");
    }
}
