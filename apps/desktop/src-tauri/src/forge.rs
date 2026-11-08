//! Panneau Forge : bases, affixes d'une base et plan de craft (`poe2_core::forge::atelier`).
//!
//! Le calcul d'un plan prend de quelques dixièmes de seconde à quelques secondes : il tourne
//! hors du fil de l'interface, un seul à la fois.
//!
//! Historique des plans (`forge-historique.json` dans le dossier de configuration) : chaque plan
//! calculé y entre ; le plan suivi par l'overlay est recalculé au démarrage, aux prix du moment.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use poe2_core::forge::atelier::{self, DemandePlan, ErreurAtelier, ResultatPlan};
use poe2_core::forge::historique::Historique;
use poe2_core::forge::prix::TablePrix;
use poe2_core::forge::suivi::{self, ConseilForge, ErreurSuivi, PlanActif};
use tauri::{AppHandle, Manager};

use crate::contracts::{AppError, ErrorCode, app_error};

#[derive(Default)]
pub struct ForgePlanner {
    busy: AtomicBool,
    /// Dernier plan calculé, suivi par l'overlay (Ctrl+Alt+C sur l'objet en cours de craft).
    actif: Mutex<Option<PlanActif>>,
    historique: Mutex<Historique>,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("forge-historique.json"))
}

fn read(bytes: &[u8]) -> Historique {
    serde_json::from_slice::<Historique>(bytes)
        .map(Historique::reparer)
        .unwrap_or_default()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
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
    pub fn load(app: &AppHandle) -> Self {
        let historique = file(app)
            .and_then(|p| std::fs::read(p).ok())
            .map(|b| read(&b))
            .unwrap_or_default();
        Self {
            historique: Mutex::new(historique),
            ..Default::default()
        }
    }

    fn historique_mut(&self) -> std::sync::MutexGuard<'_, Historique> {
        self.historique.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn historique(&self) -> Historique {
        self.historique_mut().clone()
    }

    /// Applique `f` à l'historique et l'enregistre (un échec d'écriture est seulement journalisé :
    /// l'historique reste juste en mémoire).
    fn changer(&self, app: &AppHandle, f: impl FnOnce(&mut Historique)) -> Historique {
        let mut h = self.historique_mut();
        f(&mut h);
        if let Some(path) = file(app) {
            let ecrire = || -> std::io::Result<()> {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let json = serde_json::to_vec_pretty(&*h).map_err(std::io::Error::other)?;
                std::fs::write(&path, json)
            };
            if let Err(e) = ecrire() {
                log::warn!("historique Forge non enregistré : {e}");
            }
        }
        h.clone()
    }

    /// Calcule un plan, l'ajoute à l'historique et le fait suivre par l'overlay.
    pub async fn plan(
        &self,
        app: &AppHandle,
        demande: DemandePlan,
        prix: TablePrix,
    ) -> Result<ResultatPlan, AppError> {
        let (resultat, actif) = self.calculer(demande.clone(), prix).await?;
        self.changer(app, |h| {
            h.ajouter(
                now_ms(),
                demande,
                actif.base_nom.clone(),
                actif.labels.clone(),
                resultat.plan.expected_cost,
            );
        });
        *self.lock() = Some(actif);
        Ok(resultat)
    }

    /// Au démarrage : recalcule le plan suivi avant la fermeture de l'appli. S'il ne se calcule
    /// plus (base retirée des données, prix manquant), il n'est plus suivi mais reste dans
    /// l'historique.
    pub async fn restaurer(&self, app: &AppHandle, prix: TablePrix) {
        let Some(demande) = self.historique_mut().demande_suivie().cloned() else {
            return;
        };
        match self.calculer(demande, prix).await {
            Ok((_, actif)) => *self.lock() = Some(actif),
            Err(e) => {
                log::warn!("plan Forge suivi non restauré : {}", e.message);
                self.changer(app, Historique::arreter_suivi);
            }
        }
    }

    async fn calculer(
        &self,
        demande: DemandePlan,
        prix: TablePrix,
    ) -> Result<(ResultatPlan, PlanActif), AppError> {
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
            Ok(Ok(r)) => Ok(r),
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

    pub fn arreter(&self, app: &AppHandle) {
        *self.lock() = None;
        self.changer(app, Historique::arreter_suivi);
    }

    /// Supprime une entrée de l'historique ; si c'était le plan suivi, l'overlay ne le suit plus.
    pub fn supprimer(&self, app: &AppHandle, id: u64) -> Historique {
        let suivi = self.historique_mut().suivi == Some(id);
        if suivi {
            *self.lock() = None;
        }
        self.changer(app, |h| {
            h.supprimer(id);
        })
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
