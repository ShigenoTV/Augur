//! Arme d'une attaque : dégâts, temps d'attaque et critique de la base
//! (RePoE `base_items.json`), modifiés par les mods locaux de l'arme et sa
//! qualité.
//!
//! Sur une arme, ces lignes sont locales (elles ne valent que pour l'arme) :
//! « Adds N to M <Type> Damage » (sans « to Attacks » ni « to Spells »),
//! « N% increased Physical Damage », « N% increased Attack Speed » et
//! « +N% to Critical Hit Chance ». La qualité ajoute ses points aux dégâts
//! physiques augmentés. Ces lignes sont retirées des mods globaux.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

use super::donnees::BaseArme;
use super::traduction::nettoyer;
use crate::calc::stats::TypeDegats;

/// Un mod local d'arme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Local {
    Ajoutes(TypeDegats, f64, f64),
    PhysiqueAugmente(f64),
    VitesseAttaque(f64),
    Critique(f64),
}

struct Motifs {
    ajoutes: Regex,
    physique: Regex,
    vitesse: Regex,
    critique: Regex,
    qualite: Regex,
}

fn motifs() -> &'static Motifs {
    static M: OnceLock<Motifs> = OnceLock::new();
    M.get_or_init(|| {
        let n = r"(\d+(?:\.\d+)?)";
        let r = |s: String| Regex::new(&s).unwrap();
        Motifs {
            ajoutes: r(format!(
                r"^Adds {n} to {n} (Physical|Fire|Cold|Lightning|Chaos) Damage$"
            )),
            physique: r(format!(r"^{n}% (increased|reduced) Physical Damage$")),
            vitesse: r(format!(r"^{n}% (increased|reduced) Attack Speed$")),
            critique: r(format!(r"^\+{n}% to Critical Hit Chance$")),
            qualite: r(r"^Quality: \+?(\d+)".to_string()),
        }
    })
}

fn type_degats(mot: &str) -> Option<TypeDegats> {
    TypeDegats::TOUS
        .into_iter()
        .find(|t| t.cle().eq_ignore_ascii_case(mot))
}

fn signe(mot: &str) -> f64 {
    if mot == "reduced" { -1.0 } else { 1.0 }
}

/// Le mod local que porte cette ligne d'arme, s'il y en a un.
pub fn local(ligne: &str) -> Option<Local> {
    let l = nettoyer(ligne);
    let m = motifs();
    let v = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    if let Some(c) = m.ajoutes.captures(&l) {
        return Some(Local::Ajoutes(type_degats(&c[3])?, v(&c[1]), v(&c[2])));
    }
    if let Some(c) = m.physique.captures(&l) {
        return Some(Local::PhysiqueAugmente(v(&c[1]) * signe(&c[2])));
    }
    if let Some(c) = m.vitesse.captures(&l) {
        return Some(Local::VitesseAttaque(v(&c[1]) * signe(&c[2])));
    }
    if let Some(c) = m.critique.captures(&l) {
        return Some(Local::Critique(v(&c[1])));
    }
    None
}

/// Qualité lue dans le texte PoB d'un objet (« Quality: 20 »).
pub fn qualite(texte: &str) -> f64 {
    texte
        .lines()
        .find_map(|l| motifs().qualite.captures(l.trim()))
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0.0)
}

/// Arme après ses mods locaux.
#[derive(Debug, Clone, PartialEq)]
pub struct Arme {
    pub base: String,
    /// Dégâts par type, min et max.
    pub degats: BTreeMap<TypeDegats, [f64; 2]>,
    /// Temps d'une attaque, en secondes.
    pub temps_attaque: f64,
    /// Chance de critique, en %.
    pub critique: f64,
}

pub fn construire<'a>(
    nom_base: &str,
    base: &BaseArme,
    qualite: f64,
    lignes: impl IntoIterator<Item = &'a str>,
) -> Arme {
    let mut ajoutes: BTreeMap<TypeDegats, [f64; 2]> = BTreeMap::new();
    let mut inc_phys = qualite;
    let mut vitesse = 0.0;
    let mut critique = base.critique;
    for l in lignes {
        match local(l) {
            Some(Local::Ajoutes(t, min, max)) => {
                let e = ajoutes.entry(t).or_insert([0.0; 2]);
                e[0] += min;
                e[1] += max;
            }
            Some(Local::PhysiqueAugmente(v)) => inc_phys += v,
            Some(Local::VitesseAttaque(v)) => vitesse += v,
            Some(Local::Critique(v)) => critique += v,
            None => {}
        }
    }
    let mut degats = BTreeMap::new();
    let ph = ajoutes.remove(&TypeDegats::Physique).unwrap_or([0.0; 2]);
    let mult = (1.0 + inc_phys / 100.0).max(0.0);
    let phys = [
        ((base.physique[0] + ph[0]) * mult).round(),
        ((base.physique[1] + ph[1]) * mult).round(),
    ];
    if phys[1] > 0.0 {
        degats.insert(TypeDegats::Physique, phys);
    }
    degats.extend(ajoutes);
    Arme {
        base: nom_base.to_string(),
        degats,
        temps_attaque: base.temps_attaque / (1.0 + vitesse / 100.0).max(0.01),
        critique,
    }
}

/// Deux armes tenues ensemble : les attaques alternent entre les mains
/// (supposé), soit la moyenne des dégâts, du temps d'attaque et du critique.
pub fn double(a: &Arme, b: &Arme) -> Arme {
    let mut degats: BTreeMap<TypeDegats, [f64; 2]> = BTreeMap::new();
    for arme in [a, b] {
        for (t, [min, max]) in &arme.degats {
            let e = degats.entry(*t).or_insert([0.0; 2]);
            e[0] += min / 2.0;
            e[1] += max / 2.0;
        }
    }
    Arme {
        base: format!("{} + {}", a.base, b.base),
        degats,
        temps_attaque: (a.temps_attaque + b.temps_attaque) / 2.0,
        critique: (a.critique + b.critique) / 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arc() -> BaseArme {
        BaseArme {
            classe: "Bows".into(),
            physique: [39.0, 72.0],
            temps_attaque: 0.87,
            critique: 5.0,
        }
    }

    #[test]
    fn mods_locaux_et_qualite() {
        let lignes = [
            "Adds 10 to 20 Physical Damage",
            "Adds 5 to 50 Lightning Damage",
            "80% increased Physical Damage",
            "15% increased Attack Speed",
            "+2.5% to Critical Hit Chance",
            "Adds 3 to 6 Fire Damage to Attacks",
        ];
        let a = construire("Gemini Bow", &arc(), 20.0, lignes);
        // (39 + 10) × 2 = 98 ; (72 + 20) × 2 = 184.
        assert_eq!(a.degats[&TypeDegats::Physique], [98.0, 184.0]);
        assert_eq!(a.degats[&TypeDegats::Foudre], [5.0, 50.0]);
        assert!(!a.degats.contains_key(&TypeDegats::Feu));
        assert!((a.temps_attaque - 0.87 / 1.15).abs() < 1e-9);
        assert_eq!(a.critique, 7.5);
    }

    #[test]
    fn deux_armes_alternent() {
        let a = construire("Gemini Bow", &arc(), 0.0, []);
        let mut b = a.clone();
        b.degats.insert(TypeDegats::Feu, [10.0, 20.0]);
        b.temps_attaque = 0.5;
        b.critique = 7.0;
        let d = double(&a, &b);
        assert_eq!(d.degats[&TypeDegats::Physique], [39.0, 72.0]);
        assert_eq!(d.degats[&TypeDegats::Feu], [5.0, 10.0]);
        assert!((d.temps_attaque - (0.87 + 0.5) / 2.0).abs() < 1e-9);
        assert_eq!(d.critique, 6.0);
    }

    #[test]
    fn global_n_est_pas_local() {
        assert_eq!(local("Adds 3 to 6 Fire Damage to Attacks"), None);
        assert_eq!(local("Adds 3 to 6 Fire Damage to Spells"), None);
        assert_eq!(local("20% increased Critical Hit Chance"), None);
        assert_eq!(qualite("Rarity: RARE\nX\nGemini Bow\nQuality: 20\n"), 20.0);
        assert_eq!(qualite("Quality: +17% (augmented)"), 17.0);
    }
}
