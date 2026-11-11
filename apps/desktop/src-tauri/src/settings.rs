//! Réglages locaux de l'application (`settings.json` dans le dossier de
//! configuration Augur). Aucune donnée sensible : les jetons OAuth iront dans
//! le Gestionnaire d'identification Windows (SECURITY.md).

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Ligue poe.ninja utilisée quand le joueur n'en a pas choisi.
pub const DEFAULT_LEAGUE: &str = "Forbidden Rites";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Canal bêta des mises à jour (option « Rejoindre la bêta »).
    pub beta_channel: bool,
    /// Chemin de `Client.txt` choisi par le joueur (prioritaire sur la détection).
    pub client_log_path: Option<String>,
    /// Ligue poe.ninja des prix de la Forge (`None` : [`DEFAULT_LEAGUE`]).
    pub league: Option<String>,
    /// Raccourcis de l'overlay, modifiables (voir `shortcuts.rs`).
    pub shortcuts: ShortcutSettings,
    /// Budget de la recherche d'upgrades, en exalts (`None` : sans limite).
    pub upgrade_budget: Option<u32>,
}

impl Settings {
    /// Ligue effective : celle du joueur, sinon la ligue par défaut.
    pub fn league(&self) -> &str {
        self.league
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .unwrap_or(DEFAULT_LEAGUE)
    }
}

/// Raccourcis de l'overlay, au format « Alt+U », « Ctrl+Shift+F2 », « Escape ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutSettings {
    pub close_overlay: String,
    pub upgrade_search: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            close_overlay: "Escape".into(),
            upgrade_search: "Alt+U".into(),
        }
    }
}

#[derive(Default)]
pub struct SettingsStore {
    current: Mutex<Settings>,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("settings.json"))
}

impl SettingsStore {
    pub fn load(app: &AppHandle) -> Self {
        let settings = file(app)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            current: Mutex::new(settings),
        }
    }

    pub fn get(&self) -> Settings {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn update(
        &self,
        app: &AppHandle,
        change: impl FnOnce(&mut Settings),
    ) -> std::io::Result<Settings> {
        let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
        change(&mut guard);
        if let Some(path) = file(app) {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let json = serde_json::to_vec_pretty(&*guard).map_err(std::io::Error::other)?;
            std::fs::write(path, json)?;
        }
        Ok(guard.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relit_un_fichier_partiel() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Settings::default());
        let s: Settings = serde_json::from_str(r#"{"betaChannel":true,"inconnu":1}"#).unwrap();
        assert!(s.beta_channel);
        assert_eq!(s.league(), DEFAULT_LEAGUE);
        assert_eq!(s.shortcuts, ShortcutSettings::default());
    }

    #[test]
    fn ligue_choisie_ou_par_defaut() {
        let mut s = Settings::default();
        assert_eq!(s.league(), "Forbidden Rites");
        s.league = Some("  ".into());
        assert_eq!(s.league(), "Forbidden Rites");
        s.league = Some(" Standard ".into());
        assert_eq!(s.league(), "Standard");
        let s: Settings =
            serde_json::from_str(r#"{"shortcuts":{"upgradeSearch":"Ctrl+F5"}}"#).unwrap();
        assert_eq!(s.shortcuts.close_overlay, "Escape");
        assert_eq!(s.shortcuts.upgrade_search, "Ctrl+F5");
    }
}
