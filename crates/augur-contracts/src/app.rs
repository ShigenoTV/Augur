//! Contrat de l'application : commandes Tauri, événements, état, erreurs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ParsedItem;

/// Noms des commandes Tauri (`invoke`). Les arguments et retours sont
/// décrits dans `docs/architecture/contrats.md`.
pub mod commands {
    /// `(text: string) -> ParsedItem` — appelé aussi en interne sur Ctrl+C.
    pub const PARSE_CLIPBOARD_ITEM: &str = "parse_clipboard_item";
    /// `() -> OverlayState`
    pub const GET_OVERLAY_STATE: &str = "get_overlay_state";
    /// `(item: ParsedItem) -> ItemDelta`
    pub const EVALUATE_ITEM: &str = "evaluate_item";
    /// `(item: ParsedItem) -> PriceEstimate` — peut attendre le rate limit.
    pub const ESTIMATE_PRICE: &str = "estimate_price";
    /// `() -> RateLimitStatus`
    pub const GET_RATE_LIMIT_STATUS: &str = "get_rate_limit_status";
    /// `() -> FeatureFlags`
    pub const GET_FEATURE_FLAGS: &str = "get_feature_flags";
}

/// Noms des événements émis par le cœur vers l'interface.
pub mod events {
    /// Charge : `ParsedItem`.
    pub const ITEM_COPIED: &str = "augur://item-copied";
    /// Charge : `ZoneChanged`.
    pub const ZONE_CHANGED: &str = "augur://zone-changed";
    /// Charge : `RateLimitStatus`.
    pub const RATE_LIMIT_CHANGED: &str = "augur://rate-limit-changed";
    /// Charge : `FeatureFlags`.
    pub const FEATURE_FLAGS_CHANGED: &str = "augur://feature-flags-changed";
}

/// Charge de `augur://zone-changed`, lue dans `Client.txt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ZoneChanged {
    pub zone: String,
    /// Horodatage de la ligne de log, RFC 3339.
    pub at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CharacterSource {
    /// Import OAuth (`account:characters`, realm `poe2`).
    Oauth,
    /// Code Path of Building PoE 2 collé par l'utilisateur.
    PobCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CharacterSummary {
    pub name: String,
    pub class: String,
    pub level: u32,
    pub league: String,
    pub source: CharacterSource,
}

/// Résultat de `get_overlay_state`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OverlayState {
    pub contract_version: u32,
    pub app_version: String,
    /// La fenêtre du jeu est au premier plan.
    pub game_focused: bool,
    pub character: Option<CharacterSummary>,
    pub last_item: Option<ParsedItem>,
    pub data_patch: Option<String>,
}

/// Interrupteurs distants (kill switch par fonctionnalité). Une
/// fonctionnalité absente du manifeste distant est désactivée.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct FeatureFlags {
    pub clipboard_parse: bool,
    pub item_evaluation: bool,
    pub trade_price_check: bool,
    pub oauth_character_import: bool,
    pub pob_import: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ErrorCode {
    /// Le texte n'est pas un objet reconnu.
    ParseFailed,
    /// Objet ou archétype non supporté par le moteur.
    Unsupported,
    /// Aucun personnage chargé.
    NoCharacter,
    /// Restriction GGG en cours ; `retryAfterMs` renseigné.
    RateLimited,
    /// Fonctionnalité coupée par le kill switch.
    FeatureDisabled,
    Network,
    Oauth,
    Internal,
}

/// Erreur renvoyée par toute commande Tauri.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppError {
    pub code: ErrorCode,
    /// Message en français, affichable tel quel.
    pub message: String,
    pub retry_after_ms: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_en_camel_case() {
        let e = AppError {
            code: ErrorCode::RateLimited,
            message: "x".into(),
            retry_after_ms: Some(1500),
        };
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"code":"rateLimited","message":"x","retryAfterMs":1500}"#
        );
    }

    #[test]
    fn drapeau_absent_vaut_desactive() {
        let f: FeatureFlags = serde_json::from_str(r#"{"pobImport":true}"#).unwrap();
        assert!(f.pob_import);
        assert!(!f.trade_price_check);
    }
}
