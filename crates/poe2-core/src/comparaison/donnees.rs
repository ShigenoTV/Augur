//! Données de jeu embarquées pour la comparaison d'objets, produites par
//! `tools/gamedata/extract_build_data.py` :
//!
//! - `data/gemmes.json` : chaque sort à dégâts par niveau (RePoE ; Lightning
//!   Bolt et Arc vérifiés en jeu, les autres en formule générique), et le
//!   texte des effets de chaque support ;
//! - `data/arbre.json` : texte des stats de chaque passif (arbre de Path of
//!   Building PoE 2, licence MIT, données d'origine GGG).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::calc::Infusion;
use crate::calc::stats::TypeDegats;

#[derive(Debug, Clone, Deserialize)]
pub struct NiveauGemme {
    /// Dégâts de base min et max par type (« foudre », « feu »…).
    pub degats: BTreeMap<String, [f64; 2]>,
    #[serde(default)]
    pub chaines: Option<f64>,
}

impl NiveauGemme {
    pub fn degats(&self) -> BTreeMap<TypeDegats, [f64; 2]> {
        self.degats
            .iter()
            .filter_map(|(k, v)| TypeDegats::depuis_nom(k).map(|t| (t, *v)))
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GemmeActive {
    pub nom: String,
    pub temps_incantation: f64,
    pub critique_base: f64,
    pub niveaux: BTreeMap<String, NiveauGemme>,
    #[serde(default)]
    pub chaines_par_qualite: f64,
    #[serde(default)]
    pub infusion: Option<Infusion>,
    /// Tags d'élément du sort, pour les bonus de niveaux (« Fire Spell Skills »).
    #[serde(default)]
    pub elements: Vec<String>,
    /// Archétype vérifié en jeu ; sinon, formule générique d'un sort à impact.
    #[serde(default)]
    pub verifiee: bool,
}

impl GemmeActive {
    pub fn elements(&self) -> Vec<TypeDegats> {
        self.elements
            .iter()
            .filter_map(|e| TypeDegats::depuis_nom(e))
            .collect()
    }

    /// Niveau borné aux niveaux connus (1 à 40 dans RePoE).
    pub fn niveau(&self, n: i32) -> Option<(u32, &NiveauGemme)> {
        let max = self
            .niveaux
            .keys()
            .filter_map(|k| k.parse::<u32>().ok())
            .max()?;
        let n = (n.max(1) as u32).min(max);
        self.niveaux.get(&n.to_string()).map(|d| (n, d))
    }
}

#[derive(Debug, Deserialize)]
pub struct Gemmes {
    pub actives: BTreeMap<String, GemmeActive>,
    pub supports: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct Arbre {
    pub version: String,
    pub passifs: BTreeMap<String, Vec<String>>,
}

pub fn gemmes() -> &'static Gemmes {
    static G: OnceLock<Gemmes> = OnceLock::new();
    G.get_or_init(|| {
        serde_json::from_str(include_str!("../../data/gemmes.json"))
            .expect("data/gemmes.json invalide (vérifié par les tests)")
    })
}

pub fn arbre() -> &'static Arbre {
    static A: OnceLock<Arbre> = OnceLock::new();
    A.get_or_init(|| {
        serde_json::from_str(include_str!("../../data/arbre.json"))
            .expect("data/arbre.json invalide (vérifié par les tests)")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn donnees_lisibles() {
        let lb = &gemmes().actives["LightningBoltPlayer"];
        let (n, d) = lb.niveau(20).unwrap();
        assert_eq!((n, d.degats["foudre"]), (20, [30.0, 565.0]));
        assert!(lb.verifiee);
        let fb = &gemmes().actives["FireballPlayer"];
        assert!(!fb.verifiee);
        assert_eq!(fb.elements(), vec![TypeDegats::Feu]);
        assert!(
            fb.niveau(20)
                .unwrap()
                .1
                .degats()
                .contains_key(&TypeDegats::Feu)
        );
        assert!(gemmes().actives.len() > 50);
        assert_eq!(lb.niveau(99).unwrap().0, 40);
        assert_eq!(lb.niveau(0).unwrap().0, 1);
        let arc = &gemmes().actives["ArcPlayer"];
        assert_eq!(arc.chaines_par_qualite, 0.1);
        assert!(arc.infusion.is_some());
        assert!(gemmes().supports.contains_key("SupportZenithPlayerTwo"));
        assert!(arbre().passifs.len() > 4000);
    }
}
