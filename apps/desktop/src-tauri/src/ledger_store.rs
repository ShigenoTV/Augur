//! Historique local d'Atlas Ledger : une base SQLite dans le dossier de
//! données d'Augur (`%APPDATA%\com.shigenotv.augur\ledger.sqlite`).
//!
//! Rien ne quitte le PC. Chaque session terminée est enregistrée avec son
//! bilan (calculé par `poe2_core::ledger`) et la stratégie étiquetée par le
//! joueur. Le bilan est gardé en JSON : son format suit le moteur, la base
//! ne garde en colonnes que ce qui sert au tri et aux regroupements.
//!
//! Depuis le schéma 2, chaque session garde aussi le butin saisi pour ses
//! maps et l'instantané des prix (en Exalted) relevés à son arrêt : la valeur
//! d'une session ancienne ne bouge pas avec le marché.

use std::path::Path;

use std::collections::BTreeMap;

use poe2_core::ledger::{Bilan, BilanGains, Butin, ButinsSession, bilan_gains};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// Version du schéma, dans `PRAGMA user_version`.
const SCHEMA: i32 = 2;

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
    /// Butin saisi par map (position dans `summary.maps`).
    pub loot: ButinsSession,
    /// Prix (en Exalted) relevés à l'arrêt de la session.
    pub prices: BTreeMap<String, f64>,
    pub gains: BilanGains,
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
    /// Maps dont le butin a été saisi.
    pub loot_maps: u32,
    /// Gain net (Exalted), absent sans saisie.
    pub net: Option<f64>,
    pub net_per_hour: Option<f64>,
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
            db.pragma_update(None, "user_version", 1)?;
        }
        if version < 2 {
            db.execute_batch(
                "ALTER TABLE sessions ADD COLUMN loot   TEXT NOT NULL DEFAULT '{}';
                 ALTER TABLE sessions ADD COLUMN prices TEXT NOT NULL DEFAULT '{}';",
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
        loot: &ButinsSession,
        prices: &BTreeMap<String, f64>,
    ) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO sessions (started_at, ended_at, strategy, summary, maps, total_secs, map_secs, loot, prices)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                started_at,
                ended_at,
                to_json(strategy),
                to_json(summary),
                summary.maps.len() as i64,
                summary.temps.total(),
                summary.temps.map,
                to_json(loot),
                to_json(prices),
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

    /// Saisit le butin d'une map d'une session enregistrée (un butin vide l'efface). Une session
    /// enregistrée sans prix (schéma 1) prend les prix donnés. Rien si la session ou la map est
    /// absente.
    pub fn set_loot(
        &self,
        id: i64,
        map: usize,
        loot: Butin,
        current_prices: &BTreeMap<String, f64>,
    ) -> rusqlite::Result<Option<StoredSession>> {
        let Some(mut s) = self.get(id)? else {
            return Ok(None);
        };
        if map >= s.summary.maps.len() {
            return Ok(None);
        }
        if loot.est_vide() {
            s.loot.remove(&map);
        } else {
            s.loot.insert(map, loot);
        }
        if s.prices.is_empty() {
            s.prices = current_prices.clone();
        }
        self.db.execute(
            "UPDATE sessions SET loot = ?1, prices = ?2 WHERE id = ?3",
            params![to_json(&s.loot), to_json(&s.prices), id],
        )?;
        self.get(id)
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
                "SELECT id, started_at, ended_at, strategy, summary, loot, prices FROM sessions WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()
            .map(|row| {
                row.and_then(
                    |(id, started_at, ended_at, strategy, summary, loot, prices)| {
                        // Un bilan illisible (format futur) est ignoré plutôt que bloquant.
                        let summary: Bilan = serde_json::from_str(&summary).ok()?;
                        let loot = read_loot(&loot);
                        let prices = read_prices(&prices);
                        let gains = bilan_gains(&summary, &loot, &prices);
                        Some(StoredSession {
                            id,
                            started_at,
                            ended_at,
                            strategy: serde_json::from_str(&strategy).unwrap_or_default(),
                            summary,
                            loot,
                            prices,
                            gains,
                        })
                    },
                )
            })
    }

    /// Sessions les plus récentes d'abord.
    pub fn list(&self, limit: u32) -> rusqlite::Result<Vec<SessionRow>> {
        let mut stmt = self.db.prepare(
            "SELECT id, started_at, ended_at, strategy, maps, total_secs, map_secs, summary, loot, prices
             FROM sessions ORDER BY started_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            let summary: Option<Bilan> = serde_json::from_str(&r.get::<_, String>(7)?).ok();
            let gains = summary.as_ref().map(|s| {
                bilan_gains(
                    s,
                    &read_loot(&r.get::<_, String>(8).unwrap_or_default()),
                    &read_prices(&r.get::<_, String>(9).unwrap_or_default()),
                )
            });
            let saisies = gains.as_ref().map_or(0, |g| g.maps_saisies);
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
                loot_maps: saisies,
                net: gains.as_ref().filter(|_| saisies > 0).map(|g| g.net),
                net_per_hour: gains.as_ref().and_then(|g| g.net_par_heure),
            })
        })?;
        rows.collect()
    }
}

/// Butin enregistré : entrées illisibles ou invalides écartées.
fn read_loot(json: &str) -> ButinsSession {
    serde_json::from_str::<ButinsSession>(json)
        .unwrap_or_default()
        .into_iter()
        .map(|(i, b)| (i, b.normalise()))
        .filter(|(_, b)| !b.est_vide())
        .collect()
}

fn read_prices(json: &str) -> BTreeMap<String, f64> {
    serde_json::from_str::<BTreeMap<String, f64>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, p)| p.is_finite() && *p >= 0.0)
        .collect()
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
                &ButinsSession::new(),
                &BTreeMap::new(),
            )
            .unwrap();
        let b = store
            .insert(
                "2026-11-08T19:00:00Z",
                "2026-11-08T19:10:00Z",
                &Strategy::default(),
                &bilan(),
                &ButinsSession::new(),
                &BTreeMap::new(),
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
                &ButinsSession::new(),
                &BTreeMap::new(),
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
                    &ButinsSession::new(),
                    &BTreeMap::new(),
                )
                .unwrap()
        };
        let store = LedgerStore::open(&path).unwrap();
        assert!(store.get(id).unwrap().is_some());
    }

    fn prices() -> BTreeMap<String, f64> {
        [("exalt".to_string(), 1.0), ("divine".to_string(), 600.0)]
            .into_iter()
            .collect()
    }

    fn loot(q: &[(&str, u32)], depense: f64) -> Butin {
        Butin {
            quantites: q.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
            autre: 0.0,
            depense,
        }
    }

    #[test]
    fn garde_le_butin_et_les_prix_de_la_session() {
        let store = LedgerStore::in_memory().unwrap();
        let butins: ButinsSession = [(0, loot(&[("exalt", 20)], 5.0))].into_iter().collect();
        let id = store
            .insert(
                "2026-11-09T19:00:00Z",
                "2026-11-09T19:10:00Z",
                &Strategy::default(),
                &bilan(),
                &butins,
                &prices(),
            )
            .unwrap();
        let s = store.get(id).unwrap().unwrap();
        assert_eq!(s.loot, butins);
        assert_eq!(s.gains.net, 15.0);

        // Saisie après coup, aux prix gardés (pas à ceux du moment).
        let moment: BTreeMap<String, f64> = [("divine".to_string(), 900.0)].into_iter().collect();
        let s = store
            .set_loot(id, 1, loot(&[("divine", 1)], 0.0), &moment)
            .unwrap()
            .unwrap();
        assert_eq!(s.gains.net, 615.0);
        assert_eq!(s.gains.maps_saisies, 2);
        let row = &store.list(1).unwrap()[0];
        assert_eq!((row.loot_maps, row.net), (2, Some(615.0)));
        // 615 ex en 10 minutes.
        assert_eq!(row.net_per_hour, Some(3_690.0));

        // Un butin vide efface la saisie ; une map absente est refusée.
        let s = store
            .set_loot(id, 1, Butin::default(), &moment)
            .unwrap()
            .unwrap();
        assert_eq!(s.gains.maps_saisies, 1);
        assert!(
            store
                .set_loot(id, 2, loot(&[("exalt", 1)], 0.0), &moment)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .set_loot(id + 1, 0, loot(&[("exalt", 1)], 0.0), &moment)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn migre_une_base_du_schema_1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(
                "CREATE TABLE sessions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT, started_at TEXT NOT NULL,
                    ended_at TEXT NOT NULL, strategy TEXT NOT NULL, summary TEXT NOT NULL,
                    maps INTEGER NOT NULL, total_secs INTEGER NOT NULL, map_secs INTEGER NOT NULL);
                 PRAGMA user_version = 1;",
            )
            .unwrap();
            db.execute(
                "INSERT INTO sessions (started_at, ended_at, strategy, summary, maps, total_secs, map_secs)
                 VALUES ('2026-11-07T19:00:00Z', '2026-11-07T19:10:00Z', '{}', ?1, 2, 600, 540)",
                params![to_json(&bilan())],
            )
            .unwrap();
        }
        let store = LedgerStore::open(&path).unwrap();
        let row = &store.list(1).unwrap()[0];
        assert_eq!((row.loot_maps, row.net), (0, None));
        // Une ancienne session prend les prix du moment à la première saisie.
        let s = store
            .set_loot(row.id, 0, loot(&[("divine", 1)], 0.0), &prices())
            .unwrap()
            .unwrap();
        assert_eq!(s.prices, prices());
        assert_eq!(s.gains.net, 600.0);
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
