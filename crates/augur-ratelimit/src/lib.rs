//! # augur-ratelimit
//!
//! Le budget manager des rate limits de GGG, composant d'architecture
//! d'Augur (voir `docs/architecture/adr/0002-budget-manager-rate-limits.md`).
//!
//! - [`headers`] lit `X-Rate-Limit-Policy`, `X-Rate-Limit-Rules`,
//!   `X-Rate-Limit-{règle}`, `X-Rate-Limit-{règle}-State` et `Retry-After`.
//! - [`limiter`] est une machine à états pure (temps injecté), testable en
//!   temps simulé.
//! - [`BudgetManager`] (feature `async`) partage les limiteurs entre tâches
//!   tokio, avec priorité aux actions explicites du joueur.

pub mod headers;
pub mod limiter;
#[cfg(feature = "async")]
mod manager;

pub use headers::{
    HeaderError, PolicySnapshot, Rule, WindowLimit, WindowState, parse_policy,
    parse_policy_from_pairs,
};
pub use limiter::{Decision, LimiterConfig, PolicyLimiter, PolicyView, Priority, WindowView};
#[cfg(feature = "async")]
pub use manager::{BudgetManager, Permit};
