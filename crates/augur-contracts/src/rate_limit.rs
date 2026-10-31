//! État du budget de rate limit, affiché à l'utilisateur (file visible).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Une fenêtre d'une règle (`ip`, `account`, `client`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RuleWindowState {
    pub rule: String,
    pub hits: u32,
    pub max_hits: u32,
    /// Requêtes réellement utilisables (après marge de sécurité).
    pub usable_hits: u32,
    pub period_s: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PolicyStatus {
    /// Nom de la politique GGG (ex. « trade-search-request-limit »).
    pub policy: String,
    #[serde(default)]
    pub windows: Vec<RuleWindowState>,
    /// Restriction en cours, en millisecondes.
    pub blocked_for_ms: Option<u32>,
    /// Requêtes en attente dans la file.
    pub queued: u32,
}

/// Résultat de `get_rate_limit_status` et charge de `augur://rate-limit-changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RateLimitStatus {
    #[serde(default)]
    pub policies: Vec<PolicyStatus>,
}
