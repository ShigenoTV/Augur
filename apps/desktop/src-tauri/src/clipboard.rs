//! Surveillance du presse-papier : quand le joueur fait Ctrl+C sur un objet
//! dans le jeu, le texte de l'objet arrive dans le presse-papier. Augur le
//! lit, sans jamais simuler de touche ni écrire dans le presse-papier.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, game_window, item_text, overlay, time};

pub const EVENT_ITEM_COPIED: &str = "augur://item-copied";

const POLL: Duration = Duration::from_millis(60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCopied {
    pub text: String,
    pub captured_at_ms: u64,
}

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
                if !game_window::game_is_foreground() {
                    continue;
                }
                if clipboard.is_none() {
                    clipboard = arboard::Clipboard::new().ok();
                }
                let Some(text) = clipboard.as_mut().and_then(|c| c.get_text().ok()) else {
                    continue;
                };
                if !item_text::looks_like_item(&text) {
                    continue;
                }
                app.state::<AppState>().set_last_text(text.clone());
                let payload = ItemCopied {
                    text,
                    captured_at_ms: time::unix_millis_now(),
                };
                let _ = app.emit_to(overlay::LABEL, EVENT_ITEM_COPIED, payload);
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || overlay::show_near_cursor(&handle));
            }
        });
    if let Err(err) = spawned {
        log::error!("impossible de surveiller le presse-papier : {err}");
    }
}
