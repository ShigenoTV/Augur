//! Miroir local des types de `crates/augur-contracts` (chantier Architecture,
//! pas encore fusionné). Mêmes noms, mêmes champs, même sérialisation
//! camelCase : à la fusion, ce module devient `pub use augur_contracts::*;`.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Rarity {
    Normal,
    Magic,
    Rare,
    Unique,
    Currency,
    Gem,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModKind {
    Implicit,
    Explicit,
    Enchant,
    Rune,
    Crafted,
    Fractured,
    Desecrated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Affix {
    Prefix,
    Suffix,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemMod {
    pub text: String,
    pub kind: ModKind,
    pub affix: Option<Affix>,
    pub tier: Option<u8>,
    pub stat_ids: Vec<String>,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedItem {
    pub raw_text: String,
    pub advanced: bool,
    pub item_class: String,
    pub rarity: Rarity,
    pub name: Option<String>,
    pub base_type: String,
    pub item_level: Option<u32>,
    pub quality: Option<u32>,
    pub mods: Vec<ItemMod>,
    pub corrupted: bool,
    pub unidentified: bool,
    pub data_patch: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSummary {
    pub name: String,
    pub class: String,
    pub level: u32,
    pub league: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    pub contract_version: u32,
    pub app_version: String,
    pub game_focused: bool,
    pub character: Option<CharacterSummary>,
    pub last_item: Option<ParsedItem>,
    pub data_patch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    ParseFailed,
    Unsupported,
    NoCharacter,
    RateLimited,
    FeatureDisabled,
    Network,
    Oauth,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retry_after_ms: Option<u64>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retry_after_ms: None,
        }
    }
}

/// Charge de `augur://zone-changed`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneChanged {
    pub zone: String,
    pub at: String,
}
