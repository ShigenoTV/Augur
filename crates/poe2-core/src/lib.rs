//! `poe2-core` : le cœur partagé d'Augur.
//!
//! - [`parser`] lit le texte copié en jeu (Ctrl+C et Ctrl+Alt+C) et en tire
//!   un objet structuré. Le texte du presse-papier est une entrée non fiable :
//!   le parseur ne panique jamais et borne la taille de ce qu'il accepte.
//! - [`calc`] est le moteur de calcul : modificateurs typés, DPS, mitigations,
//!   avec une trace de calcul complète pour chaque valeur produite.
//!
//! Le crate ne fait aucune entrée-sortie réseau et ne lit jamais la mémoire du
//! jeu : il ne travaille que sur des données que l'utilisateur lui fournit.

#![forbid(unsafe_code)]

pub mod calc;
pub mod parser;
