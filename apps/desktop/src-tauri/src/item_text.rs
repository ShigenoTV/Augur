//! Reconnaissance minimale du texte d'objet copié depuis le jeu.
//!
//! TEMPORAIRE : le vrai parseur vit dans `poe2-core` (chantier moteur). Ce
//! module ne sert qu'à faire tourner la coquille de l'overlay de bout en bout :
//! il lit l'en-tête (classe, rareté, nom, base), le niveau d'objet, la qualité,
//! l'état corrompu et les lignes de mods, sans reconnaître les `stat_id`.

use crate::contracts::{
    Affix, AppError, ErrorCode, ItemMod, ModKind, ParsedItem, Rarity, app_error,
};

const SEPARATOR: &str = "--------";

/// En-têtes acceptés. Le français est à vérifier sur un client réel.
const CLASS_PREFIXES: [&str; 2] = ["Item Class:", "Classe d'objet:"];
const RARITY_PREFIXES: [&str; 2] = ["Rarity:", "Rareté:"];

/// Vrai si le texte ressemble à un objet copié depuis PoE 2 (test bon marché,
/// appelé à chaque changement du presse-papier).
pub fn looks_like_item(text: &str) -> bool {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next().unwrap_or_default();
    let second = lines.next().unwrap_or_default();
    CLASS_PREFIXES.iter().any(|p| first.starts_with(p))
        && RARITY_PREFIXES.iter().any(|p| second.starts_with(p))
        && text.contains(SEPARATOR)
}

fn value_after<'a>(line: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes
        .iter()
        .find_map(|p| line.strip_prefix(p))
        .map(str::trim)
}

fn parse_rarity(value: &str) -> Rarity {
    match value {
        "Normal" => Rarity::Normal,
        "Magic" | "Magique" => Rarity::Magic,
        "Rare" => Rarity::Rare,
        "Unique" => Rarity::Unique,
        "Currency" | "Objet monétaire" => Rarity::Currency,
        "Gem" | "Gemme" => Rarity::Gem,
        _ => Rarity::Other,
    }
}

/// Une ligne « Clé: valeur » de propriété (Armour: 412, Item Level: 81…).
fn is_property_line(line: &str) -> bool {
    match line.split_once(": ") {
        Some((key, _)) => {
            !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_alphabetic() || c == ' ' || c == '\'')
        }
        None => false,
    }
}

const KIND_SUFFIXES: [(&str, ModKind); 6] = [
    ("(implicit)", ModKind::Implicit),
    ("(enchant)", ModKind::Enchant),
    ("(rune)", ModKind::Rune),
    ("(crafted)", ModKind::Crafted),
    ("(fractured)", ModKind::Fractured),
    ("(desecrated)", ModKind::Desecrated),
];

fn split_kind(line: &str) -> (String, ModKind) {
    for (suffix, kind) in KIND_SUFFIXES {
        if let Some(text) = line.strip_suffix(suffix) {
            return (text.trim().to_owned(), kind);
        }
    }
    (line.to_owned(), ModKind::Explicit)
}

/// En-tête du format avancé (Ctrl+Alt+C) :
/// `{ Prefix Modifier "Tempered" (Tier: 2) — Damage, Physical }`.
fn parse_advanced_header(line: &str) -> (Option<Affix>, Option<u8>) {
    let affix = if line.contains("Prefix") {
        Some(Affix::Prefix)
    } else if line.contains("Suffix") {
        Some(Affix::Suffix)
    } else {
        None
    };
    let tier = line
        .split_once("Tier: ")
        .and_then(|(_, rest)| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse().ok());
    (affix, tier)
}

fn first_number(s: &str) -> Option<u32> {
    s.split(|c: char| !c.is_ascii_digit())
        .find(|p| !p.is_empty())
        .and_then(|p| p.parse().ok())
}

pub fn parse(text: &str) -> Result<ParsedItem, AppError> {
    if !looks_like_item(text) {
        return Err(app_error(
            ErrorCode::ParseFailed,
            "Le texte copié ne ressemble pas à un objet de Path of Exile 2.",
        ));
    }

    let normalized = text.replace("\r\n", "\n");
    let sections: Vec<Vec<&str>> = normalized
        .split(SEPARATOR)
        .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).collect())
        .filter(|s: &Vec<&str>| !s.is_empty())
        .collect();

    let header = sections.first().cloned().unwrap_or_default();
    let item_class = value_after(header.first().copied().unwrap_or_default(), &CLASS_PREFIXES)
        .unwrap_or_default()
        .to_owned();
    let rarity = parse_rarity(
        value_after(header.get(1).copied().unwrap_or_default(), &RARITY_PREFIXES)
            .unwrap_or_default(),
    );
    let names: Vec<&str> = header.iter().skip(2).copied().collect();
    let (name, base_type) = match names.as_slice() {
        [name, base, ..] => (Some((*name).to_owned()), (*base).to_owned()),
        [base] => (None, (*base).to_owned()),
        [] => (None, String::new()),
    };

    let mut item = ParsedItem {
        raw_text: text.to_owned(),
        advanced: text.contains("{ "),
        item_class,
        rarity,
        name,
        base_type,
        item_level: None,
        quality: None,
        mods: Vec::new(),
        corrupted: false,
        unidentified: false,
        data_patch: "stub".to_owned(),
    };

    let mut mod_sections = 0usize;
    let last_index = sections.len().saturating_sub(1);
    for (index, section) in sections.iter().enumerate().skip(1) {
        match section.as_slice() {
            ["Corrupted"] | ["Corrompu"] => {
                item.corrupted = true;
                continue;
            }
            ["Unidentified"] | ["Non identifié"] => {
                item.unidentified = true;
                continue;
            }
            _ => {}
        }

        let content: Vec<&str> = section
            .iter()
            .copied()
            .filter(|l| !l.starts_with('{'))
            .collect();
        if content.iter().all(|l| is_property_line(l)) {
            for line in content {
                if let Some(v) = value_after(line, &["Item Level:", "Niveau d'objet:"]) {
                    item.item_level = first_number(v);
                } else if let Some(v) = value_after(line, &["Quality:", "Qualité:"]) {
                    item.quality = first_number(v);
                }
            }
            continue;
        }

        // Texte d'ambiance des uniques : dernière section, sans chiffre, après les mods.
        let is_flavour = item.rarity == Rarity::Unique
            && mod_sections > 0
            && index == last_index
            && !section
                .iter()
                .any(|l| l.chars().any(|c| c.is_ascii_digit()));
        if is_flavour {
            continue;
        }

        mod_sections += 1;
        let mut pending: (Option<Affix>, Option<u8>) = (None, None);
        for line in section {
            if line.starts_with('{') {
                pending = parse_advanced_header(line);
                continue;
            }
            let (text, kind) = split_kind(line);
            item.mods.push(ItemMod {
                text,
                kind,
                affix: pending.0.take(),
                tier: pending.1.take(),
                stat_ids: Vec::new(),
                values: Vec::new(),
            });
        }
    }

    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOW: &str = "Item Class: Bows\r\nRarity: Rare\r\nStorm Thirst\r\nAdvanced Dualstring Bow\r\n--------\r\nPhysical Damage: 62-118 (augmented)\r\nAttacks per Second: 1.15\r\n--------\r\nRequires: Level 78, 163 Dex\r\n--------\r\nItem Level: 81\r\n--------\r\nBow Attacks fire an additional Arrow (implicit)\r\n--------\r\n168% increased Physical Damage\r\nAdds 4 to 131 Lightning Damage\r\n--------\r\nCorrupted\r\n";

    const ADVANCED: &str = "Item Class: Helmets\nRarity: Rare\nDread Crown\nExpert Visored Helm\n--------\nQuality: +20% (augmented)\nArmour: 412\n--------\nItem Level: 79\n--------\n{ Prefix Modifier \"Vigorous\" (Tier: 2) — Life }\n+92 to maximum Life\n{ Suffix Modifier \"of the Furnace\" (Tier: 3) — Elemental, Fire, Resistance }\n+38% to Fire Resistance\n";

    const UNIQUE: &str = "Item Class: Gloves\nRarity: Unique\nNorthpaw\nRingmail Gauntlets\n--------\nItem Level: 72\n--------\n+40 to Dexterity\n--------\nThe great bear sleeps beneath the ice\n";

    #[test]
    fn reconnait_un_objet() {
        assert!(looks_like_item(BOW));
        assert!(!looks_like_item("bonjour"));
        assert!(!looks_like_item("Item Class: Bows\nsans rareté"));
    }

    #[test]
    fn lit_en_tete_niveau_et_mods() {
        let item = parse(BOW).unwrap();
        assert_eq!(item.item_class, "Bows");
        assert_eq!(item.rarity, Rarity::Rare);
        assert_eq!(item.name.as_deref(), Some("Storm Thirst"));
        assert_eq!(item.base_type, "Advanced Dualstring Bow");
        assert_eq!(item.item_level, Some(81));
        assert!(item.corrupted);
        assert!(!item.advanced);
        let kinds: Vec<_> = item
            .mods
            .iter()
            .map(|m| (m.text.as_str(), m.kind))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("Bow Attacks fire an additional Arrow", ModKind::Implicit),
                ("168% increased Physical Damage", ModKind::Explicit),
                ("Adds 4 to 131 Lightning Damage", ModKind::Explicit),
            ]
        );
    }

    #[test]
    fn lit_le_format_avance() {
        let item = parse(ADVANCED).unwrap();
        assert!(item.advanced);
        assert_eq!(item.quality, Some(20));
        assert_eq!(item.mods.len(), 2);
        assert_eq!(item.mods[0].affix, Some(Affix::Prefix));
        assert_eq!(item.mods[0].tier, Some(2));
        assert_eq!(item.mods[1].affix, Some(Affix::Suffix));
        assert_eq!(item.mods[1].tier, Some(3));
    }

    #[test]
    fn ignore_le_texte_d_ambiance_des_uniques() {
        let item = parse(UNIQUE).unwrap();
        assert_eq!(item.mods.len(), 1);
        assert_eq!(item.mods[0].text, "+40 to Dexterity");
    }

    #[test]
    fn refuse_un_texte_quelconque() {
        let err = parse("Hello world").unwrap_err();
        assert_eq!(err.code, ErrorCode::ParseFailed);
    }
}
