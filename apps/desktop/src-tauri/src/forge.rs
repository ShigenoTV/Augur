//! Panneau Forge : bases, affixes d'une base et plan de craft (`poe2_core::forge::atelier`).
//!
//! Le calcul d'un plan prend de quelques dixièmes de seconde à quelques secondes : il tourne
//! hors du fil de l'interface, un seul à la fois.

use std::sync::atomic::{AtomicBool, Ordering};

use poe2_core::forge::atelier::{self, DemandePlan, ErreurAtelier, ResultatPlan};
use poe2_core::forge::prix::TablePrix;

use crate::contracts::{AppError, ErrorCode, app_error};

#[derive(Default)]
pub struct ForgePlanner {
    busy: AtomicBool,
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
            Ok(r) => r.map_err(error),
            Err(e) => Err(app_error(
                ErrorCode::Internal,
                format!("Calcul interrompu ({e})."),
            )),
        }
    }
}
