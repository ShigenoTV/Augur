//! Table de prix de Forge (`forge-prix.json` dans le dossier de configuration Augur).
//!
//! Prix saisis par le joueur, en Exalted Orbs ; aucune source réseau. Le fichier est écrit
//! uniquement par le cœur Rust. Un fichier abîmé ou ancien est réparé au chargement
//! (`TablePrix::reparer`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use poe2_core::forge::prix::TablePrix;
use tauri::{AppHandle, Manager};

use crate::contracts::{AppError, ErrorCode, app_error};

#[derive(Default)]
pub struct ForgePrices {
    current: Mutex<TablePrix>,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("forge-prix.json"))
}

fn read(bytes: &[u8]) -> TablePrix {
    serde_json::from_slice::<TablePrix>(bytes)
        .map(TablePrix::reparer)
        .unwrap_or_default()
}

impl ForgePrices {
    pub fn load(app: &AppHandle) -> Self {
        let table = file(app)
            .and_then(|p| std::fs::read(p).ok())
            .map(|b| read(&b))
            .unwrap_or_default();
        Self {
            current: Mutex::new(table),
        }
    }

    pub fn get(&self) -> TablePrix {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn save(app: &AppHandle, table: &TablePrix) -> Result<(), AppError> {
        let Some(path) = file(app) else {
            return Ok(());
        };
        let write = || -> std::io::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let json = serde_json::to_vec_pretty(table).map_err(std::io::Error::other)?;
            std::fs::write(&path, json)
        };
        write().map_err(|e| app_error(ErrorCode::Internal, format!("Prix non enregistrés ({e}).")))
    }

    /// Corrige des prix ; refuse toute la saisie si une valeur est invalide.
    pub fn set(
        &self,
        app: &AppHandle,
        saisie: &BTreeMap<String, f64>,
    ) -> Result<TablePrix, AppError> {
        let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
        let mut table = guard.clone();
        let origine = format!(
            "Prix saisis par le joueur ({}).",
            crate::time::rfc3339_now()
        );
        table
            .saisir(saisie, &origine)
            .map_err(|e| app_error(ErrorCode::Unsupported, e.to_string()))?;
        Self::save(app, &table)?;
        *guard = table.clone();
        Ok(table)
    }

    /// Revient aux valeurs indicatives.
    pub fn reset(&self, app: &AppHandle) -> Result<TablePrix, AppError> {
        let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
        let table = TablePrix::indicative();
        Self::save(app, &table)?;
        *guard = table.clone();
        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relit_un_fichier_abime() {
        assert_eq!(read(b"pas du json"), TablePrix::indicative());
        let t = read(br#"{"prix":{"exalt":3},"origine":"Prix saisis"}"#);
        assert_eq!(t.prix["exalt"], 3.0);
        assert_eq!(t.prix.len(), TablePrix::indicative().prix.len());
    }
}
