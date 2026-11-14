//! Réglages locaux de l'application (`settings.json` dans le dossier de
//! configuration Augur). Aucune donnée sensible : les jetons OAuth iront dans
//! le Gestionnaire d'identification Windows (SECURITY.md).

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::game::Game;

/// Ligue poe.ninja utilisée quand le joueur n'en a pas choisi.
pub const DEFAULT_LEAGUE: &str = "Forbidden Rites";
/// Ligue PoE 1 par défaut : Standard ne disparaît pas à la fin d'une ligue temporaire.
pub const DEFAULT_LEAGUE_POE1: &str = "Standard";

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
    /// Jeu actif (sélecteur « POE 1 / POE 2 »). Les champs ci-dessus sans
    /// préfixe sont ceux de PoE 2, pour relire les anciens fichiers à l'identique.
    pub game: Game,
    /// Réglages propres à PoE 1.
    pub poe1: GameSettings,
}

/// Réglages qui dépendent du jeu (pour PoE 1 ; ceux de PoE 2 sont à la racine).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameSettings {
    pub client_log_path: Option<String>,
    pub league: Option<String>,
    /// Nom de compte pathofexile.com (« Nom#1234 »), pour lire les coffres.
    /// La session (`POESESSID`) n'est jamais ici : voir `secret.rs`.
    pub account: Option<String>,
    /// Onglets de coffre suivis (index du jeu).
    pub stash_tabs: Vec<u32>,
}

fn non_vide(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|l| !l.is_empty())
}

impl Settings {
    /// Ligue effective : celle du joueur, sinon la ligue par défaut.
    pub fn league(&self) -> &str {
        non_vide(&self.league).unwrap_or(DEFAULT_LEAGUE)
    }

    /// Ligue effective d'un jeu.
    pub fn league_for(&self, game: Game) -> &str {
        match game {
            Game::Poe2 => self.league(),
            Game::Poe1 => non_vide(&self.poe1.league).unwrap_or(DEFAULT_LEAGUE_POE1),
        }
    }

    /// Chemin de `Client.txt` choisi par le joueur pour un jeu.
    pub fn client_log_path_for(&self, game: Game) -> Option<&str> {
        match game {
            Game::Poe2 => self.client_log_path.as_deref(),
            Game::Poe1 => self.poe1.client_log_path.as_deref(),
        }
    }

    /// Change le chemin de `Client.txt` d'un jeu.
    pub fn set_client_log_path_for(&mut self, game: Game, path: Option<String>) {
        match game {
            Game::Poe2 => self.client_log_path = path,
            Game::Poe1 => self.poe1.client_log_path = path,
        }
    }

    /// Change la ligue d'un jeu.
    pub fn set_league_for(&mut self, game: Game, league: Option<String>) {
        match game {
            Game::Poe2 => self.league = league,
            Game::Poe1 => self.poe1.league = league,
        }
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

    #[test]
    fn reglages_par_jeu() {
        // Ancien fichier : PoE 2, mêmes valeurs qu'avant.
        let mut s: Settings =
            serde_json::from_str(r#"{"clientLogPath":"D:/jeu","league":"Standard"}"#).unwrap();
        assert_eq!(s.game, Game::Poe2);
        assert_eq!(s.client_log_path_for(Game::Poe2), Some("D:/jeu"));
        assert_eq!(s.client_log_path_for(Game::Poe1), None);
        assert_eq!(s.league_for(Game::Poe1), DEFAULT_LEAGUE_POE1);
        s.set_league_for(Game::Poe1, Some("Covens".into()));
        s.set_client_log_path_for(Game::Poe1, Some("C:/poe1".into()));
        assert_eq!(s.league_for(Game::Poe1), "Covens");
        assert_eq!(s.league_for(Game::Poe2), "Standard");
        assert_eq!(s.client_log_path.as_deref(), Some("D:/jeu"));
        let relu: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(relu, s);
    }
}
