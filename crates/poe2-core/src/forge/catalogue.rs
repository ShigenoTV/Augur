//! Catalogue des actions de craft de Forge : monnaies (et leurs versions Greater / Perfect) et
//! Omens qui s'y combinent. Repris du jeu de données de poe2-craft-assistant (règles seulement,
//! sans les prix ni les poids).
//!
//! Le coût d'une action est en Exalted Orbs, lu dans une table de prix fournie par l'appelant
//! (`prix`, identifiant -> prix). Un prix manquant est une erreur : jamais de prix inventé.
//!
//! Les Essences dépendent de la classe d'objet : elles sont ajoutées par `atelier::planifier`, d'après
//! les données embarquées. Pas encore au catalogue : Désécration (os abyssaux) et ses Omens, Liquid
//! Emotions. Le moteur sait les appliquer ; il leur manque les poids des mods désacralisés.

use super::moteur::{Currency, CurrencyKind, Slot};
use super::solveur::{Action, ActionKind};
use std::collections::{BTreeMap, BTreeSet};

/// Une monnaie du catalogue.
#[derive(Clone, Copy, Debug)]
pub struct Monnaie {
    pub id: &'static str,
    pub nom: &'static str,
    pub genre: CurrencyKind,
    /// Niveau de mod minimum (Craft of Exile, émulateur 0.5.5.5) : Transmutation et Augmentation
    /// Greater 44 / Perfect 70 ; Regal, Exalted et Chaos Greater 35 / Perfect 50.
    pub niveau_min: u8,
}

/// Un Omen qui modifie une monnaie.
#[derive(Clone, Copy, Debug)]
pub struct Omen {
    pub id: &'static str,
    pub nom: &'static str,
    pub s_applique_a: CurrencyKind,
    /// Slot forcé de l'affixe ajouté (Exaltation).
    pub ajout: Option<Slot>,
    /// Slot auquel le retrait est restreint (Annulment, Erasure).
    pub retrait: Option<Slot>,
}

const fn monnaie(
    id: &'static str,
    nom: &'static str,
    genre: CurrencyKind,
    niveau_min: u8,
) -> Monnaie {
    Monnaie {
        id,
        nom,
        genre,
        niveau_min,
    }
}

pub const MONNAIES: [Monnaie; 18] = {
    use CurrencyKind::*;
    [
        monnaie("transmute", "Orb of Transmutation", Transmute, 0),
        monnaie(
            "transmute_greater",
            "Greater Orb of Transmutation",
            Transmute,
            44,
        ),
        monnaie(
            "transmute_perfect",
            "Perfect Orb of Transmutation",
            Transmute,
            70,
        ),
        monnaie("augment", "Orb of Augmentation", Augment, 0),
        monnaie(
            "augment_greater",
            "Greater Orb of Augmentation",
            Augment,
            44,
        ),
        monnaie(
            "augment_perfect",
            "Perfect Orb of Augmentation",
            Augment,
            70,
        ),
        monnaie("regal", "Regal Orb", Regal, 0),
        monnaie("regal_greater", "Greater Regal Orb", Regal, 35),
        monnaie("regal_perfect", "Perfect Regal Orb", Regal, 50),
        monnaie("alchemy", "Orb of Alchemy", Alchemy, 0),
        monnaie("exalt", "Exalted Orb", Exalt, 0),
        monnaie("exalt_greater", "Greater Exalted Orb", Exalt, 35),
        monnaie("exalt_perfect", "Perfect Exalted Orb", Exalt, 50),
        monnaie("chaos", "Chaos Orb", Chaos, 0),
        monnaie("chaos_greater", "Greater Chaos Orb", Chaos, 35),
        monnaie("chaos_perfect", "Perfect Chaos Orb", Chaos, 50),
        monnaie("annul", "Orb of Annulment", Annul, 0),
        monnaie("fracture", "Fracturing Orb", Fracture, 0),
    ]
};

pub const OMENS: [Omen; 6] = {
    use CurrencyKind::*;
    use Slot::*;
    [
        Omen {
            id: "omen_sinistral_exaltation",
            nom: "Omen of Sinistral Exaltation",
            s_applique_a: Exalt,
            ajout: Some(Prefix),
            retrait: None,
        },
        Omen {
            id: "omen_dextral_exaltation",
            nom: "Omen of Dextral Exaltation",
            s_applique_a: Exalt,
            ajout: Some(Suffix),
            retrait: None,
        },
        Omen {
            id: "omen_sinistral_annulment",
            nom: "Omen of Sinistral Annulment",
            s_applique_a: Annul,
            ajout: None,
            retrait: Some(Prefix),
        },
        Omen {
            id: "omen_dextral_annulment",
            nom: "Omen of Dextral Annulment",
            s_applique_a: Annul,
            ajout: None,
            retrait: Some(Suffix),
        },
        Omen {
            id: "omen_sinistral_erasure",
            nom: "Omen of Sinistral Erasure",
            s_applique_a: Chaos,
            ajout: None,
            retrait: Some(Prefix),
        },
        Omen {
            id: "omen_dextral_erasure",
            nom: "Omen of Dextral Erasure",
            s_applique_a: Chaos,
            ajout: None,
            retrait: Some(Suffix),
        },
    ]
};

/// Identifiant d'une action « monnaie + Omen ».
pub fn id_combinee(monnaie: &str, omen: &str) -> String {
    format!("{monnaie}+{omen}")
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErreurCatalogue {
    #[error("prix manquant : {0}")]
    PrixManquant(String),
    #[error("prix invalide pour {0} (nombre positif ou nul attendu)")]
    PrixInvalide(String),
}

fn prix_de(prix: &BTreeMap<String, f64>, id: &str) -> Result<f64, ErreurCatalogue> {
    let p = *prix
        .get(id)
        .ok_or_else(|| ErreurCatalogue::PrixManquant(id.to_string()))?;
    if p.is_finite() && p >= 0.0 {
        Ok(p)
    } else {
        Err(ErreurCatalogue::PrixInvalide(id.to_string()))
    }
}

/// Actions de craft autorisées, avec leur coût, plus l'action « abandon » (repartir d'une base
/// neuve) que le solveur utilise toujours.
///
/// `actives` : identifiants des actions permises (monnaies et combinaisons `id_combinee`) ;
/// `None` = tout le catalogue. Seules les actions retenues ont besoin d'un prix.
pub fn actions(
    prix: &BTreeMap<String, f64>,
    actives: Option<&BTreeSet<String>>,
) -> Result<Vec<Action>, ErreurCatalogue> {
    let permise = |id: &str| actives.is_none_or(|a| a.contains(id));
    let mut out = Vec::new();
    for m in &MONNAIES {
        let base = Currency {
            id: m.id.to_string(),
            label: m.nom.to_string(),
            kind: m.genre,
            min_mod_level: m.niveau_min,
            add_slot: None,
            remove_slot: None,
            target: None,
            alt_target: None,
            require_tag: None,
            remove_desecrated_only: false,
            remove_lowest_level: false,
            requires_rare: false,
            unit_cost: 0.0,
        };
        if permise(m.id) {
            let cout = prix_de(prix, m.id)?;
            out.push(action(Currency {
                unit_cost: cout,
                ..base.clone()
            }));
        }
        for o in OMENS.iter().filter(|o| o.s_applique_a == m.genre) {
            let id = id_combinee(m.id, o.id);
            if !permise(&id) {
                continue;
            }
            let cout = prix_de(prix, m.id)? + prix_de(prix, o.id)?;
            out.push(action(Currency {
                label: format!("{} + {}", m.nom, o.nom),
                add_slot: o.ajout,
                remove_slot: o.retrait,
                unit_cost: cout,
                id,
                ..base.clone()
            }));
        }
    }
    out.push(Action {
        id: "abandon".into(),
        label: "Repartir d'une base neuve".into(),
        cost: 0.0,
        kind: ActionKind::Abandon,
    });
    Ok(out)
}

fn action(c: Currency) -> Action {
    Action {
        id: c.id.clone(),
        label: c.label.clone(),
        cost: c.unit_cost,
        kind: ActionKind::Currency(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tous_les_prix(v: f64) -> BTreeMap<String, f64> {
        MONNAIES
            .iter()
            .map(|m| m.id)
            .chain(OMENS.iter().map(|o| o.id))
            .map(|id| (id.to_string(), v))
            .collect()
    }

    #[test]
    fn catalogue_complet() {
        let a = actions(&tous_les_prix(1.0), None).unwrap();
        // 18 monnaies + Exalt×3 + Chaos×3 (2 Omens chacune) + Annul (2 Omens) + abandon
        assert_eq!(a.len(), 18 + 3 * 2 + 3 * 2 + 2 + 1);
        let combo = a
            .iter()
            .find(|x| x.id == "exalt_greater+omen_dextral_exaltation")
            .unwrap();
        assert_eq!(combo.cost, 2.0, "monnaie + Omen");
        let ActionKind::Currency(c) = &combo.kind else {
            panic!()
        };
        assert_eq!(
            (c.kind, c.min_mod_level, c.add_slot),
            (CurrencyKind::Exalt, 35, Some(Slot::Suffix))
        );
    }

    #[test]
    fn seules_les_actions_retenues_demandent_un_prix() {
        let actives: BTreeSet<String> = [
            "exalt".to_string(),
            id_combinee("annul", "omen_sinistral_annulment"),
        ]
        .into();
        let prix: BTreeMap<String, f64> = [
            ("exalt".to_string(), 1.0),
            ("annul".to_string(), 300.0),
            ("omen_sinistral_annulment".to_string(), 7000.0),
        ]
        .into();
        let a = actions(&prix, Some(&actives)).unwrap();
        let ids: Vec<_> = a.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["exalt", "annul+omen_sinistral_annulment", "abandon"]);
        assert_eq!(a[1].cost, 7300.0);

        let sans_omen: BTreeMap<String, f64> =
            [("exalt".to_string(), 1.0), ("annul".to_string(), 300.0)].into();
        assert_eq!(
            actions(&sans_omen, Some(&actives)).unwrap_err(),
            ErreurCatalogue::PrixManquant("omen_sinistral_annulment".into())
        );
        let negatif: BTreeMap<String, f64> = [("exalt".to_string(), -1.0)].into();
        let seul: BTreeSet<String> = ["exalt".to_string()].into();
        assert_eq!(
            actions(&negatif, Some(&seul)).unwrap_err(),
            ErreurCatalogue::PrixInvalide("exalt".into())
        );
    }
}
