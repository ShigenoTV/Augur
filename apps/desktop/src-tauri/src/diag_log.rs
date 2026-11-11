//! Journal de diagnostic : les messages `log` d'Augur (infos, avertissements,
//! erreurs) gardés en mémoire pour la page Réglages et écrits dans
//! `augur.log`, dans le dossier de journaux d'Augur (`%LOCALAPPDATA%`).
//!
//! Rien ne quitte la machine : le joueur copie le journal lui-même (bouton
//! « Copier le diagnostic ») ou le joint à un ticket qu'il relit avant de
//! l'envoyer. Le dossier personnel est masqué (`~`) pour ne pas exposer le
//! nom du compte Windows.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Lignes gardées en mémoire (affichées dans Réglages).
const MAX_LINES: usize = 300;
/// Au-delà, `augur.log` devient `augur.old.log` et un nouveau fichier commence.
const MAX_FILE_BYTES: u64 = 512 * 1024;

struct Journal {
    lines: Mutex<VecDeque<String>>,
    file: Mutex<Option<(PathBuf, File)>>,
    home: Option<String>,
}

static JOURNAL: OnceLock<Journal> = OnceLock::new();

fn journal() -> &'static Journal {
    JOURNAL.get_or_init(|| Journal {
        lines: Mutex::new(VecDeque::with_capacity(MAX_LINES)),
        file: Mutex::new(None),
        home: home_dir(),
    })
}

fn home_dir() -> Option<String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .filter(|h| h.len() > 3)
}

/// Remplace le dossier personnel par `~` (insensible à la casse et aux `/`).
fn masquer(text: &str, home: Option<&str>) -> String {
    let Some(home) = home else {
        return text.to_string();
    };
    let mut out = text.to_string();
    for variante in [home.to_string(), home.replace('\\', "/")] {
        // ASCII seulement : les positions restent valables dans `out`.
        let bas = out.to_ascii_lowercase();
        let cible = variante.to_ascii_lowercase();
        let mut res = String::with_capacity(out.len());
        let mut i = 0;
        while let Some(pos) = bas[i..].find(&cible) {
            res.push_str(&out[i..i + pos]);
            res.push('~');
            i += pos + cible.len();
        }
        res.push_str(&out[i..]);
        out = res;
    }
    out
}

struct Logger;

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // Infos d'Augur ; seulement les avertissements et erreurs des bibliothèques.
        let augur = metadata.target().starts_with("augur");
        metadata.level() <= if augur { Level::Info } else { Level::Warn }
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let j = journal();
        let niveau = match record.level() {
            Level::Error => "ERREUR",
            Level::Warn => "ALERTE",
            _ => "info",
        };
        let message = masquer(&record.args().to_string(), j.home.as_deref());
        let line = format!("{} {niveau} {message}", crate::time::rfc3339_now());
        {
            let mut lines = j.lines.lock().unwrap_or_else(|e| e.into_inner());
            if lines.len() == MAX_LINES {
                lines.pop_front();
            }
            lines.push_back(line.clone());
        }
        let mut file = j.file.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((path, f)) = file.as_mut() {
            if f.metadata()
                .map(|m| m.len() >= MAX_FILE_BYTES)
                .unwrap_or(false)
            {
                let _ = std::fs::rename(&*path, path.with_extension("old.log"));
                match open(path) {
                    Ok(nouveau) => *f = nouveau,
                    Err(_) => {
                        *file = None;
                        return;
                    }
                }
            }
            let _ = writeln!(f, "{line}");
        }
    }

    fn flush(&self) {
        if let Some((_, f)) = journal()
            .file
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            let _ = f.flush();
        }
    }
}

fn open(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Installe le journal (mémoire seulement). Appelé une fois, au démarrage.
pub fn init() {
    journal();
    if log::set_logger(&Logger).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

/// Active l'écriture dans `<dir>/augur.log`.
pub fn attach_file(dir: &Path) {
    let path = dir.join("augur.log");
    let opened = std::fs::create_dir_all(dir).and_then(|_| open(&path));
    match opened {
        Ok(f) => {
            *journal().file.lock().unwrap_or_else(|e| e.into_inner()) = Some((path, f));
        }
        Err(e) => log::warn!("journal de diagnostic non écrit sur disque : {e}"),
    }
}

/// Dernières lignes du journal, la plus ancienne en premier.
pub fn lines() -> Vec<String> {
    journal()
        .lines
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dossier_personnel_masque() {
        let home = Some(r"C:\Users\Max");
        assert_eq!(
            masquer(r"suivi de C:\users\max\Documents\Client.txt", home),
            r"suivi de ~\Documents\Client.txt"
        );
        assert_eq!(
            masquer("chemin C:/Users/Max/AppData et C:\\Users\\Max", home),
            "chemin ~/AppData et ~"
        );
        assert_eq!(masquer("rien à masquer", home), "rien à masquer");
        assert_eq!(masquer("texte", None), "texte");
    }

    #[test]
    fn journal_borne_en_memoire() {
        init();
        for i in 0..(MAX_LINES + 20) {
            log::warn!(target: "augur_test", "ligne {i}");
        }
        let l = lines();
        assert!(l.len() <= MAX_LINES);
        assert!(
            l.last()
                .unwrap()
                .ends_with(&format!("ALERTE ligne {}", MAX_LINES + 19))
        );
    }
}
