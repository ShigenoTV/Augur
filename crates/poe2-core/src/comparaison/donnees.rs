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

use crate::calc::stats::TypeDegats;
use crate::calc::{Infusion, TypeModificateur};

#[derive(Debug, Clone, Deserialize)]
pub struct NiveauGemme {
    /// Dégâts de base min et max par type (« foudre », « feu »…) ; vide pour
    /// une attaque, dont les dégâts viennent de l'arme.
    #[serde(default)]
    pub degats: BTreeMap<String, [f64; 2]>,
    /// Attaque : dégâts de l'arme, en % (« Attack Damage: 250% of base »).
    #[serde(default)]
    pub multiplicateur: Option<f64>,
    #[serde(default)]
    pub chaines: Option<f64>,
    /// Dégâts dans le temps de base, par seconde et par type.
    #[serde(default)]
    pub dot: BTreeMap<String, f64>,
    /// Effets de la gemme sur les ailments à dégâts : (stat du moteur,
    /// nature, valeur), par exemple `["ignite_chance", "more", 200]`.
    #[serde(default)]
    pub ailments: Vec<(String, TypeModificateur, f64)>,
}

impl NiveauGemme {
    pub fn degats(&self) -> BTreeMap<TypeDegats, [f64; 2]> {
        self.degats
            .iter()
            .filter_map(|(k, v)| TypeDegats::depuis_nom(k).map(|t| (t, *v)))
            .collect()
    }

    pub fn dot(&self) -> BTreeMap<TypeDegats, f64> {
        self.dot
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
    /// Archétype vérifié en jeu ; sinon, formule générique.
    #[serde(default)]
    pub verifiee: bool,
    /// Sort déclenché (Hailstorm, Coursing Current…) : `temps_incantation`
    /// est son temps de recharge, ou 1 s s'il n'en a pas.
    #[serde(default)]
    pub declenche: bool,
    /// Les mods de dégâts des sorts s'appliquent à ses dégâts dans le temps.
    #[serde(default)]
    pub dot_sort: bool,
    /// « sort » (défaut), « attaque », « minion_attaque » ou « minion_sort ».
    /// Pour les minions, les dégâts de chaque niveau sont ceux d'un monstre
    /// allié de niveau 2 × niveau de la gemme : un indice, juste en %.
    #[serde(default)]
    pub genre: Option<String>,
    /// Attaque : vitesse de la gemme en % de celle de l'arme (« Attack Speed: 90% of base »).
    #[serde(default = "cent")]
    pub vitesse_attaque: f64,
    /// Conversion propre à la compétence, en % : `{ physique: { foudre: 80 } }`.
    #[serde(default)]
    pub conversion: BTreeMap<String, BTreeMap<String, f64>>,
}

fn cent() -> f64 {
    100.0
}

/// Base d'arme : dégâts physiques, temps d'attaque (s), critique (%).
#[derive(Debug, Clone, Deserialize)]
pub struct BaseArme {
    pub classe: String,
    pub physique: [f64; 2],
    pub temps_attaque: f64,
    pub critique: f64,
}

impl GemmeActive {
    pub fn est_attaque(&self) -> bool {
        self.genre.as_deref() == Some("attaque")
    }

    pub fn est_minion(&self) -> bool {
        matches!(
            self.genre.as_deref(),
            Some("minion_attaque" | "minion_sort")
        )
    }

    pub fn conversion(&self) -> BTreeMap<TypeDegats, BTreeMap<TypeDegats, f64>> {
        self.conversion
            .iter()
            .filter_map(|(src, c)| {
                let src = TypeDegats::depuis_nom(src)?;
                let c = c
                    .iter()
                    .filter_map(|(t, v)| TypeDegats::depuis_nom(t).map(|t| (t, *v)))
                    .collect();
                Some((src, c))
            })
            .collect()
    }

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
    #[serde(default)]
    pub armes: BTreeMap<String, BaseArme>,
}

impl Gemmes {
    /// Base d'arme d'après le nom de base (un objet magique l'entoure de ses
    /// affixes : « Heavy Gemini Bow of Skill »). Le nom le plus long gagne.
    pub fn base_arme(&self, nom: &str) -> Option<(&str, &BaseArme)> {
        self.armes
            .iter()
            .filter(|(b, _)| nom.contains(b.as_str()))
            .max_by_key(|(b, _)| b.len())
            .map(|(b, a)| (b.as_str(), a))
    }
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
        let la = &gemmes().actives["LightningArrowPlayer"];
        assert!(la.est_attaque());
        assert_eq!(la.vitesse_attaque, 90.0);
        assert_eq!(la.niveau(20).unwrap().1.multiplicateur, Some(250.0));
        assert_eq!(
            la.conversion()[&TypeDegats::Physique][&TypeDegats::Foudre],
            80.0
        );
        let (nom, arc) = gemmes().base_arme("Heavy Gemini Bow of Skill").unwrap();
        assert_eq!((nom, arc.classe.as_str()), ("Gemini Bow", "Bows"));
        assert_eq!(lb.niveau(99).unwrap().0, 40);
        assert_eq!(lb.niveau(0).unwrap().0, 1);
        let arc = &gemmes().actives["ArcPlayer"];
        assert_eq!(arc.chaines_par_qualite, 0.1);
        assert!(arc.infusion.is_some());
        assert!(gemmes().supports.contains_key("SupportZenithPlayerTwo"));
        assert!(arbre().passifs.len() > 4000);
    }
}
