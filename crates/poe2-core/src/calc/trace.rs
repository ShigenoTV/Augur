//! Trace de calcul (« breakdown »).
//!
//! Chaque valeur produite par le moteur est accompagnée d'un arbre : la
//! formule appliquée, les modificateurs qui y ont contribué (avec leur
//! source) et les sous-calculs dont elle dépend. C'est l'outil de travail
//! du Theorycrafter : une erreur se trouve en lisant la trace, pas le code.

use std::fmt;

use serde::Serialize;

use super::modifier::TypeModificateur;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contribution {
    pub source: String,
    pub stat: String,
    #[serde(rename = "type")]
    pub kind: TypeModificateur,
    pub valeur: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Trace {
    pub libelle: String,
    pub valeur: f64,
    /// La formule en clair, avec les nombres substitués.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formule: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contributions: Vec<Contribution>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enfants: Vec<Trace>,
}

impl Trace {
    pub fn feuille(libelle: impl Into<String>, valeur: f64) -> Self {
        Self {
            libelle: libelle.into(),
            valeur,
            formule: None,
            contributions: Vec::new(),
            enfants: Vec::new(),
        }
    }

    pub fn avec_formule(mut self, formule: impl Into<String>) -> Self {
        self.formule = Some(formule.into());
        self
    }

    pub fn avec_enfant(mut self, enfant: Trace) -> Self {
        self.enfants.push(enfant);
        self
    }

    pub fn avec_enfants(mut self, enfants: impl IntoIterator<Item = Trace>) -> Self {
        self.enfants.extend(enfants);
        self
    }

    /// Cherche un nœud par libellé, en profondeur.
    pub fn trouver(&self, libelle: &str) -> Option<&Trace> {
        if self.libelle == libelle {
            return Some(self);
        }
        self.enfants.iter().find_map(|e| e.trouver(libelle))
    }

    fn ecrire(&self, f: &mut fmt::Formatter<'_>, prefixe: &str) -> fmt::Result {
        write!(f, "{prefixe}{} = {}", self.libelle, nombre(self.valeur))?;
        if let Some(formule) = &self.formule {
            write!(f, "    [{formule}]")?;
        }
        writeln!(f)?;
        let sous = format!("{prefixe}  ");
        for c in &self.contributions {
            writeln!(
                f,
                "{sous}· {} {} {} ({})",
                c.kind.libelle(),
                nombre(c.valeur),
                c.stat,
                c.source
            )?;
        }
        for e in &self.enfants {
            e.ecrire(f, &sous)?;
        }
        Ok(())
    }
}

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.ecrire(f, "")
    }
}

/// Affichage compact : pas de zéros inutiles, 4 décimales au plus.
pub fn nombre(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}
