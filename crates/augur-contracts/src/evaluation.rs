//! Delta de DPS / EHP d'un objet pour le personnage chargé.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Confidence, SupportLevel};

/// Une valeur avant / après l'équipement de l'objet.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatDelta {
    pub before: f64,
    pub after: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Element {
    Fire,
    Cold,
    Lightning,
    Chaos,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResistanceDelta {
    pub element: Element,
    pub before: f64,
    pub after: f64,
    /// Plafond de résistance du personnage (75 par défaut).
    pub cap: f64,
}

/// Une mécanique qui a pesé dans le calcul, avec sa confiance, pour
/// expliquer un résultat « approximatif ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MechanicNote {
    /// Identifiant de la fiche mécanique (ex. « electrocute-buildup »).
    pub mechanic: String,
    pub confidence: Confidence,
}

/// Résultat de `evaluate_item`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemDelta {
    /// Emplacement comparé, en identifiant stable (ex. « weapon1 »,
    /// « bodyArmour ») ; la traduction en libellé est faite par l'UI.
    pub slot: String,
    /// Nom de l'objet actuellement équipé dans cet emplacement, s'il y en a un
    /// (« remplace Gale Song »).
    pub equipped_name: Option<String>,
    /// Set d'armes concerné (PoE 2 : 1 ou 2), absent hors armes.
    #[ts(type = "1 | 2 | null")]
    pub weapon_set: Option<u8>,
    pub support: SupportLevel,
    pub dps: StatDelta,
    pub ehp: StatDelta,
    #[serde(default)]
    pub resistances: Vec<ResistanceDelta>,
    /// Spirit restant, si l'objet le modifie.
    pub spirit: Option<StatDelta>,
    #[serde(default)]
    pub mechanics: Vec<MechanicNote>,
    /// Temps de calcul, pour suivre l'objectif de 100 ms.
    pub compute_ms: u32,
}
