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

use crate::game::Game;
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
    /// Gain de la session d'après les relevés de coffres (PoE 1), à partir
    /// de deux relevés faits pendant la session.
    pub gain: Option<SessionGain>,
}

/// Gain de la session : valeur du dernier relevé de coffres moins celle du
/// premier relevé fait pendant la session (même ligue), en Chaos.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGain {
    pub chaos: f64,
    /// Gain ramené à l'heure, sur le temps écoulé entre ces deux relevés
    /// (absent sous une minute).
    pub chaos_per_hour: Option<f64>,
}

/// Relevés de coffres faits pendant la session.
#[derive(Debug, Clone)]
struct CoffresSession {
    ligue: String,
    base_chaos: f64,
    base_at: Instant,
    dernier: Option<(f64, Instant)>,
}

impl CoffresSession {
    /// Ajoute un relevé : le premier (ou le premier d'une autre ligue) sert de base.
    fn pousser(slot: &mut Option<Self>, ligue: &str, chaos: f64, at: Instant) {
        match slot {
            Some(c) if c.ligue == ligue => c.dernier = Some((chaos, at)),
            _ => {
                *slot = Some(Self {
                    ligue: ligue.to_string(),
                    base_chaos: chaos,
                    base_at: at,
                    dernier: None,
                })
            }
        }
    }

    fn gain(&self) -> Option<SessionGain> {
        let (chaos, at) = self.dernier?;
        let gain = chaos - self.base_chaos;
        let secs = at.saturating_duration_since(self.base_at).as_secs_f64();
        Some(SessionGain {
            chaos: gain,
            chaos_per_hour: (secs >= 60.0).then(|| gain * 3600.0 / secs),
        })
    }
}

struct Active {
    started_at: String,
    suivi: SuiviSession,
    strategy: Strategy,
    coffres: Option<CoffresSession>,
    /// Début de la session, pour le premier relevé automatique des coffres.
    started: Instant,
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
            gain: self
                .active
                .as_ref()
                .and_then(|a| a.coffres.as_ref())
                .and_then(CoffresSession::gain),
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

/// Fichier d'historique d'un jeu (`ledger.sqlite` reste celui de PoE 2, comme avant).
pub fn store_file(game: Game) -> &'static str {
    match game {
        Game::Poe2 => "ledger.sqlite",
        Game::Poe1 => "ledger-poe1.sqlite",
    }
}

fn open_store(app: &AppHandle, game: Game) -> (Option<LedgerStore>, Option<String>) {
    match app.path().app_data_dir() {
        Ok(dir) => match LedgerStore::open(&dir.join(store_file(game))) {
            Ok(store) => (Some(store), None),
            Err(e) => {
                log::error!("historique Ledger illisible : {e}");
                (None, Some(format!("Historique illisible ({e}).")))
            }
        },
        Err(e) => (None, Some(format!("Dossier de données introuvable ({e})."))),
    }
}

impl LedgerState {
    /// Ouvre l'historique du jeu actif dans le dossier de données d'Augur.
    pub fn open(app: &AppHandle) -> Self {
        let mut inner = Inner::default();
        (inner.store, inner.storage_error) = open_store(app, crate::game::active());
        Self(Mutex::new(inner))
    }

    /// Passe à l'historique d'un autre jeu. Refusé pendant une session : elle
    /// mélangerait les maps des deux jeux.
    pub fn switch_game(&self, app: &AppHandle, game: Game) -> Result<(), String> {
        let status = {
            let mut g = self.lock();
            if g.active.is_some() {
                return Err(
                    "Une session Atlas Ledger est en cours : arrête-la avant de changer de jeu."
                        .into(),
                );
            }
            let (store, error) = open_store(app, game);
            *g = Inner {
                store,
                storage_error: error,
                ..Inner::default()
            };
            g.status()
        };
        emit(app, &status);
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn status(&self) -> LedgerStatus {
        self.lock().status()
    }

    /// Session en cours (son début, qui l'identifie) et temps écoulé depuis
    /// son dernier relevé de coffres, ou depuis son début sans relevé.
    pub fn stash_age(&self) -> Option<(String, std::time::Duration)> {
        let g = self.lock();
        let a = g.active.as_ref()?;
        let depuis = a
            .coffres
            .as_ref()
            .map(|c| c.dernier.map_or(c.base_at, |d| d.1))
            .unwrap_or(a.started);
        Some((a.started_at.clone(), depuis.elapsed()))
    }

    /// Vrai pendant une session (lu souvent par le bandeau : pas de bilan).
    pub fn is_active(&self) -> bool {
        self.lock().active.is_some()
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

/// Relevé de coffres terminé (PoE 1) : sert au gain de la session en cours.
pub fn on_stash_snapshot(app: &AppHandle, ligue: &str, chaos: f64) {
    let status = {
        let state = app.state::<LedgerState>();
        let mut g = state.lock();
        match g.active.as_mut() {
            Some(active) => {
                CoffresSession::pousser(&mut active.coffres, ligue, chaos, Instant::now());
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
                coffres: None,
                started: Instant::now(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn gain_a_partir_du_second_releve() {
        let t0 = Instant::now();
        let mut c = None;
        CoffresSession::pousser(&mut c, "Standard", 1_000.0, t0);
        assert_eq!(
            c.as_ref().and_then(CoffresSession::gain),
            None,
            "un seul relevé"
        );
        CoffresSession::pousser(&mut c, "Standard", 1_600.0, t0 + Duration::from_secs(1_800));
        let g = c.as_ref().and_then(CoffresSession::gain).unwrap();
        assert_eq!(g.chaos, 600.0);
        assert_eq!(g.chaos_per_hour, Some(1_200.0));
        // Le gain suit le dernier relevé, toujours depuis le premier.
        CoffresSession::pousser(&mut c, "Standard", 900.0, t0 + Duration::from_secs(3_600));
        let g = c.as_ref().and_then(CoffresSession::gain).unwrap();
        assert_eq!(g.chaos, -100.0);
        assert_eq!(g.chaos_per_hour, Some(-100.0));
    }

    #[test]
    fn pas_de_gain_horaire_sous_une_minute() {
        let t0 = Instant::now();
        let mut c = None;
        CoffresSession::pousser(&mut c, "Standard", 10.0, t0);
        CoffresSession::pousser(&mut c, "Standard", 25.0, t0 + Duration::from_secs(30));
        let g = c.as_ref().and_then(CoffresSession::gain).unwrap();
        assert_eq!(g.chaos, 15.0);
        assert_eq!(g.chaos_per_hour, None);
    }

    #[test]
    fn changer_de_ligue_repart_de_zero() {
        let t0 = Instant::now();
        let mut c = None;
        CoffresSession::pousser(&mut c, "Standard", 10_000.0, t0);
        CoffresSession::pousser(&mut c, "Keepers", 50.0, t0 + Duration::from_secs(600));
        assert_eq!(c.as_ref().and_then(CoffresSession::gain), None);
        CoffresSession::pousser(&mut c, "Keepers", 80.0, t0 + Duration::from_secs(1_200));
        assert_eq!(
            c.as_ref().and_then(CoffresSession::gain).unwrap().chaos,
            30.0
        );
    }
}
