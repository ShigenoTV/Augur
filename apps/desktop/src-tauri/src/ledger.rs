//! Atlas Ledger dans l'application : session de farm en cours, alimentée
//! par les lignes de `Client.txt`, et historique local (SQLite).
//!
//! Le découpage en maps et la répartition du temps viennent de
//! `poe2_core::ledger` ; ce module ne fait que tenir l'état, horodater et
//! enregistrer. Rien ne quitte le PC.

use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use poe2_core::ledger::{Bilan, EvenementJournal, Horodatage, LigneJournal, SuiviSession};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::ledger_store::{LedgerStore, SessionRow, Strategy};
use crate::time::rfc3339_now;

/// Événement émis à chaque changement de la session (charge : `LedgerStatus`).
pub const EVENT_STATUS: &str = "augur://ledger-session";
/// Nombre maximal de sessions renvoyées par l'historique.
pub const HISTORY_MAX: u32 = 500;

/// État visible par l'interface.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerStatus {
    pub active: bool,
    /// Début réel de la session en cours (RFC 3339, UTC).
    pub started_at: Option<String>,
    /// Stratégie de la session en cours.
    pub strategy: Strategy,
    /// Bilan de la session en cours, jusqu'à maintenant.
    pub summary: Option<Bilan>,
    /// Dernière session enregistrée.
    pub last_saved_id: Option<i64>,
    /// Historique indisponible (base illisible) : message affichable.
    pub storage_error: Option<String>,
}

struct Active {
    started_at: String,
    suivi: SuiviSession,
    strategy: Strategy,
}

#[derive(Default)]
struct Inner {
    active: Option<Active>,
    /// Dernière génération de zone lue, pour démarrer une session dans la
    /// zone où se trouve déjà le joueur.
    last_generated: Option<LigneJournal>,
    /// Dernière ligne datée et instant de sa lecture : donne « maintenant »
    /// sur l'échelle du journal (heure locale du jeu) sans fuseau horaire.
    last_log: Option<(Horodatage, Instant)>,
    last_saved_id: Option<i64>,
    store: Option<LedgerStore>,
    storage_error: Option<String>,
}

impl Inner {
    fn now_log(&self) -> Option<Horodatage> {
        self.last_log
            .map(|(t, at)| t + at.elapsed().as_secs() as Horodatage)
    }

    fn status(&self) -> LedgerStatus {
        let now = self.now_log();
        LedgerStatus {
            active: self.active.is_some(),
            started_at: self.active.as_ref().map(|a| a.started_at.clone()),
            strategy: self
                .active
                .as_ref()
                .map(|a| a.strategy.clone())
                .unwrap_or_default(),
            summary: self.active.as_ref().map(|a| a.suivi.bilan(now)),
            last_saved_id: self.last_saved_id,
            storage_error: self.storage_error.clone(),
        }
    }

    fn store(&self) -> Result<&LedgerStore, String> {
        self.store.as_ref().ok_or_else(|| {
            self.storage_error
                .clone()
                .unwrap_or_else(|| "Historique indisponible.".into())
        })
    }
}

pub struct LedgerState(Mutex<Inner>);

impl LedgerState {
    /// Ouvre la base `ledger.sqlite` du dossier de données d'Augur.
    pub fn open(app: &AppHandle) -> Self {
        let mut inner = Inner::default();
        match app.path().app_data_dir() {
            Ok(dir) => match LedgerStore::open(&dir.join("ledger.sqlite")) {
                Ok(store) => inner.store = Some(store),
                Err(e) => {
                    log::error!("historique Ledger illisible : {e}");
                    inner.storage_error = Some(format!("Historique illisible ({e})."));
                }
            },
            Err(e) => inner.storage_error = Some(format!("Dossier de données introuvable ({e}).")),
        }
        Self(Mutex::new(inner))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn status(&self) -> LedgerStatus {
        self.lock().status()
    }
}

fn emit(app: &AppHandle, status: &LedgerStatus) {
    let _ = app.emit(EVENT_STATUS, status);
}

/// Ligne utile de `Client.txt` (appelée par le suivi du journal).
pub fn on_log_line(app: &AppHandle, line: &LigneJournal) {
    let status = {
        let state = app.state::<LedgerState>();
        let mut g = state.lock();
        if let Some(t) = line.a {
            g.last_log = Some((t, Instant::now()));
        }
        if matches!(line.evenement, EvenementJournal::ZoneGeneree { .. }) {
            g.last_generated = Some(line.clone());
        }
        match g.active.as_mut() {
            Some(active) => {
                active.suivi.pousser(line);
                Some(g.status())
            }
            None => None,
        }
    };
    if let Some(status) = status {
        emit(app, &status);
    }
}

pub fn start(app: &AppHandle) -> LedgerStatus {
    let status = {
        let state = app.state::<LedgerState>();
        let mut g = state.lock();
        if g.active.is_none() {
            let mut suivi = SuiviSession::new();
            // Le joueur est déjà dans une zone : elle compte dès maintenant.
            if let (Some(line), Some(now)) = (g.last_generated.clone(), g.now_log()) {
                suivi.pousser(&LigneJournal {
                    a: Some(now),
                    evenement: line.evenement,
                });
            }
            g.active = Some(Active {
                started_at: rfc3339_now(),
                suivi,
                strategy: Strategy::default(),
            });
        }
        g.status()
    };
    emit(app, &status);
    status
}

/// Arrête la session et l'enregistre (sauf si elle est vide).
pub fn stop(app: &AppHandle) -> Result<LedgerStatus, String> {
    let state = app.state::<LedgerState>();
    let (status, result) = {
        let mut g = state.lock();
        let now = g.now_log();
        let result = match g.active.take() {
            Some(active) => {
                let summary = active.suivi.bilan(now);
                if summary.temps.total() == 0 && summary.maps.is_empty() {
                    Ok(())
                } else {
                    let saved = g.store().and_then(|store| {
                        store
                            .insert(
                                &active.started_at,
                                &rfc3339_now(),
                                &active.strategy,
                                &summary,
                            )
                            .map_err(|e| format!("Session non enregistrée ({e})."))
                    });
                    if let Ok(id) = saved {
                        g.last_saved_id = Some(id);
                    }
                    saved.map(|_| ())
                }
            }
            None => Ok(()),
        };
        (g.status(), result)
    };
    emit(app, &status);
    result.map(|()| status)
}

/// Étiquette la session en cours (`id` absent) ou une session enregistrée.
pub fn set_strategy(
    app: &AppHandle,
    id: Option<i64>,
    strategy: Strategy,
) -> Result<LedgerStatus, String> {
    let strategy = strategy.normalized();
    let state = app.state::<LedgerState>();
    let status = {
        let mut g = state.lock();
        match id {
            None => match g.active.as_mut() {
                Some(active) => active.strategy = strategy,
                None => return Err("Aucune session en cours.".into()),
            },
            Some(id) => {
                let found = g
                    .store()?
                    .set_strategy(id, &strategy)
                    .map_err(|e| format!("Étiquette non enregistrée ({e})."))?;
                if !found {
                    return Err("Session introuvable.".into());
                }
            }
        }
        g.status()
    };
    emit(app, &status);
    Ok(status)
}

pub fn history(app: &AppHandle, limit: u32) -> Result<Vec<SessionRow>, String> {
    let state = app.state::<LedgerState>();
    let g = state.lock();
    g.store()?
        .list(limit.clamp(1, HISTORY_MAX))
        .map_err(|e| format!("Historique illisible ({e})."))
}

pub fn session(app: &AppHandle, id: i64) -> Result<crate::ledger_store::StoredSession, String> {
    let state = app.state::<LedgerState>();
    let g = state.lock();
    g.store()?
        .get(id)
        .map_err(|e| format!("Historique illisible ({e})."))?
        .ok_or_else(|| "Session introuvable.".into())
}

pub fn delete(app: &AppHandle, id: i64) -> Result<(), String> {
    let state = app.state::<LedgerState>();
    let g = state.lock();
    let found = g
        .store()?
        .delete(id)
        .map_err(|e| format!("Suppression impossible ({e})."))?;
    if found {
        Ok(())
    } else {
        Err("Session introuvable.".into())
    }
}
