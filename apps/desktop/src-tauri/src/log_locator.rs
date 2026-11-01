//! Recherche de `Client.txt`, le journal écrit par le jeu dans
//! `<dossier du jeu>\logs\Client.txt`.
//!
//! Ordre de recherche :
//! 1. chemin choisi par le joueur dans Augur (réglage) ;
//! 2. variable d'environnement `AUGUR_CLIENT_LOG` ;
//! 3. dossier de l'exécutable du jeu quand il est au premier plan ;
//! 4. bibliothèques Steam déclarées dans `libraryfolders.vdf` (tous disques) ;
//! 5. emplacements par défaut de Steam et du client autonome, sur chaque disque.

use std::path::{Path, PathBuf};

pub const GAME_FOLDER: &str = "Path of Exile 2";

/// `<dossier>\logs\Client.txt`.
pub fn log_in(game_dir: &Path) -> PathBuf {
    game_dir.join("logs").join("Client.txt")
}

/// Accepte le chemin du fichier, du dossier `logs` ou du dossier du jeu.
pub fn normalize_user_path(input: &str) -> PathBuf {
    let trimmed = input.trim().trim_matches('"');
    let path = PathBuf::from(trimmed);
    let is_file_name = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("Client.txt"));
    if is_file_name {
        return path;
    }
    let is_logs = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("logs"));
    if is_logs {
        path.join("Client.txt")
    } else {
        log_in(&path)
    }
}

/// Extrait les dossiers de bibliothèque d'un `libraryfolders.vdf` de Steam
/// (format clé-valeur de Valve, chemins aux barres obliques inverses doublées).
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|line| {
            let mut parts = line.split('"').filter(|p| !p.trim().is_empty());
            let key = parts.next()?;
            let value = parts.next()?;
            key.eq_ignore_ascii_case("path")
                .then(|| PathBuf::from(value.replace("\\\\", "\\")))
        })
        .collect()
}

fn steam_game_dir(library: &Path) -> PathBuf {
    library.join("steamapps").join("common").join(GAME_FOLDER)
}

/// Racines Steam probables : registre Windows, puis emplacements par défaut.
fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = registry::steam_path() {
        roots.push(root);
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
    roots.push(PathBuf::from(r"C:\Program Files\Steam"));
    roots
}

/// Emplacements par défaut sur chaque disque (D:\SteamLibrary, etc.).
fn default_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for drive in 'C'..='H' {
        let root = format!(r"{drive}:\");
        let root = Path::new(&root);
        dirs.push(steam_game_dir(&root.join("SteamLibrary")));
        dirs.push(steam_game_dir(&root.join("Steam")));
        dirs.push(steam_game_dir(
            &root.join("Program Files (x86)").join("Steam"),
        ));
        dirs.push(
            root.join("Program Files (x86)")
                .join("Grinding Gear Games")
                .join(GAME_FOLDER),
        );
        dirs.push(
            root.join("Program Files")
                .join("Grinding Gear Games")
                .join(GAME_FOLDER),
        );
        dirs.push(root.join("Grinding Gear Games").join(GAME_FOLDER));
        dirs.push(root.join("Games").join(GAME_FOLDER));
        dirs.push(root.join(GAME_FOLDER));
    }
    dirs
}

/// Tous les chemins candidats, dans l'ordre de priorité, sans doublon.
pub fn candidates(user_path: Option<&str>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !out
            .iter()
            .any(|q| q.as_os_str().eq_ignore_ascii_case(p.as_os_str()))
        {
            out.push(p);
        }
    };
    if let Some(p) = user_path.filter(|p| !p.trim().is_empty()) {
        push(normalize_user_path(p));
    }
    if let Ok(p) = std::env::var("AUGUR_CLIENT_LOG") {
        push(PathBuf::from(p));
    }
    if cfg!(windows) {
        if let Some(dir) = crate::game_window::foreground_game_dir() {
            push(log_in(&dir));
        }
        for root in steam_roots() {
            push(log_in(&steam_game_dir(&root)));
            let vdf = root.join("steamapps").join("libraryfolders.vdf");
            if let Ok(text) = std::fs::read_to_string(vdf) {
                for library in parse_library_folders(&text) {
                    push(log_in(&steam_game_dir(&library)));
                }
            }
        }
        for dir in default_dirs() {
            push(log_in(&dir));
        }
    }
    out
}

/// Premier candidat qui existe, et la liste complète pour le diagnostic.
pub fn locate(user_path: Option<&str>) -> (Option<PathBuf>, Vec<PathBuf>) {
    let all = candidates(user_path);
    let found = all.iter().find(|p| p.is_file()).cloned();
    (found, all)
}

#[cfg(windows)]
mod registry {
    use std::path::PathBuf;

    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::w;

    /// `HKCU\Software\Valve\Steam\SteamPath`, écrit par Steam à l'installation.
    pub fn steam_path() -> Option<PathBuf> {
        let mut buf = [0u16; 520];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY : lecture d'une valeur texte dans un tampon local dont la
        // taille en octets est passée à l'API.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!(r"Software\Valve\Steam"),
                w!("SteamPath"),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if status.is_err() {
            return None;
        }
        let len = (size as usize / 2).saturating_sub(1);
        let path = String::from_utf16_lossy(&buf[..len]).replace('/', "\\");
        (!path.is_empty()).then(|| PathBuf::from(path))
    }
}

#[cfg(not(windows))]
mod registry {
    pub fn steam_path() -> Option<std::path::PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VDF: &str = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"228980"		"0"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"apps"
		{
			"2694490"		"52612741120"
		}
	}
}"#;

    #[test]
    fn lit_les_bibliotheques_steam() {
        assert_eq!(
            parse_library_folders(VDF),
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
            ]
        );
    }

    #[test]
    fn accepte_fichier_dossier_logs_ou_dossier_du_jeu() {
        let expected = Path::new("jeu").join("logs").join("Client.txt");
        assert_eq!(normalize_user_path("jeu"), expected);
        assert_eq!(
            normalize_user_path(&format!("\"{}\"", Path::new("jeu").join("logs").display())),
            expected
        );
        assert_eq!(
            normalize_user_path(&expected.display().to_string()),
            expected
        );
    }

    #[test]
    fn cherche_steam_sur_les_autres_disques() {
        // Installation réelle signalée : D:\Steam\steamapps\common\Path of Exile 2
        let expected = Path::new(r"D:\")
            .join("Steam")
            .join("steamapps")
            .join("common")
            .join(GAME_FOLDER);
        assert!(default_dirs().contains(&expected));
    }

    #[test]
    fn le_chemin_du_joueur_passe_en_premier() {
        let dir = tempfile::tempdir().unwrap();
        let log = log_in(dir.path());
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, "").unwrap();
        let (found, all) = locate(Some(&dir.path().display().to_string()));
        assert_eq!(found, Some(log.clone()));
        assert_eq!(all.first(), Some(&log));
    }
}
