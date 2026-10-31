//! Suivi de `Client.txt`, le journal texte que le jeu écrit lui-même.
//!
//! C'est une source tolérée par GGG (lecture d'un fichier de log, aucune
//! lecture mémoire). On lit uniquement les lignes ajoutées depuis le
//! lancement d'Augur, par interrogation périodique de la taille du fichier.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::contracts::ZoneChanged;
use crate::time::rfc3339_now;

pub const EVENT_ZONE_CHANGED: &str = "augur://zone-changed";

const POLL: Duration = Duration::from_millis(500);
const DISCOVERY_RETRY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    /// Entrée dans une zone, nom affiché (indépendant de la langue du client).
    ZoneEntered(String),
    /// Génération d'une zone : niveau de zone et identifiant interne.
    AreaGenerated { level: u32, area_id: String },
}

/// Analyse une ligne du journal. Formats observés sur PoE 1 et PoE 2, à
/// confirmer sur un client PoE 2 à jour (tâche S1 front) :
/// `2026/10/31 14:12:34 123 abc [INFO Client 42] [SCENE] Set Source [Clearfell]`
/// `2026/10/31 14:12:33 123 abc [DEBUG Client 42] Generating level 12 area "G1_2" with seed 1`
pub fn parse_line(line: &str) -> Option<LogEvent> {
    if let Some((_, rest)) = line.split_once("[SCENE] Set Source [") {
        let zone = rest.strip_suffix(']').unwrap_or(rest).trim_end_matches(']');
        if zone.is_empty() || zone == "(null)" || zone == "(unknown)" {
            return None;
        }
        return Some(LogEvent::ZoneEntered(zone.to_owned()));
    }
    if let Some((_, rest)) = line.split_once("Generating level ") {
        let (level, rest) = rest.split_once(" area \"")?;
        let (area_id, _) = rest.split_once('"')?;
        return Some(LogEvent::AreaGenerated {
            level: level.trim().parse().ok()?,
            area_id: area_id.to_owned(),
        });
    }
    None
}

/// Lecture incrémentale d'un fichier qui grossit (et peut être tronqué).
pub struct Tail {
    path: PathBuf,
    offset: u64,
    partial: String,
}

impl Tail {
    /// Ouvre en fin de fichier : l'historique antérieur est ignoré.
    pub fn open_at_end(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let offset = std::fs::metadata(&path)?.len();
        Ok(Self {
            path,
            offset,
            partial: String::new(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Renvoie les lignes complètes ajoutées depuis le dernier appel.
    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        let len = std::fs::metadata(&self.path)?.len();
        if len < self.offset {
            // Fichier tronqué ou recréé : on repart du début.
            self.offset = 0;
            self.partial.clear();
        }
        if len == self.offset {
            return Ok(Vec::new());
        }
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = Vec::with_capacity((len - self.offset) as usize);
        file.by_ref()
            .take(len - self.offset)
            .read_to_end(&mut buf)?;
        self.offset += buf.len() as u64;

        self.partial.push_str(&String::from_utf8_lossy(&buf));
        let mut lines: Vec<String> = Vec::new();
        while let Some(pos) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=pos).collect();
            let line = line.trim_end_matches(['\r', '\n']);
            if !line.is_empty() {
                lines.push(line.to_owned());
            }
        }
        Ok(lines)
    }
}

/// Emplacements connus de `Client.txt`. `AUGUR_CLIENT_LOG` force un chemin
/// (bibliothèque Steam sur un autre disque) en attendant le réglage dans l'UI.
pub fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(custom) = std::env::var("AUGUR_CLIENT_LOG") {
        paths.push(PathBuf::from(custom));
    }
    if cfg!(windows) {
        for root in [
            r"C:\Program Files (x86)\Steam\steamapps\common\Path of Exile 2",
            r"C:\Program Files\Steam\steamapps\common\Path of Exile 2",
            r"C:\Program Files (x86)\Grinding Gear Games\Path of Exile 2",
            r"C:\Program Files\Grinding Gear Games\Path of Exile 2",
        ] {
            paths.push(Path::new(root).join("logs").join("Client.txt"));
        }
    }
    paths
}

fn find_log() -> Option<PathBuf> {
    candidate_paths().into_iter().find(|p| p.is_file())
}

/// Lance le suivi en tâche de fond et émet `augur://zone-changed`.
pub fn spawn(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("augur-client-log".into())
        .spawn(move || {
            loop {
                let Some(path) = find_log() else {
                    std::thread::sleep(DISCOVERY_RETRY);
                    continue;
                };
                let Ok(mut tail) = Tail::open_at_end(&path) else {
                    std::thread::sleep(DISCOVERY_RETRY);
                    continue;
                };
                log::info!("suivi de {}", tail.path().display());
                loop {
                    std::thread::sleep(POLL);
                    match tail.poll() {
                        Ok(lines) => {
                            for event in lines.iter().filter_map(|l| parse_line(l)) {
                                if let LogEvent::ZoneEntered(zone) = event {
                                    let payload = ZoneChanged {
                                        zone,
                                        at: rfc3339_now(),
                                    };
                                    let _ = app.emit(EVENT_ZONE_CHANGED, payload);
                                }
                            }
                        }
                        Err(err) => {
                            log::warn!("Client.txt illisible : {err}");
                            break;
                        }
                    }
                }
            }
        });
    if let Err(err) = spawned {
        log::error!("impossible de lancer le suivi de Client.txt : {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn lit_les_changements_de_zone() {
        let line = "2026/10/31 14:12:34 1234567 cffb0719 [INFO Client 4242] [SCENE] Set Source [Clearfell]";
        assert_eq!(
            parse_line(line),
            Some(LogEvent::ZoneEntered("Clearfell".into()))
        );
        let null = "2026/10/31 14:12:34 1 a [INFO Client 1] [SCENE] Set Source [(null)]";
        assert_eq!(parse_line(null), None);
    }

    #[test]
    fn lit_la_generation_de_zone() {
        let line = "2026/10/31 14:12:33 1 a [DEBUG Client 1] Generating level 65 area \"MapCrypt\" with seed 3735928559";
        assert_eq!(
            parse_line(line),
            Some(LogEvent::AreaGenerated {
                level: 65,
                area_id: "MapCrypt".into()
            })
        );
    }

    #[test]
    fn ignore_le_reste() {
        assert_eq!(
            parse_line("2026/10/31 14:12:33 1 a [INFO Client 1] @From Bob: hi"),
            None
        );
        assert_eq!(parse_line(""), None);
    }

    #[test]
    fn suit_les_ajouts_et_les_troncatures() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Client.txt");
        std::fs::write(&path, "ancienne ligne\n").unwrap();

        let mut tail = Tail::open_at_end(&path).unwrap();
        assert!(tail.poll().unwrap().is_empty());

        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        write!(f, "ligne 1\r\nligne 2\nligne inco").unwrap();
        f.flush().unwrap();
        assert_eq!(tail.poll().unwrap(), vec!["ligne 1", "ligne 2"]);

        writeln!(f, "mplète").unwrap();
        f.flush().unwrap();
        assert_eq!(tail.poll().unwrap(), vec!["ligne incomplète"]);

        std::fs::write(&path, "après rotation\n").unwrap();
        assert_eq!(tail.poll().unwrap(), vec!["après rotation"]);
    }
}
