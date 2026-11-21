//! Catalogue des actions de craft de Forge : monnaies (et leurs versions Greater / Perfect) et
//! Omens qui s'y combinent. Repris du jeu de données de poe2-craft-assistant (règles seulement,
//! sans les prix ni les poids).
//!
//! Le coût d'une action est en Exalted Orbs, lu dans une table de prix fournie par l'appelant
//! (`prix`, identifiant -> prix). Un prix manquant est une erreur : jamais de prix inventé.
//!
//! Les Essences dépendent de la classe d'objet : elles sont ajoutées par `atelier::planifier`, d'après
//! les données embarquées, avec les Omens of Crystallisation. Pas encore au catalogue : Désécration (os
//! abyssaux) et ses Omens, Liquid Emotions. Le moteur sait les appliquer ; il leur manque les poids des
//! mods désacralisés.

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
    /// Slot forcé de l'affixe ajouté (Exaltation, Coronation) ; pour l'Alchemy, le côté rempli au
    /// maximum.
    pub ajout: Option<Slot>,
    /// Slot auquel le retrait est restreint (Annulment, Erasure, Crystallisation).
    pub retrait: Option<Slot>,
    /// Omen of Whittling : le Chaos retire le mod du niveau le plus bas.
    pub plus_bas: bool,
    /// Omen of Greater Exaltation / Annulment : deux mods ajoutés ou retirés.
    pub double: bool,
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

const fn omen(
    id: &'static str,
    nom: &'static str,
    s_applique_a: CurrencyKind,
    ajout: Option<Slot>,
    retrait: Option<Slot>,
) -> Omen {
    Omen {
        id,
        nom,
        s_applique_a,
        ajout,
        retrait,
        plus_bas: false,
        double: false,
    }
}

/// Omens de craft d'affixes (hors Désécration). Pas au catalogue : Homogenising Exaltation /
/// Coronation (le mod ajouté partage une étiquette avec un mod présent : l'état abstrait du solveur ne
/// connaît pas les étiquettes des mods hors objectif), Catalysing Exaltation (qualité de catalyseur, que
/// Forge ne suit pas), et ceux qui ne touchent pas aux affixes (Sanctification, Blessed, Corruption,
/// Chance, Ancients…).
pub const OMENS: [Omen; 15] = {
    use CurrencyKind::*;
    use Slot::*;
    [
        omen(
            "omen_sinistral_exaltation",
            "Omen of Sinistral Exaltation",
            Exalt,
            Some(Prefix),
            None,
        ),
        omen(
            "omen_dextral_exaltation",
            "Omen of Dextral Exaltation",
            Exalt,
            Some(Suffix),
            None,
        ),
        Omen {
            double: true,
            ..omen(
                "omen_greater_exaltation",
                "Omen of Greater Exaltation",
                Exalt,
                None,
                None,
            )
        },
        omen(
            "omen_sinistral_annulment",
            "Omen of Sinistral Annulment",
            Annul,
            None,
            Some(Prefix),
        ),
        omen(
            "omen_dextral_annulment",
            "Omen of Dextral Annulment",
            Annul,
            None,
            Some(Suffix),
        ),
        Omen {
            double: true,
            ..omen(
                "omen_greater_annulment",
                "Omen of Greater Annulment",
                Annul,
                None,
                None,
            )
        },
        omen(
            "omen_sinistral_erasure",
            "Omen of Sinistral Erasure",
            Chaos,
            None,
            Some(Prefix),
        ),
        omen(
            "omen_dextral_erasure",
            "Omen of Dextral Erasure",
            Chaos,
            None,
            Some(Suffix),
        ),
        Omen {
            plus_bas: true,
            ..omen("omen_whittling", "Omen of Whittling", Chaos, None, None)
        },
        omen(
            "omen_sinistral_coronation",
            "Omen of Sinistral Coronation",
            Regal,
            Some(Prefix),
            None,
        ),
        omen(
            "omen_dextral_coronation",
            "Omen of Dextral Coronation",
            Regal,
            Some(Suffix),
            None,
        ),
        omen(
            "omen_sinistral_alchemy",
            "Omen of Sinistral Alchemy",
            Alchemy,
            Some(Prefix),
            None,
        ),
        omen(
            "omen_dextral_alchemy",
            "Omen of Dextral Alchemy",
            Alchemy,
            Some(Suffix),
            None,
        ),
        omen(
            "omen_sinistral_crystallisation",
            "Omen of Sinistral Crystallisation",
            Essence,
            None,
            Some(Prefix),
        ),
        omen(
            "omen_dextral_crystallisation",
            "Omen of Dextral Crystallisation",
            Essence,
            None,
            Some(Suffix),
        ),
    ]
};

/// Paires d'Omens utilisées ensemble (compatibles dans l'émulateur de Craft of Exile : seuls les
/// Omens opposés, Sinistral et Dextral, s'excluent).
pub const PAIRES: [(&str, &str); 4] = [
    ("omen_greater_exaltation", "omen_sinistral_exaltation"),
    ("omen_greater_exaltation", "omen_dextral_exaltation"),
    ("omen_greater_annulment", "omen_sinistral_annulment"),
    ("omen_greater_annulment", "omen_dextral_annulment"),
];

/// Omen du catalogue par identifiant.
pub fn omen_par_id(id: &str) -> Option<&'static Omen> {
    OMENS.iter().find(|o| o.id == id)
}

/// Omens qui s'appliquent à une monnaie de ce genre, seuls puis par paires.
pub fn combinaisons(genre: CurrencyKind) -> impl Iterator<Item = Vec<&'static Omen>> {
    let seuls = OMENS
        .iter()
        .filter(move |o| o.s_applique_a == genre)
        .map(|o| vec![o]);
    let paires = PAIRES.iter().filter_map(move |(a, b)| {
        let (a, b) = (omen_par_id(a)?, omen_par_id(b)?);
        (a.s_applique_a == genre && b.s_applique_a == genre).then(|| vec![a, b])
    });
    seuls.chain(paires)
}

/// Monnaie (ou Essence) modifiée par des Omens.
pub fn avec_omens(c: &Currency, omens: &[&Omen]) -> Currency {
    let mut c = c.clone();
    for o in omens {
        c.add_slot = o.ajout.or(c.add_slot);
        c.remove_slot = o.retrait.or(c.remove_slot);
        c.remove_lowest_level |= o.plus_bas;
        c.double |= o.double;
        c.id = format!("{}+{}", c.id, o.id);
        c.label = format!("{} + {}", c.label, o.nom);
    }
    c
}

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
            more_targets: Vec::new(),
            require_tag: None,
            remove_desecrated_only: false,
            remove_lowest_level: false,
            requires_rare: false,
            double: false,
            unit_cost: 0.0,
        };
        if permise(m.id) {
            let cout = prix_de(prix, m.id)?;
            out.push(action(Currency {
                unit_cost: cout,
                ..base.clone()
            }));
        }
        for omens in combinaisons(m.genre) {
            let ids: Vec<&str> = omens.iter().map(|o| o.id).collect();
            let id = id_combinee(m.id, &ids.join("+"));
            if !permise(&id) {
                continue;
            }
            let mut cout = prix_de(prix, m.id)?;
            for o in &ids {
                cout += prix_de(prix, o)?;
            }
            out.push(action(Currency {
                unit_cost: cout,
                ..avec_omens(&base, &omens)
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
        // 18 monnaies + Exalt×3 (3 Omens, 2 paires) + Chaos×3 (3 Omens) + Annul (3 Omens, 2 paires)
        // + Regal×3 (2 Omens) + Alchemy (2 Omens) + abandon
        assert_eq!(a.len(), 18 + 3 * 5 + 3 * 3 + 5 + 3 * 2 + 2 + 1);
        let deux = a
            .iter()
            .find(|x| x.id == "exalt+omen_greater_exaltation+omen_dextral_exaltation")
            .unwrap();
        assert_eq!(deux.cost, 3.0);
        let ActionKind::Currency(c) = &deux.kind else {
            panic!()
        };
        assert!(c.double && c.add_slot == Some(Slot::Suffix));
        let ActionKind::Currency(w) = &a
            .iter()
            .find(|x| x.id == "chaos+omen_whittling")
            .unwrap()
            .kind
        else {
            panic!()
        };
        assert!(w.remove_lowest_level);
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
