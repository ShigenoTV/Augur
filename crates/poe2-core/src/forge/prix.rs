//! Table de prix de Forge, en Exalted Orbs, saisie par le joueur.
//!
//! Aucune source de prix officielle n'est ouverte (cadrage, section « Prix ») : la table part de
//! valeurs **indicatives** que le joueur corrige. Elle couvre chaque monnaie et Omen du
//! [catalogue](super::catalogue), plus la base neuve et sa revente (abandon d'un craft).

use super::catalogue::{MONNAIES, OMENS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Prix d'une base neuve (point de départ et coût d'un abandon).
pub const BASE_NEUVE: &str = "base_neuve";
/// Ce que rapporte la revente d'une base ratée (déduit du coût d'un abandon).
pub const REVENTE_BASE: &str = "revente_base";

/// Origine des valeurs indicatives, affichée avec la table tant que le joueur ne l'a pas corrigée.
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

/// Table de prix enregistrée en local.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePrix {
    /// identifiant -> prix en Exalted Orbs
    pub prix: BTreeMap<String, f64>,
    /// D'où viennent les valeurs (indicatives, ou date de la dernière saisie du joueur).
    pub origine: String,
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

impl TablePrix {
    pub fn indicative() -> Self {
        Self {
            prix: INDICATIFS
                .iter()
                .map(|(k, v)| ((*k).to_string(), *v))
                .collect(),
            origine: ORIGINE_INDICATIVE.to_string(),
        }
    }

    /// Applique une saisie du joueur : chaque prix doit être connu, fini et positif ou nul.
    /// Rien n'est modifié si une valeur est refusée.
    pub fn saisir(
        &mut self,
        saisie: &BTreeMap<String, f64>,
        origine: &str,
    ) -> Result<(), ErreurPrix> {
        let connus: Vec<&str> = identifiants().collect();
        for (id, p) in saisie {
            if !connus.contains(&id.as_str()) {
                return Err(ErreurPrix::Inconnu(id.clone()));
            }
            if !p.is_finite() || *p < 0.0 {
                return Err(ErreurPrix::Invalide(id.clone()));
            }
        }
        let mut nouveau = self.prix.clone();
        nouveau.extend(saisie.iter().map(|(k, v)| (k.clone(), *v)));
        let base = nouveau.get(BASE_NEUVE).copied().unwrap_or(0.0);
        if nouveau.get(REVENTE_BASE).copied().unwrap_or(0.0) > base {
            return Err(ErreurPrix::ReventeTropHaute(REVENTE_BASE.into()));
        }
        self.prix = nouveau;
        self.origine = origine.to_string();
        Ok(())
    }

    /// Relit une table enregistrée : les valeurs invalides ou inconnues sont écartées et les
    /// identifiants manquants reprennent leur valeur indicative (fichier ancien ou modifié à la
    /// main).
    pub fn reparer(mut self) -> Self {
        let connus: Vec<&str> = identifiants().collect();
        self.prix
            .retain(|k, v| connus.contains(&k.as_str()) && v.is_finite() && *v >= 0.0);
        for (k, v) in INDICATIFS {
            self.prix.entry(k.to_string()).or_insert(v);
        }
        if self.prix[REVENTE_BASE] > self.prix[BASE_NEUVE] {
            self.prix.insert(REVENTE_BASE.to_string(), 0.0);
        }
        self
    }

    /// Prix de la base neuve et de sa revente (paramètres du solveur).
    pub fn base(&self) -> (f64, f64) {
        (
            self.prix.get(BASE_NEUVE).copied().unwrap_or(0.0),
            self.prix.get(REVENTE_BASE).copied().unwrap_or(0.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::catalogue::actions;

    #[test]
    fn la_table_indicative_couvre_tout_le_catalogue() {
        let t = TablePrix::indicative();
        assert_eq!(t.prix.len(), identifiants().count());
        let a = actions(&t.prix, None).unwrap();
        assert_eq!(a.len(), 18 + 14 + 1);
        assert_eq!(t.base(), (2.0, 0.0));
    }

    #[test]
    fn saisie_validee_en_bloc() {
        let mut t = TablePrix::indicative();
        let ok: BTreeMap<String, f64> =
            [("exalt".to_string(), 1.0), ("chaos".to_string(), 40.0)].into();
        t.saisir(&ok, "saisie du 07/11/2026").unwrap();
        assert_eq!(
            (t.prix["chaos"], t.origine.as_str()),
            (40.0, "saisie du 07/11/2026")
        );

        let avant = t.clone();
        let mauvais: BTreeMap<String, f64> =
            [("chaos".to_string(), 1.0), ("divine".to_string(), 200.0)].into();
        assert_eq!(
            t.saisir(&mauvais, "x"),
            Err(ErreurPrix::Inconnu("divine".into()))
        );
        let nan: BTreeMap<String, f64> = [("chaos".to_string(), f64::NAN)].into();
        assert_eq!(
            t.saisir(&nan, "x"),
            Err(ErreurPrix::Invalide("chaos".into()))
        );
        let revente: BTreeMap<String, f64> = [(REVENTE_BASE.to_string(), 5.0)].into();
        assert!(matches!(
            t.saisir(&revente, "x"),
            Err(ErreurPrix::ReventeTropHaute(_))
        ));
        assert_eq!(t, avant, "rien n'est modifié sur un refus");
    }

    #[test]
    fn un_fichier_abime_est_repare() {
        let json =
            r#"{"prix":{"exalt":2.5,"chaos":-1,"divine":300,"revente_base":9},"origine":"vieux"}"#;
        let t: TablePrix = serde_json::from_str(json).unwrap();
        let t = t.reparer();
        assert_eq!(t.prix["exalt"], 2.5);
        assert_eq!(t.prix["chaos"], 56.4, "valeur négative remplacée");
        assert!(!t.prix.contains_key("divine"));
        assert_eq!(t.prix[REVENTE_BASE], 0.0);
        assert_eq!(t.prix.len(), identifiants().count());
    }
}
