//! `evaluate_item` : gain de DPS de l'objet copié pour le personnage importé
//! (calcul de `poe2_core::comparaison`), converti vers le contrat `ItemDelta`.
//!
//! Seul le DPS est calculé pour l'instant : l'EHP est renvoyé à zéro
//! (avant = après = 0) et l'overlay masque alors sa ligne.

use std::time::Instant;

use poe2_core::comparaison::{self, Comparaison, ErreurComparaison};

use crate::contracts::{
    AppError, Confidence, ErrorCode, ItemDelta, MechanicNote, ParsedItem, StatDelta, SupportLevel,
    app_error,
};

/// Identifiant de contrat d'un emplacement PoB (libellés dans `@augur/ui`).
pub fn slot_id(emplacement: &str) -> String {
    match emplacement {
        "Weapon 1" => "weapon1",
        "Weapon 2" => "offhand1",
        "Helmet" => "helmet",
        "Body Armour" => "bodyArmour",
        "Gloves" => "gloves",
        "Boots" => "boots",
        "Belt" => "belt",
        "Amulet" => "amulet",
        "Ring 1" => "ring1",
        "Ring 2" => "ring2",
        autre => autre,
    }
    .to_string()
}

fn note(mechanic: &str) -> MechanicNote {
    note_avec(mechanic, Vec::new())
}

fn note_avec(mechanic: &str, details: Vec<String>) -> MechanicNote {
    MechanicNote {
        mechanic: mechanic.to_string(),
        confidence: Confidence::Assumed,
        details,
    }
}

fn zero() -> StatDelta {
    StatDelta {
        before: 0.0,
        after: 0.0,
    }
}

fn unsupported(slot: &str, ms: u32) -> ItemDelta {
    ItemDelta {
        slot: slot.to_string(),
        equipped_name: None,
        weapon_set: None,
        support: SupportLevel::Unsupported,
        dps: zero(),
        ehp: zero(),
        resistances: Vec::new(),
        spirit: None,
        mechanics: Vec::new(),
        compute_ms: ms,
    }
}

/// Convertit une comparaison du moteur. Toujours « approximatif » : les
/// formules de l'archétype sont encore « supposées » (fiches du Theorycrafter).
pub fn to_delta(c: &Comparaison, ms: u32) -> ItemDelta {
    let r = &c.rapport;
    let mut mechanics = vec![note("formules-supposees")];
    if r.formule_generique {
        mechanics.push(note_avec(
            "formule-generique",
            vec![format!(
                "{} : dégâts de base, cadence et critique du jeu ; ses effets propres (projectiles, zones, chaînes, conditions) ne sont pas comptés",
                c.competence
            )],
        ));
    }
    if !r.mods_candidat_non_traduits.is_empty() {
        mechanics.push(note_avec(
            "mods-objet-non-traduits",
            r.mods_candidat_non_traduits.clone(),
        ));
    }
    if !r.mods_non_traduits.is_empty() {
        mechanics.push(note_avec(
            "mods-equipement-non-traduits",
            r.mods_non_traduits.clone(),
        ));
    }
    if r.passifs_non_traduits > 0 || r.passifs_inconnus > 0 {
        let mut details: Vec<String> = r
            .lignes_passifs_non_traduites
            .iter()
            .map(|(l, n)| {
                if *n > 1 {
                    format!("{l} (×{n})")
                } else {
                    l.clone()
                }
            })
            .collect();
        if r.passifs_inconnus > 0 {
            details.push(format!(
                "{} passifs absents de l'arbre d'Augur",
                r.passifs_inconnus
            ));
        }
        mechanics.push(note_avec("passifs-non-traduits", details));
    }
    if !r.supports_non_modelises.is_empty() {
        mechanics.push(note_avec(
            "supports-non-modelises",
            r.supports_non_modelises.iter().cloned().collect(),
        ));
    }
    if r.arbre_autre_version {
        mechanics.push(note("arbre-autre-version"));
    }
    let weapon_set = matches!(c.emplacement.as_str(), "Weapon 1" | "Weapon 2").then_some(1);
    ItemDelta {
        slot: slot_id(&c.emplacement),
        equipped_name: c.remplace.clone(),
        weapon_set,
        support: SupportLevel::Approximate,
        dps: StatDelta {
            before: c.dps_avant,
            after: c.dps_apres,
        },
        ehp: zero(),
        resistances: Vec::new(),
        spirit: None,
        mechanics,
        compute_ms: ms,
    }
}

/// Évalue l'objet pour le build ; `None` : aucun personnage importé.
pub fn evaluate(
    build: Option<&poe2_core::pob::BuildPob>,
    item: &ParsedItem,
) -> Result<ItemDelta, AppError> {
    let Some(build) = build else {
        return Err(app_error(
            ErrorCode::NoCharacter,
            "Importe ton personnage (code Path of Building) dans la fenêtre Augur.",
        ));
    };
    let debut = Instant::now();
    let nom = item.name.as_deref().unwrap_or(&item.base_type);
    let lignes: Vec<String> = item.mods.iter().map(|m| m.text.clone()).collect();
    let resultat = comparaison::comparer(build, &item.item_class, nom, &lignes);
    let ms = u32::try_from(debut.elapsed().as_millis()).unwrap_or(u32::MAX);
    match resultat {
        Ok(c) => Ok(to_delta(&c, ms)),
        Err(
            ErreurComparaison::CompetenceNonSupportee(_)
            | ErreurComparaison::AucuneCompetence
            | ErreurComparaison::EmplacementNonSupporte(_),
        ) => Ok(unsupported(
            &comparaison::emplacements(&item.item_class)
                .and_then(|(e, _)| e.first())
                .map_or_else(String::new, |e| slot_id(e)),
            ms,
        )),
        Err(e @ ErreurComparaison::Calcul(_)) => {
            log::warn!("évaluation impossible : {e}");
            Err(app_error(
                ErrorCode::Internal,
                format!("Comparaison impossible ({e})."),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{ItemMod, ModKind, Rarity};

    fn baguette() -> ParsedItem {
        ParsedItem {
            raw_text: String::new(),
            advanced: false,
            item_class: "Wands".into(),
            rarity: Rarity::Rare,
            name: Some("Storm Song".into()),
            base_type: "Withered Wand".into(),
            item_level: Some(80),
            quality: None,
            mods: vec![ItemMod {
                text: "100% increased Lightning Damage".into(),
                kind: ModKind::Explicit,
                affix: None,
                tier: None,
                stat_ids: Vec::new(),
                values: vec![100.0],
            }],
            corrupted: false,
            unidentified: false,
            data_patch: String::new(),
        }
    }

    fn build(gemme: &str) -> poe2_core::pob::BuildPob {
        poe2_core::pob::lire_xml(&format!(
            r#"<PathOfBuilding2><Build level="90" className="Sorceress" mainSocketGroup="1"/>
<Skills activeSkillSet="1"><SkillSet id="1"><Skill enabled="true">{gemme}</Skill></SkillSet></Skills>
<Items activeItemSet="1"><Item id="1">Rarity: RARE
Old
Withered Wand
Implicits: 0
20% increased Lightning Damage
</Item><ItemSet id="1"><Slot name="Weapon 1" itemId="1"/></ItemSet></Items></PathOfBuilding2>"#
        ))
        .unwrap()
    }

    #[test]
    fn sans_personnage() {
        let e = evaluate(None, &baguette()).unwrap_err();
        assert_eq!(e.code, ErrorCode::NoCharacter);
    }

    #[test]
    fn delta_de_dps() {
        let b = build(r#"<Gem nameSpec="Arc" skillId="ArcPlayer" level="20"/>"#);
        let d = evaluate(Some(&b), &baguette()).unwrap();
        assert_eq!(d.slot, "weapon1");
        assert_eq!(d.equipped_name.as_deref(), Some("Old"));
        assert_eq!(d.support, SupportLevel::Approximate);
        assert!((d.dps.after / d.dps.before - 2.0 / 1.2).abs() < 1e-9);
        assert_eq!((d.ehp.before, d.ehp.after), (0.0, 0.0));
    }

    #[test]
    fn detaille_ce_que_le_calcul_ignore() {
        let b = build(
            r#"<Gem nameSpec="Arc" skillId="ArcPlayer" level="20"/><Gem nameSpec="Mystery" skillId="SupportMysteryPlayer" level="1"/>"#,
        );
        let mut objet = baguette();
        objet.mods.push(ItemMod {
            text: "Triggered Spells deal 16% increased Spell Damage".into(),
            ..objet.mods[0].clone()
        });
        let d = evaluate(Some(&b), &objet).unwrap();
        let details = |m: &str| {
            d.mechanics
                .iter()
                .find(|n| n.mechanic == m)
                .map(|n| n.details.clone())
                .unwrap_or_default()
        };
        assert_eq!(
            details("mods-objet-non-traduits"),
            vec!["Triggered Spells deal 16% increased Spell Damage"]
        );
        assert_eq!(details("supports-non-modelises"), vec!["Mystery"]);
        assert!(details("formules-supposees").is_empty());
    }

    #[test]
    fn sort_generique_approximatif() {
        let b = build(r#"<Gem nameSpec="Fireball" skillId="FireballPlayer" level="20"/>"#);
        let d = evaluate(Some(&b), &baguette()).unwrap();
        assert_eq!(d.support, SupportLevel::Approximate);
        assert!(
            d.mechanics
                .iter()
                .any(|n| n.mechanic == "formule-generique")
        );
    }

    #[test]
    fn archetype_non_couvert() {
        let b =
            build(r#"<Gem nameSpec="Lightning Arrow" skillId="LightningArrowPlayer" level="20"/>"#);
        let d = evaluate(Some(&b), &baguette()).unwrap();
        assert_eq!(d.support, SupportLevel::Unsupported);
        assert_eq!(d.slot, "weapon1");
    }
}
