//! Forge : le planificateur de craft (produit 2 du plan fondateur).
//!
//! Ce module est le moteur exact, repris de poe2-craft-assistant (crate
//! `craft-core`, même auteur) : modèle d'objet, tirage pondéré, monnaies et
//! Omens, objectif, simulation de Monte-Carlo. Il ne lit aucune donnée : le
//! pool d'affixes d'une base (poids compris) lui est fourni déjà résolu.
//! Cadrage : `docs/forge/cadrage.md`.
//!
//! - [`moteur`] : objet en cours de craft, pool d'affixes d'une base, effet
//!   exact de chaque monnaie (`AffixPool::apply`).
//! - [`donnees`] : pool d'une base à partir des tables RePoE et des poids
//!   poe2db (mods sans poids listés à part, jamais inventés).
//! - [`objectif`] : « ces groupes d'affixes, à tel tier ou mieux ».
//! - [`monte_carlo`] : simulation parallèle, déterministe pour une graine.
//!
//! Tout est indexé (`u16`) pour le chemin chaud ; aucune entrée-sortie.

pub mod catalogue;
pub mod donnees;
pub mod monte_carlo;
pub mod moteur;
pub mod ninja;
pub mod objectif;
pub mod prix;
pub mod solveur;

pub use monte_carlo::*;
pub use moteur::*;
pub use objectif::*;
