//! # augur-contracts
//!
//! Les contrats entre les modules d'Augur. **Rust est la source de vérité** :
//! chaque type est exporté en TypeScript par `ts-rs` dans
//! `packages/contracts/src/generated/` (paquet `@augur/contracts`) avec
//! `cargo test -p augur-contracts`. La CI vérifie que les fichiers générés
//! sont à jour.
//!
//! Règles de conception (voir `docs/architecture/contrats.md`) :
//! - champs en `camelCase` côté JSON/TS ;
//! - pas d'entier 64 bits (illisible en JS) : durées en millisecondes `u32`,
//!   dates en chaînes RFC 3339 ;
//! - tout ajout de champ est rétrocompatible (`#[serde(default)]`), toute
//!   suppression ou tout renommage passe par une nouvelle version du contrat ;
//! - `poe2-core` ne dépend pas de ce crate : l'appli convertit ses types de
//!   domaine vers ces DTO.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Version du contrat, à incrémenter à chaque changement incompatible.
pub const CONTRACT_VERSION: u32 = 1;

pub mod app;
pub mod evaluation;
pub mod item;
pub mod price;
pub mod rate_limit;

pub use app::*;
pub use evaluation::*;
pub use item::*;
pub use price::*;
pub use rate_limit::*;

/// Niveau de prise en charge d'un archétype ou d'un calcul, affiché à
/// l'utilisateur (plan fondateur, produit 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SupportLevel {
    /// Couvert par des cas golden validés.
    Verified,
    /// Calculé, mais au moins une mécanique est « supposée ».
    Approximate,
    /// Non calculé : l'UI affiche l'état « non supporté ».
    Unsupported,
}

/// Niveau de confiance d'une mécanique (fiches du Theorycrafter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Confidence {
    /// Patch notes, déclaration d'un développeur GGG, tooltip du jeu.
    Confirmed,
    /// Test empirique reproductible.
    Measured,
    /// Hypothèse non vérifiée.
    Assumed,
}
