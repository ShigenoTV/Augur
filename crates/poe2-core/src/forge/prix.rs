//! Table de prix de Forge, en Exalted Orbs.
//!
//! Trois sources, de la plus forte à la plus faible (décision de Max du 07/11/2026 : poe.ninja
//! **en plus** des prix saisis) :
//!
//! 1. les **corrections** saisies par le joueur ;
//! 2. le dernier **relevé poe.ninja** ([`super::ninja`]) ;
//! 3. des valeurs **indicatives** (relevé de septembre 2026 de poe2-craft-assistant).
//!
//! La table couvre chaque monnaie et Omen du [catalogue](super::catalogue), plus la base neuve et
//! sa revente (abandon d'un craft), qui n'ont pas de cote poe.ninja.

use super::catalogue::{MONNAIES, OMENS};
use super::ninja::ReleveMarche;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Prix d'une base neuve (point de départ et coût d'un abandon).
pub const BASE_NEUVE: &str = "base_neuve";
/// Ce que rapporte la revente d'une base ratée (déduit du coût d'un abandon).
pub const REVENTE_BASE: &str = "revente_base";

/// Origine des valeurs indicatives, à afficher avec elles.
pub const ORIGINE_INDICATIVE: &str = "Valeurs indicatives de septembre 2026 (relevé de \
    poe2-craft-assistant), à corriger aux prix du moment.";

/// Valeurs indicatives, en Exalted Orbs.
const INDICATIFS: [(&str, f64); 26] = [
    (BASE_NEUVE, 2.0),
    (REVENTE_BASE, 0.0),
    ("transmute", 1.53),
    ("transmute_greater", 1.78),
    ("transmute_perfect", 15.5),
    ("augment", 2.88),
    ("augment_greater", 5.34),
    ("augment_perfect", 135.0),
    ("regal", 2.53),
    ("regal_greater", 2.7),
    ("regal_perfect", 19.7),
    ("alchemy", 3.17),
    ("exalt", 1.0),
    ("exalt_greater", 4.78),
    ("exalt_perfect", 1379.0),
    ("chaos", 56.4),
    ("chaos_greater", 167.0),
    ("chaos_perfect", 2814.0),
    ("annul", 304.0),
    ("fracture", 4024.0),
    ("omen_sinistral_exaltation", 37.6),
    ("omen_dextral_exaltation", 19.2),
    ("omen_sinistral_annulment", 7464.0),
    ("omen_dextral_annulment", 4066.0),
    ("omen_sinistral_erasure", 6979.0),
    ("omen_dextral_erasure", 3944.0),
];

/// D'où vient un prix effectif.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourcePrix {
    Joueur,
    PoeNinja,
    Indicatif,
}

/// Table de prix enregistrée en local.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePrix {
    /// Prix saisis par le joueur (prioritaires).
    #[serde(default)]
    pub corrections: BTreeMap<String, f64>,
    /// Dernier relevé poe.ninja.
    #[serde(default)]
    pub marche: Option<ReleveMarche>,
    /// Prix effectifs, recalculés (identifiant -> prix en Exalted).
    #[serde(default)]
    pub prix: BTreeMap<String, f64>,
    /// Source de chaque prix effectif.
    #[serde(default)]
    pub sources: BTreeMap<String, SourcePrix>,
}

impl Default for TablePrix {
    fn default() -> Self {
        Self::indicative()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErreurPrix {
    #[error("identifiant de prix inconnu : {0}")]
    Inconnu(String),
    #[error("prix invalide pour {0} : nombre positif ou nul attendu")]
    Invalide(String),
    #[error("la revente d'une base ({0}) ne peut pas dépasser son prix")]
    ReventeTropHaute(String),
}

/// Identifiants qu'une table de prix peut contenir.
pub fn identifiants() -> impl Iterator<Item = &'static str> {
    [BASE_NEUVE, REVENTE_BASE]
        .into_iter()
        .chain(MONNAIES.iter().map(|m| m.id))
        .chain(OMENS.iter().map(|o| o.id))
}

fn est_connu(id: &str) -> bool {
    identifiants().any(|k| k == id)
}

fn valide(p: f64) -> bool {
    p.is_finite() && p >= 0.0
}

impl TablePrix {
    pub fn indicative() -> Self {
        let mut t = Self {
            corrections: BTreeMap::new(),
            marche: None,
            prix: BTreeMap::new(),
            sources: BTreeMap::new(),
        };
        t.recalculer();
        t
    }

    /// Recalcule les prix effectifs : correction, sinon poe.ninja, sinon indicatif.
    fn recalculer(&mut self) {
        self.prix.clear();
        self.sources.clear();
        for (id, indicatif) in INDICATIFS {
            let (p, s) = if let Some(p) = self.corrections.get(id) {
                (*p, SourcePrix::Joueur)
            } else if let Some(p) = self.marche.as_ref().and_then(|m| m.prix.get(id)) {
                (*p, SourcePrix::PoeNinja)
            } else {
                (indicatif, SourcePrix::Indicatif)
            };
            self.prix.insert(id.to_string(), p);
            self.sources.insert(id.to_string(), s);
        }
    }

    fn revente_coherente(&self) -> Result<(), ErreurPrix> {
        if self.prix[REVENTE_BASE] > self.prix[BASE_NEUVE] {
            return Err(ErreurPrix::ReventeTropHaute(REVENTE_BASE.into()));
        }
        Ok(())
    }

    /// Applique une saisie du joueur : chaque prix doit être connu, fini et positif ou nul.
    /// Rien n'est modifié si une valeur est refusée.
    pub fn saisir(&mut self, saisie: &BTreeMap<String, f64>) -> Result<(), ErreurPrix> {
        for (id, p) in saisie {
            if !est_connu(id) {
                return Err(ErreurPrix::Inconnu(id.clone()));
            }
            if !valide(*p) {
                return Err(ErreurPrix::Invalide(id.clone()));
            }
        }
        let mut nouveau = self.clone();
        nouveau
            .corrections
            .extend(saisie.iter().map(|(k, v)| (k.clone(), *v)));
        nouveau.recalculer();
        nouveau.revente_coherente()?;
        *self = nouveau;
        Ok(())
    }

    /// Oublie les corrections du joueur (le relevé poe.ninja reste).
    pub fn effacer_corrections(&mut self) {
        self.corrections.clear();
        self.recalculer();
    }

    /// Remplace le relevé poe.ninja (déjà fusionné avec le précédent par `ninja::releve`). Les
    /// cotes invalides sont ignorées.
    pub fn appliquer_marche(&mut self, mut releve: ReleveMarche) {
        releve.prix.retain(|k, v| est_connu(k) && valide(*v));
        self.marche = Some(releve);
        self.recalculer();
    }

    /// Relit une table enregistrée : valeurs invalides ou inconnues écartées, prix effectifs
    /// recalculés (fichier ancien ou modifié à la main).
    pub fn reparer(mut self) -> Self {
        self.corrections.retain(|k, v| est_connu(k) && valide(*v));
        if let Some(m) = self.marche.as_mut() {
            m.prix.retain(|k, v| est_connu(k) && valide(*v));
        }
        self.recalculer();
        if self.revente_coherente().is_err() {
            self.corrections.remove(REVENTE_BASE);
            self.recalculer();
        }
        self
    }

    /// Prix de la base neuve et de sa revente (paramètres du solveur).
    pub fn base(&self) -> (f64, f64) {
        (self.prix[BASE_NEUVE], self.prix[REVENTE_BASE])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::catalogue::actions;

    fn saisie(paires: &[(&str, f64)]) -> BTreeMap<String, f64> {
        paires.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
    }

    fn marche(paires: &[(&str, f64)]) -> ReleveMarche {
        ReleveMarche {
            ligue: "L".into(),
            releve_a: 1,
            prix: saisie(paires),
            ..Default::default()
        }
    }

    #[test]
    fn la_table_indicative_couvre_tout_le_catalogue() {
        let t = TablePrix::indicative();
        assert_eq!(t.prix.len(), identifiants().count());
        assert!(t.sources.values().all(|s| *s == SourcePrix::Indicatif));
        let a = actions(&t.prix, None).unwrap();
        assert_eq!(a.len(), 18 + 14 + 1);
        assert_eq!(t.base(), (2.0, 0.0));
    }

    #[test]
    fn joueur_puis_poe_ninja_puis_indicatif() {
        let mut t = TablePrix::indicative();
        t.appliquer_marche(marche(&[
            ("chaos", 60.0),
            ("annul", 250.0),
            ("divine", 600.0),
            ("exalt", f64::NAN),
        ]));
        t.saisir(&saisie(&[("chaos", 40.0)])).unwrap();
        assert_eq!(
            (t.prix["chaos"], t.sources["chaos"]),
            (40.0, SourcePrix::Joueur)
        );
        assert_eq!(
            (t.prix["annul"], t.sources["annul"]),
            (250.0, SourcePrix::PoeNinja)
        );
        assert_eq!(
            (t.prix["exalt"], t.sources["exalt"]),
            (1.0, SourcePrix::Indicatif),
            "cote invalide ignorée"
        );
        assert!(!t.prix.contains_key("divine"));

        t.effacer_corrections();
        assert_eq!(
            (t.prix["chaos"], t.sources["chaos"]),
            (60.0, SourcePrix::PoeNinja)
        );
    }

    #[test]
    fn saisie_validee_en_bloc() {
        let mut t = TablePrix::indicative();
        let avant = t.clone();
        assert_eq!(
            t.saisir(&saisie(&[("chaos", 1.0), ("divine", 200.0)])),
            Err(ErreurPrix::Inconnu("divine".into()))
        );
        assert_eq!(
            t.saisir(&saisie(&[("chaos", f64::NAN)])),
            Err(ErreurPrix::Invalide("chaos".into()))
        );
        assert!(matches!(
            t.saisir(&saisie(&[(REVENTE_BASE, 5.0)])),
            Err(ErreurPrix::ReventeTropHaute(_))
        ));
        assert_eq!(t, avant, "rien n'est modifié sur un refus");
    }

    #[test]
    fn un_fichier_abime_est_repare() {
        let json = r#"{"corrections":{"exalt":2.5,"chaos":-1,"divine":300,"revente_base":9},
            "marche":{"ligue":"L","releveA":1,"prix":{"annul":-3,"regal":2},"dates":{},"manquants":[]}}"#;
        let t: TablePrix = serde_json::from_str(json).unwrap();
        let t = t.reparer();
        assert_eq!(t.prix["exalt"], 2.5);
        assert_eq!(t.prix["chaos"], 56.4, "valeur négative remplacée");
        assert_eq!(
            (t.prix["regal"], t.sources["regal"]),
            (2.0, SourcePrix::PoeNinja)
        );
        assert_eq!(t.sources["annul"], SourcePrix::Indicatif);
        assert!(!t.prix.contains_key("divine"));
        assert_eq!(t.prix[REVENTE_BASE], 0.0);
        assert_eq!(t.prix.len(), identifiants().count());
        // un fichier de la version précédente (prix seuls) repart des valeurs indicatives
        let ancien: TablePrix =
            serde_json::from_str(r#"{"prix":{"exalt":9},"origine":"x"}"#).unwrap();
        assert_eq!(ancien.reparer(), TablePrix::indicative());
    }
}
