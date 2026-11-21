//! Fenêtre de l'overlay : affichage près du curseur SANS prendre le focus au
//! jeu, fermeture quand le curseur s'éloigne, raccourcis actifs seulement
//! pendant l'affichage.
//!
//! Épinglé (Réglages ou bouton de l'overlay), l'overlay ne se ferme plus quand
//! le curseur s'éloigne et reste à la place où le joueur l'a glissé.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::placement::{self, Rect};
use crate::settings::{Settings, SettingsStore};
use crate::shortcuts;

pub const LABEL: &str = "overlay";
pub const EVENT_HIDDEN: &str = "augur://overlay-hidden";
/// Affichage de l'overlay changé (charge : `OverlaySettings`), pour les deux fenêtres.
pub const EVENT_SETTINGS: &str = "augur://overlay-settings";

/// En deçà, la fenêtre est la fenêtre masquée envoyée hors écran.
const OFF_SCREEN_LIMIT: i32 = -16000;

const GUARD_POLL: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct OverlayCtl {
    visible: AtomicBool,
    anchor: Mutex<Option<(f64, f64)>>,
    /// Dernière position à l'écran : le masquage envoie la fenêtre hors écran.
    last_pos: Mutex<Option<(i32, i32)>>,
    /// Copie de `OverlaySettings::pinned`, lue par la garde toutes les 100 ms.
    pinned: AtomicBool,
}

impl OverlayCtl {
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned.load(Ordering::Acquire)
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
    if let Some(store) = app.try_state::<SettingsStore>() {
        let pinned = store.get().overlay.pinned;
        app.state::<OverlayCtl>()
            .pinned
            .store(pinned, Ordering::Release);
    }
    Ok(())
}

/// Place actuelle de l'overlay s'il est à l'écran (pixels physiques).
fn on_screen_position(win: &WebviewWindow) -> Option<(i32, i32)> {
    win.outer_position()
        .ok()
        .map(|p| (p.x, p.y))
        .filter(|&(x, y)| x > OFF_SCREEN_LIMIT && y > OFF_SCREEN_LIMIT)
}

fn monitor_rect(m: &tauri::Monitor) -> Rect {
    Rect {
        x: m.position().x as f64,
        y: m.position().y as f64,
        width: m.size().width as f64,
        height: m.size().height as f64,
    }
}

/// Place enregistrée de l'overlay épinglé, ramenée dans son écran ; `None`
/// si cet écran n'existe plus (écran débranché, résolution changée).
fn pinned_position(win: &WebviewWindow, app: &AppHandle, size: (f64, f64)) -> Option<(f64, f64)> {
    let (x, y) = app
        .try_state::<SettingsStore>()?
        .get()
        .overlay
        .pinned_position?;
    let monitor = win.monitor_from_point(x as f64, y as f64).ok().flatten()?;
    Some(placement::keep_inside(
        (x as f64, y as f64),
        size,
        monitor_rect(&monitor),
    ))
}

/// Enregistre la place de l'overlay épinglé, si elle a changé.
fn save_pinned_position(app: &AppHandle, pos: (i32, i32)) {
    let Some(store) = app.try_state::<SettingsStore>() else {
        return;
    };
    if store.get().overlay.pinned_position == Some(pos) {
        return;
    }
    if let Err(err) = store.update(app, |s| s.overlay.pinned_position = Some(pos)) {
        log::warn!("place de l'overlay épinglé non enregistrée : {err}");
    }
}

/// Épingle ou libère l'overlay. Épinglé à l'écran, il garde sa place
/// actuelle ; libéré à l'écran, il se fermera quand le curseur s'éloignera
/// de l'endroit où il se trouve maintenant.
pub fn set_pinned(app: &AppHandle, pinned: bool) -> std::io::Result<Settings> {
    let ctl = app.state::<OverlayCtl>();
    let visible = ctl.is_visible();
    let pos = window(app).as_ref().and_then(on_screen_position);
    let settings = app.state::<SettingsStore>().update(app, |s| {
        s.overlay.pinned = pinned;
        if pinned && visible && pos.is_some() {
            s.overlay.pinned_position = pos;
        }
    })?;
    ctl.pinned.store(pinned, Ordering::Release);
    if !pinned
        && visible
        && let Ok(cursor) = app.cursor_position()
    {
        *ctl.anchor.lock().unwrap_or_else(|e| e.into_inner()) = Some((cursor.x, cursor.y));
    }
    emit_settings(app, &settings);
    Ok(settings)
}

/// Prévient les deux fenêtres d'un changement d'affichage de l'overlay.
pub fn emit_settings(app: &AppHandle, settings: &Settings) {
    let _ = app.emit(EVENT_SETTINGS, settings.overlay.clone());
}

/// Affiche l'overlay à côté du curseur. Appelé après un Ctrl+C sur un objet.
pub fn show_near_cursor(app: &AppHandle) {
    let Some(win) = window(app) else { return };
    let ctl = app.state::<OverlayCtl>();
    let size = win
        .outer_size()
        .map(|s| (s.width as f64, s.height as f64))
        .unwrap_or((380.0, 600.0));
    let set_pos = |(x, y): (f64, f64)| {
        *ctl.last_pos.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((x.round() as i32, y.round() as i32));
    };
    // Épinglé et déjà affiché : le nouvel objet s'affiche à la même place.
    let stay = ctl.is_pinned() && ctl.is_visible();
    let pinned_at = if ctl.is_pinned() && !stay {
        pinned_position(&win, app, size)
    } else {
        None
    };
    if let Some(p) = pinned_at {
        set_pos(p);
    }

    if let Ok(cursor) = app.cursor_position() {
        if !stay && pinned_at.is_none() {
            let screen = win
                .monitor_from_point(cursor.x, cursor.y)
                .ok()
                .flatten()
                .or_else(|| win.current_monitor().ok().flatten())
                .map(|m| monitor_rect(&m));
            if let Some(screen) = screen {
                set_pos(placement::place_panel((cursor.x, cursor.y), size, screen));
            }
        }
        *ctl.anchor.lock().unwrap_or_else(|e| e.into_inner()) = Some((cursor.x, cursor.y));
    }

    let pos = if stay {
        None
    } else {
        *ctl.last_pos.lock().unwrap_or_else(|e| e.into_inner())
    };
    native::show_without_focus(&win, pos);
    let was_visible = ctl.visible.swap(true, Ordering::AcqRel);
    if !was_visible {
        shortcuts::register_overlay(app);
        spawn_dismiss_guard(app.clone());
    }
}

pub fn hide(app: &AppHandle) {
    let ctl = app.state::<OverlayCtl>();
    // Épinglé : la place choisie (glissée) sert au prochain affichage.
    if ctl.is_pinned()
        && ctl.is_visible()
        && let Some(pos) = window(app).as_ref().and_then(on_screen_position)
    {
        save_pinned_position(app, pos);
    }
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
                if ctl.is_pinned() {
                    continue;
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
pub(crate) mod native {
    use tauri::WebviewWindow;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        GA_ROOT, GWL_EXSTYLE, GetAncestor, GetClassNameW, GetWindowLongPtrW, GetWindowRect,
        GetWindowThreadProcessId, HWND_TOPMOST, IsWindowVisible, SW_HIDE, SW_SHOWNOACTIVATE,
        SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WindowFromPoint,
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
    /// Masque une fenêtre affichée par [`show_without_focus`], sans le
    /// diagnostic de l'overlay (bandeau Ledger, qui laisse passer les clics).
    pub fn hide_quiet(win: &WebviewWindow) {
        match hwnd(win) {
            Some(hwnd) => hide_hwnd(hwnd),
            None => {
                let _ = win.hide();
            }
        }
    }

    pub fn hide(win: &WebviewWindow) {
        match hwnd(win) {
            Some(hwnd) => {
                let mut before = RECT::default();
                // SAFETY : hwnd désigne notre propre fenêtre.
                let _ = unsafe { GetWindowRect(hwnd, &mut before) };
                hide_hwnd(hwnd);
                if before.left > OFF_SCREEN / 2 {
                    spawn_blocker_probe(before);
                }
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

    /// Diagnostic : peu après le masquage, note dans le journal quelle fenêtre
    /// se trouve sous l'ancienne place de l'overlay (classe, et si elle
    /// appartient à Augur). Si la zone reste bloquée, on sait qui la bloque.
    fn spawn_blocker_probe(area: RECT) {
        let _ = std::thread::Builder::new()
            .name("augur-overlay-probe".into())
            .spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(400));
                let point = POINT {
                    x: (area.left + area.right) / 2,
                    y: (area.top + area.bottom) / 2,
                };
                // SAFETY : lecture seule de l'état des fenêtres du bureau.
                unsafe {
                    let hit = WindowFromPoint(point);
                    if hit.is_invalid() {
                        log::info!("sous l'ancien overlay : aucune fenêtre");
                        return;
                    }
                    let root = GetAncestor(hit, GA_ROOT);
                    let describe = |h: HWND| {
                        let mut buf = [0u16; 128];
                        let n = GetClassNameW(h, &mut buf).max(0) as usize;
                        let mut pid = 0u32;
                        GetWindowThreadProcessId(h, Some(&mut pid));
                        let ours = pid == GetCurrentProcessId();
                        let mut r = RECT::default();
                        let _ = GetWindowRect(h, &mut r);
                        format!(
                            "classe « {} », {}, visible {}, rect ({}, {}, {}, {})",
                            String::from_utf16_lossy(&buf[..n]),
                            if ours { "fenêtre d'Augur" } else { "autre programme" },
                            IsWindowVisible(h).as_bool(),
                            r.left,
                            r.top,
                            r.right,
                            r.bottom
                        )
                    };
                    let (hit_txt, root_txt) = (describe(hit), describe(root));
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(root, Some(&mut pid));
                    if pid == GetCurrentProcessId() {
                        log::warn!(
                            "sous l'ancien overlay, une fenêtre d'Augur capte les clics : {hit_txt} ; racine : {root_txt}"
                        );
                    } else {
                        log::info!("sous l'ancien overlay : {root_txt}");
                    }
                }
            });
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
pub(crate) mod native {
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

    pub fn hide_quiet(win: &WebviewWindow) {
        let _ = win.hide();
    }
}
