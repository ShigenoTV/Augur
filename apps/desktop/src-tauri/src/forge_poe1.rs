//! Forge PoE 1 (`poe2_core::poe1::forge`) : bases, affixes, méthodes de spam et prix poe.ninja.
//!
//! Les prix viennent des pages « échange » de poe.ninja PoE 1 (Currency et Essence), gardées 30
//! minutes par le cache de l'overlay PoE 1. Le coût de chaque méthode est fait par le panneau : la
//! chance d'un essai ne dépend pas des prix.

use std::collections::BTreeMap;

use poe2_core::poe1::forge::{
    self, AffixesPoe1, BaseVue, DemandePoe1, ErreurForgePoe1, ResultatPoe1,
};
use poe2_core::poe1::ninja;
use serde::Serialize;
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

pub async fn planifier(demande: DemandePoe1) -> Result<ResultatPoe1, AppError> {
    tauri::async_runtime::spawn_blocking(move || forge::planifier(&demande))
        .await
        .map_err(|e| app_error(ErrorCode::Internal, e.to_string()))?
        .map_err(error)
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
    for type_page in ["Currency", "Essence"] {
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
