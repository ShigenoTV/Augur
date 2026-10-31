//! Fenêtre de l'overlay : affichage près du curseur SANS prendre le focus au
//! jeu, fermeture quand le curseur s'éloigne, raccourcis actifs seulement
//! pendant l'affichage.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

use crate::placement::{self, Rect};
use crate::shortcuts;

pub const LABEL: &str = "overlay";
pub const EVENT_HIDDEN: &str = "augur://overlay-hidden";

const GUARD_POLL: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct OverlayCtl {
    visible: AtomicBool,
    anchor: Mutex<Option<(f64, f64)>>,
}

impl OverlayCtl {
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Réglages natifs de la fenêtre, une fois au démarrage.
pub fn prepare(app: &AppHandle) -> tauri::Result<()> {
    if let Some(win) = window(app) {
        win.set_ignore_cursor_events(false)?;
        native::make_non_activating(&win);
    }
    Ok(())
}

/// Affiche l'overlay à côté du curseur. Appelé après un Ctrl+C sur un objet.
pub fn show_near_cursor(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let ctl = app.state::<OverlayCtl>();

    if let Ok(cursor) = app.cursor_position() {
        let size = win
            .outer_size()
            .map(|s| (s.width as f64, s.height as f64))
            .unwrap_or((380.0, 600.0));
        let screen = win
            .monitor_from_point(cursor.x, cursor.y)
            .ok()
            .flatten()
            .or_else(|| win.current_monitor().ok().flatten())
            .map(|m| Rect {
                x: m.position().x as f64,
                y: m.position().y as f64,
                width: m.size().width as f64,
                height: m.size().height as f64,
            });
        if let Some(screen) = screen {
            let (x, y) = placement::place_panel((cursor.x, cursor.y), size, screen);
            let _ = win.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
        }
        *ctl.anchor.lock().unwrap_or_else(|e| e.into_inner()) = Some((cursor.x, cursor.y));
    }

    native::show_without_focus(&win);
    let was_visible = ctl.visible.swap(true, Ordering::AcqRel);
    if !was_visible {
        shortcuts::register_overlay(app);
        spawn_dismiss_guard(app.clone());
    }
}

pub fn hide(app: &AppHandle) {
    let ctl = app.state::<OverlayCtl>();
    if !ctl.visible.swap(false, Ordering::AcqRel) {
        return;
    }
    shortcuts::unregister_overlay(app);
    if let Some(win) = window(app) {
        let _ = win.hide();
    }
    let _ = app.emit_to(LABEL, EVENT_HIDDEN, ());
}

/// Ajuste la taille de la fenêtre au panneau rendu (hors zone transparente,
/// qui capterait sinon les clics destinés au jeu).
pub fn fit(app: &AppHandle, width: f64, height: f64) {
    if let Some(win) = window(app) {
        let w = width.clamp(200.0, 720.0);
        let h = height.clamp(80.0, 1200.0);
        let _ = win.set_size(tauri::LogicalSize::new(w, h));
    }
}

fn spawn_dismiss_guard(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("augur-overlay-guard".into())
        .spawn(move || {
            loop {
                std::thread::sleep(GUARD_POLL);
                let ctl = app.state::<OverlayCtl>();
                if !ctl.is_visible() {
                    return;
                }
                let anchor = *ctl.anchor.lock().unwrap_or_else(|e| e.into_inner());
                let (Some(anchor), Ok(cursor), Some(win)) =
                    (anchor, app.cursor_position(), window(&app))
                else {
                    continue;
                };
                let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else {
                    continue;
                };
                let panel = Rect {
                    x: pos.x as f64,
                    y: pos.y as f64,
                    width: size.width as f64,
                    height: size.height as f64,
                };
                if placement::should_dismiss(anchor, (cursor.x, cursor.y), panel) {
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || hide(&handle));
                    return;
                }
            }
        });
}

#[cfg(windows)]
mod native {
    use tauri::WebviewWindow;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
    };

    fn hwnd(win: &WebviewWindow) -> Option<HWND> {
        // Tauri peut dépendre d'une autre version du crate `windows` : on ne
        // garde que le pointeur brut.
        win.hwnd().ok().map(|h| HWND(h.0))
    }

    /// Le jeu garde le focus clavier même quand on clique dans l'overlay.
    pub fn make_non_activating(win: &WebviewWindow) {
        let Some(hwnd) = hwnd(win) else { return };
        // SAFETY : hwnd vient de Tauri et désigne notre propre fenêtre.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                style | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize,
            );
        }
    }

    pub fn show_without_focus(win: &WebviewWindow) {
        let Some(hwnd) = hwnd(win) else {
            let _ = win.show();
            return;
        };
        // SAFETY : idem, notre propre fenêtre.
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(not(windows))]
mod native {
    use tauri::WebviewWindow;

    pub fn make_non_activating(_win: &WebviewWindow) {}

    pub fn show_without_focus(win: &WebviewWindow) {
        let _ = win.show();
    }
}
