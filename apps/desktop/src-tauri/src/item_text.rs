//! Lecture du texte d'objet copié en jeu, branchée sur le parseur du moteur
//! (`poe2_core::parser`). Ce module ne fait que deux choses :
//!
//! - un test bon marché ([`looks_like_item`]) appelé à chaque changement du
//!   presse-papier, pour ne parser que ce qui ressemble à un objet ;
//! - la conversion de `poe2_core::parser::Objet` vers le contrat
//!   `ParsedItem` envoyé à l'interface.

use poe2_core::parser::{self, Affixe, ErreurParse, GenreMod, ModObjet, Objet, Rarete};

use crate::contracts::{
    Affix, AppError, ErrorCode, ItemMod, ModKind, ParsedItem, Rarity, app_error,
};

const SEPARATOR: &str = "--------";

/// En-têtes reconnus pour le test rapide. Le français est accepté ici pour
/// que le joueur reçoive un message clair (client non pris en charge) plutôt
/// qu'un silence.
const RARITY_PREFIXES: [&str; 2] = ["Rarity:", "Rareté:"];

/// Version des données de jeu utilisée pour reconnaître les mods. Vide tant
/// que les `stat_id` ne sont pas extraits des données du jeu.
const DATA_PATCH: &str = "";

/// Vrai si le texte ressemble à un objet copié depuis PoE 2.
pub fn looks_like_item(text: &str) -> bool {
    text.len() <= parser::TAILLE_MAX
        && text.contains(SEPARATOR)
        && text
            .lines()
            .take(4)
            .any(|l| RARITY_PREFIXES.iter().any(|p| l.trim().starts_with(p)))
}

fn rarity(r: Rarete) -> Rarity {
    match r {
        Rarete::Normal => Rarity::Normal,
        Rarete::Magique => Rarity::Magic,
        Rarete::Rare => Rarity::Rare,
        Rarete::Unique => Rarity::Unique,
        Rarete::Devise => Rarity::Currency,
        Rarete::Gemme => Rarity::Gem,
    }
}

fn kind(g: GenreMod) -> ModKind {
    match g {
        GenreMod::Implicite => ModKind::Implicit,
        GenreMod::Explicite => ModKind::Explicit,
        GenreMod::Rune => ModKind::Rune,
        GenreMod::Enchantement => ModKind::Enchant,
        GenreMod::Artisanal => ModKind::Crafted,
        GenreMod::Desacralise => ModKind::Desecrated,
        GenreMod::Fracture => ModKind::Fractured,
    }
}

fn item_mod(m: ModObjet) -> ItemMod {
    let entete = m.entete.as_ref();
    ItemMod {
        affix: entete.and_then(|e| match e.affixe {
            Affixe::Prefixe => Some(Affix::Prefix),
            Affixe::Suffixe => Some(Affix::Suffix),
            _ => None,
        }),
        tier: entete
            .and_then(|e| e.palier)
            .and_then(|t| u8::try_from(t).ok()),
        kind: kind(m.genre),
        text: m.texte,
        stat_ids: Vec::new(),
        values: m.valeurs,
    }
}

fn parsed_item(raw_text: &str, o: Objet) -> ParsedItem {
    ParsedItem {
        raw_text: raw_text.to_owned(),
        advanced: o.avance,
        item_class: o.classe.unwrap_or_default(),
        rarity: rarity(o.rarete),
        name: o.nom,
        base_type: o.base,
        item_level: o.niveau_objet,
        quality: o.qualite.and_then(|q| u32::try_from(q).ok()),
        mods: o.mods.into_iter().map(item_mod).collect(),
        corrupted: o.corrompu,
        unidentified: o.non_identifie,
        data_patch: DATA_PATCH.to_owned(),
    }
}

fn error_message(err: &ErreurParse) -> String {
    match err {
        ErreurParse::LangueNonPriseEnCharge => {
            "Seul le client anglais du jeu est pris en charge pour l'instant.".to_owned()
        }
        ErreurParse::Vide | ErreurParse::TropLong(_) | ErreurParse::PasUnObjet => {
            "Le texte copié ne ressemble pas à un objet de Path of Exile 2.".to_owned()
        }
        autre => {
            let texte = autre.to_string();
            let mut lettres = texte.chars();
            match lettres.next() {
                Some(c) => format!("Objet non reconnu : {}{}.", c, lettres.as_str()),
                None => "Objet non reconnu.".to_owned(),
            }
        }
    }
}

pub fn parse(text: &str) -> Result<ParsedItem, AppError> {
    parser::parser(text)
        .map(|o| parsed_item(text, o))
        .map_err(|err| app_error(ErrorCode::ParseFailed, error_message(&err)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(nom: &str) -> String {
        let chemin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/corpus/objets")
            .join(nom);
        std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{nom} : {e}"))
    }

    const BOW: &str = "Item Class: Bows\r\nRarity: Rare\r\nStorm Thirst\r\nAdvanced Dualstring Bow\r\n--------\r\nPhysical Damage: 62-118 (augmented)\r\nAttacks per Second: 1.15\r\n--------\r\nRequires: Level 78, 163 Dex\r\n--------\r\nItem Level: 81\r\n--------\r\nBow Attacks fire an additional Arrow (implicit)\r\n--------\r\n168% increased Physical Damage\r\nAdds 4 to 131 Lightning Damage\r\n--------\r\nCorrupted\r\n";

    const FRANCAIS: &str = "Classe d'objet: Baguettes\nRareté: Rare\nChant funeste\nBaguette flétrie\n--------\nNiveau d'objet: 70\n";

    const UNIQUE: &str = "Item Class: Gloves\nRarity: Unique\nNorthpaw\nRingmail Gauntlets\n--------\nItem Level: 72\n--------\n+40 to Dexterity\n--------\nThe great bear sleeps beneath the ice\n";

    #[test]
    fn reconnait_un_objet() {
        assert!(looks_like_item(BOW));
        assert!(!looks_like_item("bonjour"));
        assert!(!looks_like_item("Item Class: Bows\nsans rareté"));
        assert!(looks_like_item(FRANCAIS));
    }

    #[test]
    fn lit_en_tete_niveau_mods_et_valeurs() {
        let item = parse(BOW).unwrap();
        assert_eq!(item.item_class, "Bows");
        assert_eq!(item.rarity, Rarity::Rare);
        assert_eq!(item.name.as_deref(), Some("Storm Thirst"));
        assert_eq!(item.base_type, "Advanced Dualstring Bow");
        assert_eq!(item.item_level, Some(81));
        assert!(item.corrupted);
        assert!(!item.advanced);
        let mods: Vec<_> = item
            .mods
            .iter()
            .map(|m| (m.text.as_str(), m.kind, m.values.clone()))
            .collect();
        assert_eq!(
            mods,
            vec![
                (
                    "Bow Attacks fire an additional Arrow",
                    ModKind::Implicit,
                    vec![]
                ),
                (
                    "168% increased Physical Damage",
                    ModKind::Explicit,
                    vec![168.0]
                ),
                (
                    "Adds 4 to 131 Lightning Damage",
                    ModKind::Explicit,
                    vec![4.0, 131.0]
                ),
            ]
        );
    }

    #[test]
    fn lit_le_format_avance_du_corpus() {
        let item = parse(&corpus("rare-casque-avance.txt")).unwrap();
        assert!(item.advanced);
        assert_eq!(item.quality, Some(20));
        assert_eq!(item.item_level, Some(81));
        let rune = &item.mods[0];
        assert_eq!(rune.kind, ModKind::Rune);
        assert_eq!(rune.affix, None);
        let es = &item.mods[1];
        assert_eq!(es.text, "+38 to maximum Energy Shield");
        assert_eq!(es.affix, Some(Affix::Prefix));
        assert_eq!(es.tier, Some(3));
        assert_eq!(es.values, vec![38.0]);
        let froid = item.mods.last().unwrap();
        assert_eq!(froid.affix, Some(Affix::Suffix));
        assert_eq!(froid.tier, Some(4));
        assert_eq!(froid.values, vec![-10.0]);
    }

    #[test]
    fn ignore_le_texte_d_ambiance_des_uniques() {
        let item = parse(UNIQUE).unwrap();
        assert_eq!(item.mods.len(), 1);
        assert_eq!(item.mods[0].text, "+40 to Dexterity");
    }

    #[test]
    fn tout_le_corpus_passe() {
        let dossier =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/corpus/objets");
        let mut n = 0;
        for e in std::fs::read_dir(dossier).unwrap().flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("txt") {
                continue;
            }
            let texte = std::fs::read_to_string(&p).unwrap();
            assert!(looks_like_item(&texte), "{} écarté", p.display());
            parse(&texte).unwrap_or_else(|e| panic!("{} : {}", p.display(), e.message));
            n += 1;
        }
        assert!(n >= 10);
    }

    #[test]
    fn explique_les_refus() {
        let err = parse("Hello world").unwrap_err();
        assert_eq!(err.code, ErrorCode::ParseFailed);
        let err = parse(FRANCAIS).unwrap_err();
        assert!(err.message.contains("client anglais"), "{}", err.message);
    }
}
