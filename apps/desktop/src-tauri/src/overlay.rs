//! Fenêtre de l'overlay : affichage près du curseur SANS prendre le focus au
//! jeu, fermeture quand le curseur s'éloigne, raccourcis actifs seulement
//! pendant l'affichage.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::placement::{self, Rect};
use crate::shortcuts;

pub const LABEL: &str = "overlay";
pub const EVENT_HIDDEN: &str = "augur://overlay-hidden";

const GUARD_POLL: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct OverlayCtl {
    visible: AtomicBool,
    anchor: Mutex<Option<(f64, f64)>>,
    /// Dernière position à l'écran : le masquage envoie la fenêtre hors écran.
    last_pos: Mutex<Option<(i32, i32)>>,
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
            *ctl.last_pos.lock().unwrap_or_else(|e| e.into_inner()) =
                Some((x.round() as i32, y.round() as i32));
        }
        *ctl.anchor.lock().unwrap_or_else(|e| e.into_inner()) = Some((cursor.x, cursor.y));
    }

    let pos = *ctl.last_pos.lock().unwrap_or_else(|e| e.into_inner());
    native::show_without_focus(&win, pos);
    let was_visible = ctl.visible.swap(true, Ordering::AcqRel);
    if !was_visible {
        shortcuts::register_overlay(app);
        spawn_dismiss_guard(app.clone());
    }
}

pub fn hide(app: &AppHandle) {
    let ctl = app.state::<OverlayCtl>();
    let was_visible = ctl.visible.swap(false, Ordering::AcqRel);
    // Toujours masquer la fenêtre, même si l'état la croit déjà cachée :
    // une fenêtre restée à l'écran par erreur bloquerait les clics du jeu.
    if let Some(win) = window(app) {
        native::hide(&win);
    }
    if !was_visible {
        return;
    }
    shortcuts::unregister_overlay(app);
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
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, HWND_TOPMOST, IsWindowVisible, SW_HIDE,
        SW_SHOWNOACTIVATE, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
    };

    /// Coordonnée hors de tout écran (convention Windows des fenêtres réduites).
    const OFF_SCREEN: i32 = -32000;

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

    /// Affiche l'overlay à `pos` (coin haut gauche, pixels physiques), au
    /// premier plan, sans prendre le focus. Position et affichage en un seul
    /// appel synchrone : la fenêtre n'apparaît jamais ailleurs.
    pub fn show_without_focus(win: &WebviewWindow, pos: Option<(i32, i32)>) {
        let Some(hwnd) = hwnd(win) else {
            if let Some((x, y)) = pos {
                let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
            }
            let _ = win.show();
            return;
        };
        show_hwnd(hwnd, pos);
    }

    /// Masque l'overlay par la même voie Win32 que son affichage.
    ///
    /// `WebviewWindow::hide()` ne suffit PAS : l'overlay est affiché par
    /// Win32 en direct (pour ne pas voler le focus au jeu), donc Tauri le
    /// croit toujours caché et son `hide()` ne fait rien. En plus du
    /// masquage, la fenêtre part hors écran : même si quelque chose la
    /// réaffichait sans contenu, elle ne pourrait plus capter les clics du jeu.
    pub fn hide(win: &WebviewWindow) {
        match hwnd(win) {
            Some(hwnd) => {
                hide_hwnd(hwnd);
                // SAFETY : hwnd désigne notre propre fenêtre.
                let still_visible = unsafe { IsWindowVisible(hwnd).as_bool() };
                let mut rect = RECT::default();
                // SAFETY : idem.
                let _ = unsafe { GetWindowRect(hwnd, &mut rect) };
                if still_visible {
                    log::warn!(
                        "overlay encore visible après masquage ({}, {}, {}, {})",
                        rect.left,
                        rect.top,
                        rect.right,
                        rect.bottom
                    );
                } else {
                    log::info!("overlay masqué (fenêtre en {}, {})", rect.left, rect.top);
                }
            }
            None => {
                let _ = win.hide();
            }
        }
    }

    pub(super) fn show_hwnd(hwnd: HWND, pos: Option<(i32, i32)>) {
        let (x, y) = pos.unwrap_or((0, 0));
        let mut flags = SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW;
        if pos.is_none() {
            flags |= SWP_NOMOVE;
        }
        // SAFETY : hwnd désigne notre propre fenêtre.
        unsafe {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, 0, 0, flags);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
    }

    pub(super) fn hide_hwnd(hwnd: HWND) {
        // SAFETY : hwnd désigne notre propre fenêtre.
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = SetWindowPos(
                hwnd,
                None,
                OFF_SCREEN,
                OFF_SCREEN,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_HIDEWINDOW,
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, IsWindowVisible, WINDOW_EX_STYLE, WS_POPUP,
        };
        use windows::core::w;

        /// Après un affichage sans focus puis un masquage, la fenêtre n'est
        /// plus visible, donc ne capte plus aucun clic.
        #[test]
        fn hide_undoes_show_without_focus() {
            // SAFETY : fenêtre de test créée et détruite ici.
            unsafe {
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE(WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0),
                    w!("STATIC"),
                    w!("augur-overlay-test"),
                    WS_POPUP,
                    0,
                    0,
                    100,
                    100,
                    None,
                    None,
                    None,
                    None,
                )
                .expect("fenêtre de test");
                let rect = |hwnd| {
                    let mut r = RECT::default();
                    GetWindowRect(hwnd, &mut r).expect("rect");
                    r
                };
                show_hwnd(hwnd, Some((120, 80)));
                assert!(IsWindowVisible(hwnd).as_bool(), "affichée");
                assert_eq!(
                    (rect(hwnd).left, rect(hwnd).top),
                    (120, 80),
                    "à la position demandée"
                );
                hide_hwnd(hwnd);
                assert!(!IsWindowVisible(hwnd).as_bool(), "masquée");
                assert_eq!(rect(hwnd).left, OFF_SCREEN, "hors écran une fois masquée");
                // Même réaffichée sans position, elle ne revient pas sur le jeu.
                show_hwnd(hwnd, None);
                assert!(IsWindowVisible(hwnd).as_bool(), "réaffichée");
                assert_eq!(
                    rect(hwnd).left,
                    OFF_SCREEN,
                    "reste hors écran sans position"
                );
                show_hwnd(hwnd, Some((200, 150)));
                assert_eq!(
                    (rect(hwnd).left, rect(hwnd).top),
                    (200, 150),
                    "revient à la position demandée"
                );
                hide_hwnd(hwnd);
                hide_hwnd(hwnd);
                assert!(
                    !IsWindowVisible(hwnd).as_bool(),
                    "masquer deux fois reste masqué"
                );
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

#[cfg(not(windows))]
mod native {
    use tauri::WebviewWindow;

    pub fn make_non_activating(_win: &WebviewWindow) {}

    pub fn show_without_focus(win: &WebviewWindow, pos: Option<(i32, i32)>) {
        if let Some((x, y)) = pos {
            let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
        }
        let _ = win.show();
    }

    pub fn hide(win: &WebviewWindow) {
        let _ = win.hide();
    }
}
