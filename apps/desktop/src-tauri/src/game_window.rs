//! Détection de la fenêtre du jeu via Win32 : titre et classe de la fenêtre
//! au premier plan, rien d'autre. Aucune lecture de la mémoire du jeu.

// Hors Windows, ces éléments ne servent qu'aux tests.
#![cfg_attr(not(windows), allow(dead_code))]

/// Titre de la fenêtre du client PoE 2.
pub const GAME_TITLE: &str = "Path of Exile 2";
/// Classe de fenêtre du client (partagée avec PoE 1, d'où la vérification du titre).
pub const GAME_CLASS: &str = "POEWindowClass";

/// Vrai si une fenêtre (titre, classe) est celle du jeu.
pub fn is_game_window(title: &str, class: &str) -> bool {
    title == GAME_TITLE || (class == GAME_CLASS && title.contains("Path of Exile 2"))
}

#[cfg(windows)]
mod imp {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowTextW,
    };

    pub fn game_is_foreground() -> bool {
        // SAFETY : appels Win32 en lecture seule sur la fenêtre au premier plan,
        // tampons locaux de taille fixe.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return false;
            }
            let mut title = [0u16; 256];
            let n = GetWindowTextW(hwnd, &mut title).max(0) as usize;
            let mut class = [0u16; 128];
            let c = GetClassNameW(hwnd, &mut class).max(0) as usize;
            super::is_game_window(
                &String::from_utf16_lossy(&title[..n]),
                &String::from_utf16_lossy(&class[..c]),
            )
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// Hors Windows (développement uniquement) : `AUGUR_ASSUME_GAME=0` coupe la simulation.
    pub fn game_is_foreground() -> bool {
        std::env::var("AUGUR_ASSUME_GAME")
            .map(|v| v != "0")
            .unwrap_or(true)
    }
}

pub use imp::game_is_foreground;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnait_la_fenetre_du_jeu() {
        assert!(is_game_window("Path of Exile 2", "POEWindowClass"));
        assert!(!is_game_window("Path of Exile", "POEWindowClass"));
        assert!(!is_game_window("Discord", "Chrome_WidgetWin_1"));
    }
}
