//! Solveur de Forge (reverse-crafting), repris de poe2-craft-assistant (crate `craft-solver`).
//!
//! MDP « Stochastic Shortest Path » sur un état abstrait de l'objet vis-à-vis de l'objectif :
//! abstraction ([`state`]) → transitions analytiques ([`model`]) → policy iteration puis value
//! iteration ([`solve`], [`linsolve`]) → graphe de plan, coût espéré et quantiles ([`plan`]) →
//! vérification Monte-Carlo du plan sur le moteur exact ([`verify`]).

// Code repris tel quel : boucles indexées et types composés gardés comme dans l'original.
#![allow(clippy::needless_range_loop, clippy::type_complexity)]

pub mod linsolve;
pub mod model;
pub mod plan;
pub mod solve;
pub mod state;
pub mod verify;

pub use model::*;
pub use plan::*;
pub use solve::*;
pub use state::*;
pub use verify::*;

#[cfg(test)]
mod tests;
