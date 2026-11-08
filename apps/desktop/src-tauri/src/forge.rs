//! Panneau Forge : bases, affixes d'une base et plan de craft (`poe2_core::forge::atelier`).
//!
//! Le calcul d'un plan prend de quelques dixièmes de seconde à quelques secondes : il tourne
//! hors du fil de l'interface, un seul à la fois.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use poe2_core::forge::atelier::{self, DemandePlan, ErreurAtelier, ResultatPlan};
use poe2_core::forge::prix::TablePrix;
use poe2_core::forge::suivi::{self, ConseilForge, ErreurSuivi, PlanActif};

use crate::contracts::{AppError, ErrorCode, app_error};

#[derive(Default)]
pub struct ForgePlanner {
    busy: AtomicBool,
    /// Dernier plan calculé, suivi par l'overlay (Ctrl+Alt+C sur l'objet en cours de craft).
    actif: Mutex<Option<PlanActif>>,
}

/// Plan suivi par l'overlay, pour le panneau.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suivi {
    pub base_nom: String,
    pub ilvl: u8,
    pub labels: Vec<String>,
}

pub fn error(e: ErreurAtelier) -> AppError {
    let code = match e {
        ErreurAtelier::Donnees(_) | ErreurAtelier::Solveur(_) => ErrorCode::Internal,
        _ => ErrorCode::Unsupported,
    };
    app_error(code, e.to_string())
}

impl ForgePlanner {
    pub async fn plan(
        &self,
        demande: DemandePlan,
        prix: TablePrix,
    ) -> Result<ResultatPlan, AppError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(app_error(
                ErrorCode::RateLimited,
                "Un calcul de plan est déjà en cours.",
            ));
        }
        let res = tauri::async_runtime::spawn_blocking(move || {
            atelier::planifier(&demande, &prix, &AtomicBool::new(false))
        })
        .await;
        self.busy.store(false, Ordering::SeqCst);
        match res {
            Ok(Ok((resultat, actif))) => {
                *self.lock() = Some(actif);
                Ok(resultat)
            }
            Ok(Err(e)) => Err(error(e)),
            Err(e) => Err(app_error(
                ErrorCode::Internal,
                format!("Calcul interrompu ({e})."),
            )),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<PlanActif>> {
        self.actif.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn suivi(&self) -> Option<Suivi> {
        self.lock().as_ref().map(|a| Suivi {
            base_nom: a.base_nom.clone(),
            ilvl: a.ilvl,
            labels: a.labels.clone(),
        })
    }

    pub fn arreter(&self) {
        *self.lock() = None;
    }

    /// Conseil pour le texte d'objet copié en jeu. `Ok(None)` : pas de plan suivi, ou objet
    /// étranger au plan (l'overlay montre alors Upgrade Copilot).
    pub fn conseil(&self, texte: &str) -> Result<Option<ConseilForge>, AppError> {
        let guard = self.lock();
        let Some(actif) = guard.as_ref() else {
            return Ok(None);
        };
        let Ok(objet) = poe2_core::parser::parser(texte) else {
            return Ok(None);
        };
        match suivi::conseiller(actif, &objet) {
            Ok(c) => Ok(Some(c)),
            Err(ErreurSuivi::AutreBase(_) | ErreurSuivi::Rarete) => Ok(None),
            Err(e) => Err(app_error(ErrorCode::Unsupported, format!("Forge : {e}."))),
        }
    }
}
