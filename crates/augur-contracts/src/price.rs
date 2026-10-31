//! Estimation de prix affichée à côté du delta.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PriceSource {
    /// Recherche de trade lancée depuis le poste du joueur.
    Trade,
    /// Agrégats horaires de l'échange de devises (backend).
    CurrencyExchange,
    /// Modèle de pricing de l'équipe Data (backend).
    Model,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PriceEstimate {
    /// Devise de référence, identifiant du jeu (ex. « exalted »).
    pub currency: String,
    pub low: f64,
    pub median: f64,
    pub high: f64,
    pub sample_size: u32,
    pub source: PriceSource,
    pub league: String,
    /// Date de l'observation, RFC 3339.
    pub observed_at: String,
    /// Vrai si la donnée est plus vieille que la fraîcheur attendue :
    /// l'UI affiche l'horodatage au lieu de masquer le prix.
    pub stale: bool,
}
