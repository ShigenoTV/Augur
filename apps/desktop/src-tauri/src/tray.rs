//! Icône de la zone de notification : ouvrir Augur, session de mesures,
//! mises à jour, quitter.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

const MEASURE_START: &str = "Démarrer une session de mesures";
const MEASURE_STOP: &str = "Arrêter et exporter les mesures";

/// Entrée du menu dont le libellé suit l'état de la session de mesures.
struct MeasureMenuItem(MenuItem<Wry>);

pub fn set_measure_active(app: &AppHandle, active: bool) {
    if let Some(item) = app.try_state::<MeasureMenuItem>() {
        let _ = item
            .0
            .set_text(if active { MEASURE_STOP } else { MEASURE_START });
    }
    if let Some(tray) = app.tray_by_id("augur") {
        let tip = if active {
            "Augur · mesures en cours"
        } else {
            "Augur"
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Ouvrir Augur", true, None::<&str>)?;
    let measure = MenuItem::with_id(app, "measure", MEASURE_START, true, None::<&str>)?;
    let update = MenuItem::with_id(
        app,
        "update",
        "Rechercher des mises à jour",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &measure, &update, &quit])?;
    app.manage(MeasureMenuItem(measure));

    let mut builder = TrayIconBuilder::with_id("augur")
        .tooltip("Augur")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::show_main_window(app),
            "measure" => crate::measures::toggle(app),
            "update" => {
                // La fenêtre principale affiche le résultat (bandeau ou « à jour »).
                crate::show_main_window(app);
                let _ = app.emit_to(
                    crate::MAIN_WINDOW,
                    crate::updates::EVENT_CHECK_REQUESTED,
                    (),
                );
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
