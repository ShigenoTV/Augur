//! Jeu actif : Path of Exile 1 ou Path of Exile 2 (sélecteur « POE 1 / POE 2 »
//! de la fenêtre principale).
//!
//! Le choix est gardé dans les réglages ; ce module en garde une copie lisible
//! sans `AppHandle` (détection de la fenêtre, recherche de `Client.txt`).
//! PoE 2 reste le jeu par défaut : un fichier de réglages d'avant ce module se
//! relit à l'identique.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

/// Événement émis quand le joueur change de jeu (charge : [`Game`]).
pub const EVENT_CHANGED: &str = "augur://game-changed";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    Poe1,
    #[default]
    Poe2,
}

impl Game {
    /// Nom du dossier d'installation (Steam comme client autonome).
    pub fn folder(self) -> &'static str {
        match self {
            Game::Poe1 => "Path of Exile",
            Game::Poe2 => "Path of Exile 2",
        }
    }

    /// Titre de la fenêtre du client.
    pub fn window_title(self) -> &'static str {
        self.folder()
    }

    /// Nom affiché.
    pub fn label(self) -> &'static str {
        match self {
            Game::Poe1 => "PoE 1",
            Game::Poe2 => "PoE 2",
        }
    }

    fn code(self) -> u8 {
        match self {
            Game::Poe1 => 1,
            Game::Poe2 => 2,
        }
    }
}

static ACTIVE: AtomicU8 = AtomicU8::new(2);

/// Jeu actif.
pub fn active() -> Game {
    match ACTIVE.load(Ordering::Relaxed) {
        1 => Game::Poe1,
        _ => Game::Poe2,
    }
}

/// Change le jeu actif (appelé au démarrage et par la commande `set_game`).
pub fn set_active(game: Game) {
    ACTIVE.store(game.code(), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialise_en_minuscules_et_poe2_par_defaut() {
        assert_eq!(serde_json::to_string(&Game::Poe1).unwrap(), "\"poe1\"");
        assert_eq!(
            serde_json::from_str::<Game>("\"poe2\"").unwrap(),
            Game::Poe2
        );
        assert_eq!(Game::default(), Game::Poe2);
        assert_eq!(Game::Poe1.folder(), "Path of Exile");
        assert_eq!(Game::Poe2.folder(), "Path of Exile 2");
    }
}
