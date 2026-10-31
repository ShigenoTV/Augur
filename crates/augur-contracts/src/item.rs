//! Objet lu depuis le presse-papier (Ctrl+C ou Ctrl+Alt+C).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Rarity {
    Normal,
    Magic,
    Rare,
    Unique,
    Currency,
    Gem,
    /// Toute autre rareté affichée par le jeu (fragments, objets de quête…).
    Other,
}

/// Origine d'un modificateur, telle qu'affichée par le jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ModKind {
    Implicit,
    Explicit,
    Enchant,
    Rune,
    Crafted,
    Fractured,
    Desecrated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Affix {
    Prefix,
    Suffix,
}

/// Un modificateur d'objet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemMod {
    /// Texte tel qu'affiché, pour l'UI.
    pub text: String,
    pub kind: ModKind,
    /// Préfixe ou suffixe : connu seulement en Ctrl+Alt+C.
    pub affix: Option<Affix>,
    /// Tier (1 = meilleur) : connu seulement en Ctrl+Alt+C.
    pub tier: Option<u8>,
    /// Identifiants de stats du jeu (`stat_id`) reconnus pour ce texte.
    /// Vide si le texte n'a pas été reconnu.
    #[serde(default)]
    pub stat_ids: Vec<String>,
    /// Valeurs numériques lues, dans l'ordre du texte.
    #[serde(default)]
    pub values: Vec<f64>,
}

/// Un objet parsé par `poe2-core`, converti pour l'interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParsedItem {
    /// Texte brut du presse-papier, conservé pour le signalement d'erreurs.
    pub raw_text: String,
    /// Vrai si le texte vient du format avancé (Ctrl+Alt+C).
    pub advanced: bool,
    /// Classe d'objet (« Bows », « Body Armours »…), en anglais.
    pub item_class: String,
    pub rarity: Rarity,
    /// Nom propre (rares, uniques) ; absent pour les objets normaux.
    pub name: Option<String>,
    pub base_type: String,
    pub item_level: Option<u32>,
    pub quality: Option<u32>,
    #[serde(default)]
    pub mods: Vec<ItemMod>,
    #[serde(default)]
    pub corrupted: bool,
    #[serde(default)]
    pub unidentified: bool,
    /// Version des données de jeu utilisée pour le parsing (ex. « 0.4.1 »).
    pub data_patch: String,
}
