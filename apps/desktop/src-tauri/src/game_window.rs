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
    use std::path::PathBuf;

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };
    use windows::core::PWSTR;

    /// Dossier d'installation du jeu, déduit du chemin de son exécutable quand
    /// sa fenêtre est au premier plan. Droit minimal (« query limited
    /// information ») : on lit le chemin de l'exécutable, jamais sa mémoire.
    pub fn foreground_game_dir() -> Option<PathBuf> {
        if !game_is_foreground() {
            return None;
        }
        // SAFETY : handle de processus ouvert en lecture d'informations
        // seulement, refermé avant de sortir ; tampon local de taille fixe.
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(process);
            ok.ok()?;
            let exe = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
            exe.parent().map(PathBuf::from)
        }
    }

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
    pub fn foreground_game_dir() -> Option<std::path::PathBuf> {
        None
    }

    /// Hors Windows (développement uniquement) : `AUGUR_ASSUME_GAME=0` coupe la simulation.
    pub fn game_is_foreground() -> bool {
        std::env::var("AUGUR_ASSUME_GAME")
            .map(|v| v != "0")
            .unwrap_or(true)
    }
}

pub use imp::{foreground_game_dir, game_is_foreground};

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
