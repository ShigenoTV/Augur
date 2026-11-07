//! Historique local d'Atlas Ledger : une base SQLite dans le dossier de
//! données d'Augur (`%APPDATA%\com.shigenotv.augur\ledger.sqlite`).
//!
//! Rien ne quitte le PC. Chaque session terminée est enregistrée avec son
//! bilan (calculé par `poe2_core::ledger`) et la stratégie étiquetée par le
//! joueur. Le bilan est gardé en JSON : son format suit le moteur, la base
//! ne garde en colonnes que ce qui sert au tri et aux regroupements.

use std::path::Path;

use poe2_core::ledger::Bilan;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// Version du schéma, dans `PRAGMA user_version`.
const SCHEMA: i32 = 1;

/// Longueur maximale d'une étiquette ou d'une note (saisie du joueur).
const TEXTE_MAX: usize = 500;
const ETIQUETTES_MAX: usize = 20;

/// Stratégie de farm, étiquetée par le joueur.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Strategy {
    /// Tier des Waystones utilisées (1 à 16).
    pub waystone_tier: Option<u8>,
    /// Tablettes de précurseur posées.
    pub tablets: Vec<String>,
    /// Mécaniques ciblées (Breach, Delirium, Ritual…).
    pub mechanics: Vec<String>,
    pub note: String,
}

impl Strategy {
    /// Nettoie la saisie : espaces, doublons, longueurs bornées.
    pub fn normalized(mut self) -> Self {
        fn clean(list: Vec<String>) -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for s in list {
                let s: String = s.trim().chars().take(TEXTE_MAX).collect();
                if !s.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&s)) {
                    out.push(s);
                }
            }
            out.truncate(ETIQUETTES_MAX);
            out
        }
        self.tablets = clean(self.tablets);
        self.mechanics = clean(self.mechanics);
        self.note = self.note.trim().chars().take(TEXTE_MAX).collect();
        self.waystone_tier = self.waystone_tier.filter(|t| (1..=16).contains(t));
        self
    }
}

/// Une session enregistrée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSession {
    pub id: i64,
    /// Début et fin réels (RFC 3339, UTC).
    pub started_at: String,
    pub ended_at: String,
    pub strategy: Strategy,
    pub summary: Bilan,
}

/// Résumé d'une session pour la liste de l'historique.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: i64,
    pub started_at: String,
    pub ended_at: String,
    pub strategy: Strategy,
    pub maps: u32,
    pub total_secs: i64,
    pub map_secs: i64,
    pub maps_per_hour: Option<f64>,
    pub avg_map_secs: Option<i64>,
}

pub struct LedgerStore {
    db: Connection,
}

impl LedgerStore {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> rusqlite::Result<Self> {
        let version: i32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < 1 {
            db.execute_batch(
                "CREATE TABLE IF NOT EXISTS sessions (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    started_at  TEXT    NOT NULL,
                    ended_at    TEXT    NOT NULL,
                    strategy    TEXT    NOT NULL,
                    summary     TEXT    NOT NULL,
                    maps        INTEGER NOT NULL,
                    total_secs  INTEGER NOT NULL,
                    map_secs    INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS sessions_started ON sessions(started_at);",
            )?;
            db.pragma_update(None, "user_version", SCHEMA)?;
        }
        Ok(Self { db })
    }

    /// Enregistre une session terminée et renvoie son identifiant.
    pub fn insert(
        &self,
        started_at: &str,
        ended_at: &str,
        strategy: &Strategy,
        summary: &Bilan,
    ) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO sessions (started_at, ended_at, strategy, summary, maps, total_secs, map_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                started_at,
                ended_at,
                to_json(strategy),
                to_json(summary),
                summary.maps.len() as i64,
                summary.temps.total(),
                summary.temps.map,
            ],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Change la stratégie d'une session enregistrée ; faux si absente.
    pub fn set_strategy(&self, id: i64, strategy: &Strategy) -> rusqlite::Result<bool> {
        let n = self.db.execute(
            "UPDATE sessions SET strategy = ?1 WHERE id = ?2",
            params![to_json(strategy), id],
        )?;
        Ok(n > 0)
    }

    pub fn delete(&self, id: i64) -> rusqlite::Result<bool> {
        Ok(self
            .db
            .execute("DELETE FROM sessions WHERE id = ?1", params![id])?
            > 0)
    }

    pub fn get(&self, id: i64) -> rusqlite::Result<Option<StoredSession>> {
        self.db
            .query_row(
                "SELECT id, started_at, ended_at, strategy, summary FROM sessions WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map(|row| {
                row.and_then(|(id, started_at, ended_at, strategy, summary)| {
                    Some(StoredSession {
                        id,
                        started_at,
                        ended_at,
                        strategy: serde_json::from_str(&strategy).unwrap_or_default(),
                        // Un bilan illisible (format futur) est ignoré plutôt que bloquant.
                        summary: serde_json::from_str(&summary).ok()?,
                    })
                })
            })
    }

    /// Sessions les plus récentes d'abord.
    pub fn list(&self, limit: u32) -> rusqlite::Result<Vec<SessionRow>> {
        let mut stmt = self.db.prepare(
            "SELECT id, started_at, ended_at, strategy, maps, total_secs, map_secs, summary
             FROM sessions ORDER BY started_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            let summary: Option<Bilan> = serde_json::from_str(&r.get::<_, String>(7)?).ok();
            Ok(SessionRow {
                id: r.get(0)?,
                started_at: r.get(1)?,
                ended_at: r.get(2)?,
                strategy: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                maps: r.get::<_, i64>(4)?.max(0) as u32,
                total_secs: r.get(5)?,
                map_secs: r.get(6)?,
                maps_per_hour: summary.as_ref().and_then(|s| s.maps_par_heure),
                avg_map_secs: summary.as_ref().and_then(|s| s.duree_moyenne_map),
            })
        })?;
        rows.collect()
    }
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use poe2_core::ledger::SuiviSession;

    fn bilan() -> Bilan {
        let journal = "\
2026/11/07 20:00:00 1 a [DEBUG Client 1] Generating level 79 area \"MapCrypt\" with seed 42
2026/11/07 20:00:01 1 a [INFO Client 1] [SCENE] Set Source [Crypt]
2026/11/07 20:05:00 1 a [DEBUG Client 1] Generating level 65 area \"HideoutBlankMountain\" with seed 1
2026/11/07 20:06:00 1 a [DEBUG Client 1] Generating level 80 area \"MapSump\" with seed 7
2026/11/07 20:10:00 1 a [DEBUG Client 1] Generating level 65 area \"HideoutBlankMountain\" with seed 1";
        SuiviSession::depuis_texte(journal).bilan(None)
    }

    #[test]
    fn enregistre_relit_et_liste() {
        let store = LedgerStore::in_memory().unwrap();
        let strat = Strategy {
            waystone_tier: Some(15),
            tablets: vec!["Breach".into()],
            mechanics: vec!["Breach".into()],
            note: String::new(),
        };
        let a = store
            .insert(
                "2026-11-07T19:00:00Z",
                "2026-11-07T19:10:00Z",
                &strat,
                &bilan(),
            )
            .unwrap();
        let b = store
            .insert(
                "2026-11-08T19:00:00Z",
                "2026-11-08T19:10:00Z",
                &Strategy::default(),
                &bilan(),
            )
            .unwrap();

        let s = store.get(a).unwrap().unwrap();
        assert_eq!(s.strategy, strat);
        assert_eq!(s.summary, bilan());
        assert_eq!(s.summary.maps[0].nom.as_deref(), Some("Crypt"));

        let rows = store.list(10).unwrap();
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, a]);
        assert_eq!(rows[1].maps, 2);
        assert_eq!(rows[1].total_secs, 600);
        assert_eq!(rows[1].map_secs, 540);
        assert_eq!(rows[1].maps_per_hour, Some(12.0));
        assert_eq!(rows[1].avg_map_secs, Some(270));
        assert_eq!(store.list(1).unwrap().len(), 1);
    }

    #[test]
    fn etiquette_et_supprime() {
        let store = LedgerStore::in_memory().unwrap();
        let id = store
            .insert(
                "2026-11-07T19:00:00Z",
                "2026-11-07T19:10:00Z",
                &Strategy::default(),
                &bilan(),
            )
            .unwrap();
        let strat = Strategy {
            mechanics: vec!["Ritual".into()],
            ..Strategy::default()
        };
        assert!(store.set_strategy(id, &strat).unwrap());
        assert_eq!(store.get(id).unwrap().unwrap().strategy, strat);
        assert!(!store.set_strategy(id + 1, &strat).unwrap());
        assert!(store.delete(id).unwrap());
        assert!(store.get(id).unwrap().is_none());
        assert!(!store.delete(id).unwrap());
    }

    #[test]
    fn rouvre_une_base_existante() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite");
        let id = {
            let store = LedgerStore::open(&path).unwrap();
            store
                .insert(
                    "2026-11-07T19:00:00Z",
                    "2026-11-07T19:10:00Z",
                    &Strategy::default(),
                    &bilan(),
                )
                .unwrap()
        };
        let store = LedgerStore::open(&path).unwrap();
        assert!(store.get(id).unwrap().is_some());
    }

    #[test]
    fn nettoie_la_saisie() {
        let s = Strategy {
            waystone_tier: Some(17),
            tablets: vec![" Breach ".into(), "breach".into(), "".into()],
            mechanics: (0..30).map(|i| format!("m{i}")).collect(),
            note: format!("  {}  ", "x".repeat(600)),
        }
        .normalized();
        assert_eq!(s.waystone_tier, None);
        assert_eq!(s.tablets, vec!["Breach"]);
        assert_eq!(s.mechanics.len(), ETIQUETTES_MAX);
        assert_eq!(s.note.len(), TEXTE_MAX);
    }
}
