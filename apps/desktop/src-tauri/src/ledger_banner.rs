//! Bandeau Atlas Ledger : une ligne fine en haut de l'écran pendant une
//! session (durée, maps, gain), au-dessus du jeu.
//!
//! La fenêtre laisse passer tous les clics (le jeu les reçoit), ne prend
//! jamais le focus et ne s'affiche que si l'option est active (Réglages), une
//! session est en cours et le jeu est au premier plan.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindow};

use crate::overlay::native;
use crate::placement::{self, Rect};
use crate::settings::{Settings, SettingsStore};

pub const LABEL: &str = "ledger-banner";

/// Écart avec le bord haut de l'écran, en pixels logiques.
const TOP_MARGIN: f64 = 4.0;
const POLL: Duration = Duration::from_millis(500);

#[derive(Default)]
pub struct BannerCtl {
    /// Copie de `OverlaySettings::ledger_banner`.
    enabled: AtomicBool,
    shown: AtomicBool,
}

/// Le bandeau est à l'écran seulement si les trois conditions sont réunies.
pub fn should_show(enabled: bool, session_active: bool, game_foreground: bool) -> bool {
    enabled && session_active && game_foreground
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Réglages natifs de la fenêtre et garde d'affichage, une fois au démarrage.
pub fn prepare(app: &AppHandle) -> tauri::Result<()> {
    if let Some(win) = window(app) {
        // Clics traversants : le bandeau ne bloque jamais le jeu.
        win.set_ignore_cursor_events(true)?;
        native::make_non_activating(&win);
    }
    let enabled = app
        .try_state::<SettingsStore>()
        .is_none_or(|s| s.get().overlay.ledger_banner);
    app.state::<BannerCtl>()
        .enabled
        .store(enabled, Ordering::Release);
    spawn_guard(app.clone());
    Ok(())
}

/// Active ou coupe le bandeau (Réglages).
pub fn set_enabled(app: &AppHandle, enabled: bool) -> std::io::Result<Settings> {
    let settings = app
        .state::<SettingsStore>()
        .update(app, |s| s.overlay.ledger_banner = enabled)?;
    app.state::<BannerCtl>()
        .enabled
        .store(enabled, Ordering::Release);
    crate::overlay::emit_settings(app, &settings);
    Ok(settings)
}

fn monitor_rect(m: &tauri::Monitor) -> Rect {
    Rect {
        x: m.position().x as f64,
        y: m.position().y as f64,
        width: m.size().width as f64,
        height: m.size().height as f64,
    }
}

/// Affiche le bandeau en haut de l'écran du curseur (celui du jeu).
fn show(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|c| win.monitor_from_point(c.x, c.y).ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten());
    let pos = monitor.and_then(|m| {
        let width = win.outer_size().ok()?.width as f64;
        let (x, y) = placement::top_center(width, monitor_rect(&m), TOP_MARGIN * m.scale_factor());
        Some((x.round() as i32, y.round() as i32))
    });
    native::show_without_focus(&win, pos);
}

fn hide(app: &AppHandle) {
    if let Some(win) = window(app) {
        native::hide_quiet(&win);
    }
}

fn spawn_guard(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("augur-ledger-banner".into())
        .spawn(move || {
            loop {
                std::thread::sleep(POLL);
                let ctl = app.state::<BannerCtl>();
                let enabled = ctl.enabled.load(Ordering::Acquire);
                // Le jeu n'est interrogé que si le reste l'exige.
                let want = should_show(
                    enabled,
                    enabled && app.state::<crate::ledger::LedgerState>().is_active(),
                    enabled && crate::game_window::game_is_foreground(),
                );
                if ctl.shown.swap(want, Ordering::AcqRel) == want {
                    continue;
                }
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    if want {
                        show(&handle);
                    } else {
                        hide(&handle);
                    }
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_seulement_en_jeu_pendant_une_session() {
        assert!(should_show(true, true, true));
        assert!(!should_show(false, true, true), "option coupée");
        assert!(!should_show(true, false, true), "pas de session");
        assert!(!should_show(true, true, false), "jeu en arrière-plan");
    }
}
