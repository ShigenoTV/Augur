//! Forge PoE 1 (`poe2_core::poe1::forge`) : bases, affixes, méthodes de spam et prix poe.ninja.
//!
//! Comme la Forge PoE 2 : chaque plan calculé avec des prix entre dans l'historique
//! (`forge-poe1-historique.json`) et devient le plan suivi par l'overlay (Ctrl+Alt+C sur l'objet en
//! cours de craft donne l'action conseillée).
//!
//! Les prix viennent des pages « échange » de poe.ninja PoE 1 (Currency, Essence, Fossil,
//! Resonator), gardées 30 minutes par le cache de l'overlay PoE 1. Le coût de chaque méthode est fait
//! par le panneau ; les prix envoyés servent au plan conseillé et au choix des fossiles.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use poe2_core::poe1::forge::suivi::{self, ConseilPoe1};
use poe2_core::poe1::forge::{
    self, AffixesPoe1, BaseVue, DemandePoe1, ErreurForgePoe1, ResultatPoe1,
};
use poe2_core::poe1::ninja;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::contracts::{AppError, ErrorCode, app_error};
use crate::game::Game;

fn error(e: ErreurForgePoe1) -> AppError {
    let code = match e {
        ErreurForgePoe1::Donnees(_) => ErrorCode::Internal,
        _ => ErrorCode::Unsupported,
    };
    app_error(code, e.to_string())
}

pub fn bases() -> Result<Vec<BaseVue>, AppError> {
    forge::bases().map_err(error)
}

pub fn affixes(base_id: &str, ilvl: u32, influence: Option<&str>) -> Result<AffixesPoe1, AppError> {
    forge::affixes(base_id, ilvl, influence).map_err(error)
}

pub fn analyser(texte: &str) -> Result<forge::ObjetDepartPoe1, AppError> {
    forge::analyser_objet(texte).map_err(error)
}

pub async fn planifier(app: &AppHandle, demande: DemandePoe1) -> Result<ResultatPoe1, AppError> {
    let d = demande.clone();
    let r = tauri::async_runtime::spawn_blocking(move || forge::planifier(&d))
        .await
        .map_err(|e| app_error(ErrorCode::Internal, e.to_string()))?
        .map_err(error)?;
    if let Some(plan) = &r.plan {
        app.state::<ForgePoe1State>()
            .ajouter(app, demande, plan.cout_moyen);
    }
    Ok(r)
}

/// Un plan calculé (avec des prix).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntreePoe1 {
    pub id: u64,
    /// Date du calcul (ms depuis 1970).
    pub at: u64,
    pub base_nom: String,
    pub ilvl: u32,
    pub influence: Option<String>,
    pub labels: Vec<String>,
    /// Coût moyen du plan en Chaos, aux prix du calcul.
    pub cout_moyen: f64,
    /// Demande, sans l'objet de départ : le suivi repart de l'objet copié en jeu.
    pub demande: DemandePoe1,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoriquePoe1 {
    pub entrees: Vec<EntreePoe1>,
    /// Entrée suivie par l'overlay.
    pub suivi: Option<u64>,
}

/// Plans gardés : les plus récents.
const MAX_ENTREES: usize = 30;

impl HistoriquePoe1 {
    fn ajouter(&mut self, e: EntreePoe1) {
        // Même demande recalculée : l'entrée remonte en tête au lieu d'être dupliquée.
        self.entrees.retain(|x| {
            !(x.demande.base_id == e.demande.base_id
                && x.demande.ilvl == e.demande.ilvl
                && x.demande.influence == e.demande.influence
                && x.demande.voulus == e.demande.voulus)
        });
        self.suivi = Some(e.id);
        self.entrees.insert(0, e);
        self.entrees.truncate(MAX_ENTREES);
    }

    fn supprimer(&mut self, id: u64) {
        self.entrees.retain(|e| e.id != id);
        if self.suivi == Some(id) {
            self.suivi = None;
        }
    }

    fn suivie(&self) -> Option<&EntreePoe1> {
        let id = self.suivi?;
        self.entrees.iter().find(|e| e.id == id)
    }
}

#[derive(Default)]
pub struct ForgePoe1State {
    historique: Mutex<HistoriquePoe1>,
}

fn fichier(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("forge-poe1-historique.json"))
}

fn maintenant_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

impl ForgePoe1State {
    pub fn load(app: &AppHandle) -> Self {
        let h = fichier(app)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<HistoriquePoe1>(&b).ok())
            .unwrap_or_default();
        Self {
            historique: Mutex::new(h),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HistoriquePoe1> {
        self.historique.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn historique(&self) -> HistoriquePoe1 {
        self.lock().clone()
    }

    /// Applique `f` et enregistre (un échec d'écriture est seulement journalisé).
    fn changer(&self, app: &AppHandle, f: impl FnOnce(&mut HistoriquePoe1)) -> HistoriquePoe1 {
        let mut h = self.lock();
        f(&mut h);
        if let Some(path) = fichier(app) {
            let ecrire = || -> std::io::Result<()> {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let json = serde_json::to_vec_pretty(&*h).map_err(std::io::Error::other)?;
                std::fs::write(&path, json)
            };
            if let Err(e) = ecrire() {
                log::warn!("historique Forge PoE 1 non enregistré : {e}");
            }
        }
        h.clone()
    }

    fn ajouter(&self, app: &AppHandle, mut demande: DemandePoe1, cout_moyen: f64) {
        demande.objet_depart = None;
        let base_nom = forge::bases()
            .ok()
            .and_then(|b| b.into_iter().find(|b| b.id == demande.base_id))
            .map(|b| b.nom)
            .unwrap_or_else(|| demande.base_id.clone());
        let labels = suivi::libelles(&demande).unwrap_or_default();
        let at = maintenant_ms();
        let entree = EntreePoe1 {
            id: at,
            at,
            base_nom,
            ilvl: demande.ilvl,
            influence: demande.influence.clone(),
            labels,
            cout_moyen,
            demande,
        };
        self.changer(app, |h| h.ajouter(entree));
    }

    pub fn supprimer(&self, app: &AppHandle, id: u64) -> HistoriquePoe1 {
        self.changer(app, |h| h.supprimer(id))
    }

    pub fn suivre(&self, app: &AppHandle, id: Option<u64>) -> HistoriquePoe1 {
        self.changer(app, |h| {
            h.suivi = id.filter(|i| h.entrees.iter().any(|e| e.id == *i));
        })
    }

    /// Conseil pour le texte copié en jeu. `Ok(None)` : pas de plan suivi, ou autre base.
    pub fn conseil(&self, texte: &str) -> Result<Option<ConseilPoe1>, AppError> {
        let Some(demande) = self.lock().suivie().map(|e| e.demande.clone()) else {
            return Ok(None);
        };
        suivi::conseiller(&demande, texte)
            .map_err(|e| app_error(ErrorCode::Unsupported, format!("Forge : {e}.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entree(id: u64, ilvl: u32) -> EntreePoe1 {
        EntreePoe1 {
            id,
            at: id,
            base_nom: "Two-Stone Ring".into(),
            ilvl,
            influence: None,
            labels: vec![],
            cout_moyen: 10.0,
            demande: DemandePoe1 {
                base_id: "b".into(),
                ilvl,
                influence: None,
                voulus: vec![],
                prix: Default::default(),
                objet_depart: None,
            },
        }
    }

    #[test]
    fn historique_sans_doublon_et_suivi() {
        let mut h = HistoriquePoe1::default();
        h.ajouter(entree(1, 84));
        h.ajouter(entree(2, 75));
        h.ajouter(entree(3, 84));
        let ids: Vec<u64> = h.entrees.iter().map(|e| e.id).collect();
        assert_eq!(ids, [3, 2]);
        assert_eq!(h.suivi, Some(3));
        assert_eq!(h.suivie().unwrap().ilvl, 84);
        h.supprimer(3);
        assert_eq!(h.suivi, None);
        assert!(h.suivie().is_none());
    }
}

/// Prix en Chaos des monnaies et Essences, pour la ligue PoE 1 des réglages.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrixPoe1 {
    pub league: String,
    pub prix: BTreeMap<String, f64>,
    /// Page poe.ninja injoignable : les prix manquants restent à saisir.
    pub erreur: Option<String>,
}

pub async fn prix(app: &AppHandle) -> Result<PrixPoe1, AppError> {
    let league = app
        .state::<crate::settings::SettingsStore>()
        .get()
        .league_for(Game::Poe1)
        .to_string();
    let client = crate::forge_prices::client(&app.package_info().version.to_string())?;
    let cache = app.state::<crate::poe1_items::Poe1Ninja>();
    let mut prix = BTreeMap::new();
    let mut erreur = None;
    for type_page in ["Currency", "Essence", "Fossil", "Resonator"] {
        match cache
            .page(&client, &league, ninja::Page::Echange(type_page))
            .await
        {
            Ok(doc) => prix.extend(ninja::prix_echange(&doc)),
            Err(e) => erreur = Some(e.message),
        }
    }
    prix.insert(forge::calcul::CHAOS.to_string(), 1.0);
    Ok(PrixPoe1 {
        league,
        prix,
        erreur,
    })
}
