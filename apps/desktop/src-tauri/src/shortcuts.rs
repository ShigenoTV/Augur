//! Raccourcis globaux. Règle GGG : une pression de touche ne déclenche
//! jamais plus d'une action, et Augur n'envoie aucune touche au jeu.
//!
//! Les raccourcis de l'overlay (Échap, Alt+U par défaut, modifiables dans
//! Réglages) ne sont enregistrés que pendant son affichage, pour ne pas
//! confisquer ces touches au jeu. Ctrl+C et Ctrl+Alt+C passent par le
//! presse-papier du jeu et ne sont pas modifiables.

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::overlay;
use crate::settings::{SettingsStore, ShortcutSettings};

pub const EVENT_UPGRADE_SEARCH: &str = "augur://upgrade-search-requested";

/// Combinaisons du jeu ou de Windows qu'Augur ne doit pas prendre.
const RESERVED: &[&str] = &[
    "Ctrl+C",
    "Ctrl+Alt+C",
    "Ctrl+V",
    "Ctrl+X",
    "Alt+F4",
    "Alt+Tab",
];

/// Lit et vérifie un raccourci saisi dans Réglages.
pub fn parse(text: &str) -> Result<Shortcut, String> {
    let text = text.trim();
    let shortcut: Shortcut = text
        .parse()
        .map_err(|_| format!("Raccourci illisible\u{a0}: « {text} »."))?;
    let mods = shortcut.mods;
    if mods.contains(Modifiers::SUPER) {
        return Err("La touche Windows est réservée au système.".into());
    }
    let reserved = RESERVED
        .iter()
        .filter_map(|r| r.parse::<Shortcut>().ok())
        .any(|r| r.id == shortcut.id);
    if reserved {
        return Err(format!(
            "« {text} » est déjà utilisé par le jeu ou par Windows."
        ));
    }
    let free_key = matches!(
        shortcut.key,
        Code::Escape
            | Code::F1
            | Code::F2
            | Code::F3
            | Code::F4
            | Code::F5
            | Code::F6
            | Code::F7
            | Code::F8
            | Code::F9
            | Code::F10
            | Code::F11
            | Code::F12
    );
    if !free_key && !mods.intersects(Modifiers::CONTROL | Modifiers::ALT) {
        return Err(format!(
            "« {text} » seul serait pris au jeu pendant l'affichage de l'overlay\u{a0}: ajoute Ctrl ou Alt (Échap et F1 à F12 peuvent rester seuls)."
        ));
    }
    if matches!(
        shortcut.key,
        Code::ControlLeft
            | Code::ControlRight
            | Code::AltLeft
            | Code::AltRight
            | Code::ShiftLeft
            | Code::ShiftRight
    ) {
        return Err("Il manque la touche principale.".into());
    }
    Ok(shortcut)
}

/// Vérifie le couple de raccourcis de l'overlay.
pub fn validate(s: &ShortcutSettings) -> Result<(), String> {
    let close = parse(&s.close_overlay)?;
    let upgrade = parse(&s.upgrade_search)?;
    if close.id == upgrade.id {
        return Err("Les deux raccourcis doivent être différents.".into());
    }
    Ok(())
}

/// Raccourcis en vigueur (valeurs par défaut si le fichier de réglages est abîmé).
fn current(app: &AppHandle) -> (Shortcut, Shortcut) {
    let mut s = app.state::<SettingsStore>().get().shortcuts;
    if validate(&s).is_err() {
        s = ShortcutSettings::default();
    }
    let def = ShortcutSettings::default();
    (
        parse(&s.close_overlay).unwrap_or_else(|_| parse(&def.close_overlay).unwrap()),
        parse(&s.upgrade_search).unwrap_or_else(|_| parse(&def.upgrade_search).unwrap()),
    )
}

/// Plugin avec le gestionnaire unique de tous les raccourcis d'Augur.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let (close, upgrade) = current(app);
            if shortcut.id == close.id {
                overlay::hide(app);
            } else if shortcut.id == upgrade.id {
                // Une seule action : demander à l'interface de préparer la
                // recherche. La requête de trade part ensuite du client, une fois.
                let _ = app.emit_to(overlay::LABEL, EVENT_UPGRADE_SEARCH, ());
            }
        })
        .build()
}

pub fn register_overlay(app: &AppHandle) {
    let (close, upgrade) = current(app);
    for s in [close, upgrade] {
        if let Err(err) = app.global_shortcut().register(s) {
            log::warn!("raccourci {s:?} indisponible : {err}");
        }
    }
}

pub fn unregister_overlay(app: &AppHandle) {
    let (close, upgrade) = current(app);
    for s in [close, upgrade] {
        let _ = app.global_shortcut().unregister(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(close: &str, upgrade: &str) -> ShortcutSettings {
        ShortcutSettings {
            close_overlay: close.into(),
            upgrade_search: upgrade.into(),
        }
    }

    #[test]
    fn valeurs_par_defaut_valides() {
        assert!(validate(&ShortcutSettings::default()).is_ok());
        assert_eq!(parse("Escape").unwrap().key, Code::Escape);
        let u = parse("Alt+U").unwrap();
        assert_eq!((u.mods, u.key), (Modifiers::ALT, Code::KeyU));
    }

    #[test]
    fn accepte_les_combinaisons_courantes() {
        assert!(parse("Ctrl+Shift+U").is_ok());
        assert!(parse("F5").is_ok());
        assert!(parse("Alt+Digit1").is_ok());
        assert!(validate(&set("F2", "Ctrl+Alt+U")).is_ok());
    }

    #[test]
    fn refuse_les_touches_du_jeu_et_les_touches_seules() {
        assert!(parse("Ctrl+C").is_err());
        assert!(parse("Ctrl+Alt+C").is_err());
        assert!(parse("Alt+F4").is_err());
        assert!(parse("U").is_err());
        assert!(parse("Shift+U").is_err());
        assert!(parse("Super+U").is_err());
        assert!(parse("").is_err());
        assert!(parse("Alt+").is_err());
    }

    #[test]
    fn refuse_deux_raccourcis_identiques() {
        assert!(validate(&set("Alt+U", "alt+KeyU")).is_err());
    }
}
