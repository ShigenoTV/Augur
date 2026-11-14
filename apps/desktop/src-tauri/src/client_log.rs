//! Suivi de `Client.txt`, le journal texte que le jeu écrit lui-même.
//!
//! C'est une source tolérée par GGG (lecture d'un fichier de log, aucune
//! lecture mémoire). On lit uniquement les lignes ajoutées depuis le
//! lancement d'Augur, par interrogation périodique de la taille du fichier.
//! L'analyse des lignes est celle de `poe2_core::ledger::journal`, partagée
//! avec Atlas Ledger.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use poe2_core::ledger::{EvenementJournal, analyser_ligne};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::contracts::{ZoneChanged, events};
use crate::game::Game;
use crate::log_locator;
use crate::settings::SettingsStore;
use crate::time::rfc3339_now;

const POLL: Duration = Duration::from_millis(500);
/// Nouvelle recherche toutes les 5 s tant que le fichier est introuvable
/// (le joueur peut lancer le jeu ou choisir un chemin entre-temps).
const DISCOVERY_RETRY: Duration = Duration::from_secs(5);
/// Fréquence de vérification d'un changement de chemin dans les réglages.
const SETTINGS_CHECK: Duration = Duration::from_secs(2);

/// Événement émis à chaque changement de l'état du suivi.
pub const EVENT_STATUS: &str = "augur://client-log-status";

/// État du suivi, affiché dans la fenêtre principale.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientLogStatus {
    /// Fichier suivi, s'il a été trouvé.
    pub path: Option<String>,
    /// Vrai quand le fichier est trouvé et lu.
    pub watching: bool,
    /// Dernier changement de zone lu depuis le lancement d'Augur.
    pub last_zone: Option<ZoneChanged>,
    /// Emplacements essayés (diagnostic quand rien n'est trouvé).
    pub searched: Vec<String>,
}

#[derive(Default)]
pub struct ClientLogState(Mutex<ClientLogStatus>);

impl ClientLogState {
    pub fn get(&self) -> ClientLogStatus {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Applique un changement et l'émet s'il modifie l'état.
    fn update(&self, app: &AppHandle, change: impl FnOnce(&mut ClientLogStatus)) {
        let snapshot = {
            let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
            let before = guard.clone();
            change(&mut guard);
            (*guard != before).then(|| guard.clone())
        };
        if let Some(status) = snapshot {
            let _ = app.emit(EVENT_STATUS, status);
        }
    }
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

/// Jeu actif et chemin de `Client.txt` choisi pour ce jeu : un changement de
/// l'un ou de l'autre relance la recherche du fichier.
fn user_path(app: &AppHandle) -> (Game, Option<String>) {
    let game = crate::game::active();
    let path = app
        .state::<SettingsStore>()
        .get()
        .client_log_path_for(game)
        .map(str::to_string);
    (game, path)
}

/// Lance le suivi en tâche de fond : recherche du fichier, lecture des
/// ajouts, émission de `augur://zone-changed` et de l'état du suivi.
pub fn spawn(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("augur-client-log".into())
        .spawn(move || {
            let state = app.state::<ClientLogState>();
            loop {
                let configured = user_path(&app);
                let (found, searched) = log_locator::locate(configured.0, configured.1.as_deref());
                let searched: Vec<String> =
                    searched.iter().map(|p| p.display().to_string()).collect();
                let Some(path) = found else {
                    state.update(&app, |s| {
                        s.path = None;
                        s.watching = false;
                        s.searched = searched;
                    });
                    std::thread::sleep(DISCOVERY_RETRY);
                    continue;
                };
                let Ok(mut tail) = Tail::open_at_end(&path) else {
                    std::thread::sleep(DISCOVERY_RETRY);
                    continue;
                };
                log::info!("suivi de {}", tail.path().display());
                state.update(&app, |s| {
                    s.path = Some(path.display().to_string());
                    s.watching = true;
                    s.searched = searched;
                });

                let mut last_settings_check = Instant::now();
                loop {
                    std::thread::sleep(POLL);
                    if last_settings_check.elapsed() >= SETTINGS_CHECK {
                        last_settings_check = Instant::now();
                        if user_path(&app) != configured {
                            break; // le joueur a choisi un autre chemin ou un autre jeu
                        }
                    }
                    match tail.poll() {
                        Ok(lines) => {
                            for line in lines.iter().filter_map(|l| analyser_ligne(l)) {
                                crate::ledger::on_log_line(&app, &line);
                                match line.evenement {
                                    EvenementJournal::ZoneEntree { nom: zone } => {
                                        let payload = ZoneChanged {
                                            zone,
                                            at: rfc3339_now(),
                                        };
                                        crate::measures::record_zone(
                                            &app,
                                            &payload.zone,
                                            payload.at.clone(),
                                        );
                                        state.update(&app, |s| s.last_zone = Some(payload.clone()));
                                        let _ = app.emit(events::ZONE_CHANGED, payload);
                                    }
                                    EvenementJournal::ZoneGeneree {
                                        niveau, area_id, ..
                                    } => {
                                        crate::measures::note_area_generated(&app, niveau, area_id);
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            log::warn!("Client.txt illisible : {err}");
                            state.update(&app, |s| s.watching = false);
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
