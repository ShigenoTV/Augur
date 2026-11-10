//! Ailments à dégâts : ignite, poison, saignement (confiance `supposé`).
//!
//! Règles du jeu (poe2db, « Ailments », consulté le 10/11/2026) :
//!
//! - **ignite** : feu, 20 % des dégâts de feu du coup par seconde, 4 s ;
//!   chance donnée par la Flammability, 1 % par 5 % du seuil d'ailment de la
//!   cible infligé en dégâts de feu ;
//! - **poison** : chaos, 20 % des dégâts physiques et de chaos du coup (avant
//!   mitigation) par seconde, 2 s ; aucune chance sans source explicite ;
//! - **saignement** : physique, 15 % des dégâts physiques du coup (avant
//!   mitigation) par seconde, 5 s ; aucune chance sans source explicite ;
//!   double si la cible bouge (non compté : cible immobile, comme le panneau).
//!
//! Modèle moyen de Path of Building PoE 2 (`CalcOffence.lua`, licence MIT) :
//! un seul ailment de chaque sorte actif à la fois (plus pour le poison avec
//! « +N of your Poisons »), nombre d'ailments actifs = min(chance × coups par
//! seconde × durée, maximum). Les mods de dégâts ont déjà agi sur le coup :
//! seuls la magnitude, la durée et la vitesse des ailments s'y ajoutent, puis
//! la résistance de la cible (feu pour l'ignite, chaos pour le poison ; ni
//! armure ni pénétration). Seuil d'ailment des monstres : table de Path of
//! Building (`monsterAilmentThresholdTable`, `Data/Misc.lua`), niveau de la
//! cible = niveau du personnage plafonné à 85 si rien n'est précisé.

use std::collections::BTreeMap;

use super::Cible;
use super::moddb::ModDb;
use super::modifier::TypeModificateur;
use super::stats::{self, TypeDegats};
use super::trace::{Trace, nombre};

/// Niveau maximal d'une cible par défaut (Path of Building : `MaxEnemyLevel`).
pub const NIVEAU_CIBLE_MAX: u32 = 85;

/// Seuil d'ailment d'un monstre par niveau (1 à 100), Path of Building PoE 2.
const SEUIL_MONSTRE: [f64; 100] = [
    15.0, 20.0, 24.0, 28.0, 34.0, 39.0, 46.0, 52.0, 60.0, 70.0, 81.0, 95.0, 110.0, 126.0, 144.0,
    171.0, 193.0, 218.0, 245.0, 275.0, 306.0, 340.0, 376.0, 413.0, 455.0, 497.0, 543.0, 590.0,
    641.0, 695.0, 752.0, 812.0, 874.0, 950.0, 1033.0, 1123.0, 1220.0, 1326.0, 1442.0, 1568.0,
    1705.0, 1854.0, 2015.0, 2192.0, 2384.0, 2564.0, 2757.0, 2966.0, 3188.0, 3426.0, 3681.0, 3955.0,
    4247.0, 4560.0, 4895.0, 5254.0, 5638.0, 6049.0, 6489.0, 6959.0, 7462.0, 8001.0, 8576.0, 9193.0,
    9649.0, 10228.0, 10841.0, 11492.0, 12181.0, 18272.0, 19369.0, 20531.0, 21763.0, 23068.0,
    34602.0, 36679.0, 38879.0, 41212.0, 43685.0, 65527.0, 68415.0, 71303.0, 74191.0, 77079.0,
    79967.0, 82855.0, 85743.0, 88631.0, 91519.0, 94407.0, 97295.0, 100183.0, 103071.0, 105959.0,
    108847.0, 111735.0, 114623.0, 117511.0, 120399.0, 123287.0,
];

/// Seuil d'ailment d'un monstre du niveau donné (borné à 1..100).
pub fn seuil_monstre(niveau: u32) -> f64 {
    SEUIL_MONSTRE[(niveau.clamp(1, 100) - 1) as usize]
}

/// « 1 % de chance d'ignite par 5 % du seuil infligé en feu » (`IgniteChanceMultiplier`).
const MULTIPLICATEUR_FLAMMABILITE: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ailment {
    Ignite,
    Poison,
    Saignement,
}

impl Ailment {
    pub const TOUS: [Ailment; 3] = [Ailment::Ignite, Ailment::Poison, Ailment::Saignement];

    /// Nom des sorties du moteur (`dps_ignite`, `chance_poison`…).
    pub fn nom(self) -> &'static str {
        match self {
            Ailment::Ignite => "ignite",
            Ailment::Poison => "poison",
            Ailment::Saignement => "saignement",
        }
    }

    /// Préfixe des stats (`ignite_chance`, `bleed_magnitude`…).
    fn cle(self) -> &'static str {
        match self {
            Ailment::Ignite => "ignite",
            Ailment::Poison => "poison",
            Ailment::Saignement => "bleed",
        }
    }

    /// Types de dégâts du coup qui en font la base.
    fn sources(self) -> &'static [TypeDegats] {
        match self {
            Ailment::Ignite => &[TypeDegats::Feu],
            Ailment::Poison => &[TypeDegats::Physique, TypeDegats::Chaos],
            Ailment::Saignement => &[TypeDegats::Physique],
        }
    }

    /// Type des dégâts infligés (résistance de la cible).
    fn type_degats(self) -> TypeDegats {
        match self {
            Ailment::Ignite => TypeDegats::Feu,
            Ailment::Poison => TypeDegats::Chaos,
            Ailment::Saignement => TypeDegats::Physique,
        }
    }

    /// Part des dégâts du coup infligée par seconde.
    fn part_par_seconde(self) -> f64 {
        match self {
            Ailment::Ignite | Ailment::Poison => 0.20,
            Ailment::Saignement => 0.15,
        }
    }

    /// Durée de base, en secondes.
    fn duree_base(self) -> f64 {
        match self {
            Ailment::Ignite => 4.0,
            Ailment::Poison => 2.0,
            Ailment::Saignement => 5.0,
        }
    }

    fn stat(self, suffixe: &str) -> String {
        format!("{}_{suffixe}", self.cle())
    }
}

/// Coups de la compétence, tels que le calcul offensif les a obtenus.
pub struct Coups<'a> {
    /// Dégâts moyens par type, avant mitigation et hors critique.
    pub par_type: &'a [(TypeDegats, f64)],
    /// Chance de critique, en %.
    pub chance_critique: f64,
    /// Bonus de dégâts critiques, en %.
    pub bonus_critique: f64,
    /// Coups par seconde.
    pub cadence: f64,
}

/// Résultat pour un ailment infligé.
pub struct ResultatAilment {
    pub ailment: Ailment,
    pub chance: Trace,
    pub dps: Trace,
}

/// Chance d'infliger l'ailment sur un coup dont la base vaut `source` dégâts.
fn chance_sur_coup(db: &ModDb, a: Ailment, source: f64, seuil: f64) -> f64 {
    let s = [a.stat("chance"), stats::AILMENT_CHANCE.to_string()];
    let s: Vec<&str> = s.iter().map(String::as_str).collect();
    let base_seuil = if a == Ailment::Ignite && seuil > 0.0 {
        source / seuil * MULTIPLICATEUR_FLAMMABILITE
    } else {
        0.0
    };
    let added = db.somme(TypeModificateur::Added, &s).valeur;
    let inc = (1.0 + db.somme(TypeModificateur::Increased, &s).valeur / 100.0).max(0.0);
    let more = db.produit_more(&s).valeur;
    ((base_seuil + added) * inc * more).clamp(0.0, 100.0)
}

/// Ailments à dégâts infligés par les coups de la compétence.
pub fn calculer(db: &ModDb, coups: &Coups, cible: &Cible, seuil: f64) -> Vec<ResultatAilment> {
    let par_type: BTreeMap<TypeDegats, f64> = coups.par_type.iter().copied().collect();
    let cc = coups.chance_critique.clamp(0.0, 100.0) / 100.0;
    let mult_crit = 1.0 + coups.bonus_critique / 100.0;
    let mut sortie = Vec::new();
    for a in Ailment::TOUS {
        let source: f64 = a
            .sources()
            .iter()
            .map(|t| par_type.get(t).copied().unwrap_or(0.0))
            .sum();
        if source <= 0.0 {
            continue;
        }
        // Chance et base pondérées entre coups normaux et critiques.
        let ch_coup = chance_sur_coup(db, a, source, seuil);
        let ch_crit = chance_sur_coup(db, a, source * mult_crit, seuil);
        let de_coup = ch_coup * (1.0 - cc);
        let de_crit = ch_crit * cc;
        let chance = de_coup + de_crit;
        if chance <= 0.0 {
            continue;
        }
        let base = (source * de_coup + source * mult_crit * de_crit) / chance;
        let t_chance = Trace::feuille(format!("chance de {} par coup", a.nom()), chance)
            .avec_formule(if a == Ailment::Ignite {
                format!(
                    "Flammability : {} dégâts de feu contre un seuil de {} (1 % par 5 %), puis chance ajoutée × increased × more ; {} % sur coup normal, {} % sur critique",
                    nombre(source),
                    nombre(seuil),
                    nombre(ch_coup),
                    nombre(ch_crit)
                )
            } else {
                format!(
                    "chance ajoutée × increased × more ; {} % sur coup normal, {} % sur critique",
                    nombre(ch_coup),
                    nombre(ch_crit)
                )
            });

        let s_mag = [a.stat("magnitude"), stats::AILMENT_MAGNITUDE.to_string()];
        let s_mag: Vec<&str> = s_mag.iter().map(String::as_str).collect();
        let t_mag_inc = db.trace_increased("magnitude (increased)", &s_mag);
        let t_mag_more = db.trace_more("magnitude (more)", &s_mag);
        let s_vit = [a.stat("faster")];
        let vitesse = (1.0
            + db.somme(TypeModificateur::Increased, &[s_vit[0].as_str()])
                .valeur
                / 100.0)
            .max(0.01);
        let s_duree = [a.stat("duration"), stats::AILMENT_DURATION.to_string()];
        let s_duree: Vec<&str> = s_duree.iter().map(String::as_str).collect();
        let duree = a.duree_base()
            * db.trace_increased("durée", &s_duree).valeur
            * db.trace_more("durée", &s_duree).valeur
            / vitesse;
        let maximum = if a == Ailment::Poison {
            (1.0 + db
                .somme(TypeModificateur::Added, &[stats::POISON_STACKS])
                .valeur)
                .max(1.0)
        } else {
            1.0
        };
        let infliges = chance / 100.0 * coups.cadence * duree;
        let actifs = infliges.min(maximum);
        let res = if a.type_degats() == TypeDegats::Physique {
            0.0
        } else {
            cible
                .resistances
                .get(&a.type_degats())
                .copied()
                .unwrap_or(0.0)
        };
        let par_instance =
            base * a.part_par_seconde() * t_mag_inc.valeur * t_mag_more.valeur * vitesse;
        let dps = par_instance * actifs * (1.0 - res / 100.0);
        let t_actifs = Trace::feuille(format!("{} actifs en moyenne", a.nom()), actifs)
            .avec_formule(format!(
                "min({} % × {} coups/s × {} s, maximum {})",
                nombre(chance),
                nombre(coups.cadence),
                nombre(duree),
                nombre(maximum)
            ));
        let t_dps = Trace::feuille(format!("DPS de {}", a.nom()), dps)
            .avec_formule(format!(
                "{} (base du coup, critiques compris) × {} % × {} × {} × vitesse {} × {} actifs × (1 − {} %)",
                nombre(base),
                nombre(a.part_par_seconde() * 100.0),
                nombre(t_mag_inc.valeur),
                nombre(t_mag_more.valeur),
                nombre(vitesse),
                nombre(actifs),
                nombre(res)
            ))
            .avec_enfants([t_chance.clone(), t_mag_inc, t_mag_more, t_actifs]);
        sortie.push(ResultatAilment {
            ailment: a,
            chance: t_chance,
            dps: t_dps,
        });
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::Modificateur;

    fn m(stat: &str, k: TypeModificateur, v: f64) -> Modificateur {
        Modificateur::new(stat, k, v, "test")
    }

    fn coups(par_type: &[(TypeDegats, f64)], cadence: f64) -> Coups<'_> {
        Coups {
            par_type,
            chance_critique: 0.0,
            bonus_critique: 100.0,
            cadence,
        }
    }

    #[test]
    fn seuil_par_niveau() {
        assert_eq!(seuil_monstre(1), 15.0);
        assert_eq!(seuil_monstre(85), 79967.0);
        assert_eq!(seuil_monstre(0), 15.0);
        assert_eq!(seuil_monstre(200), 123287.0);
    }

    #[test]
    fn ignite_par_la_flammabilite() {
        // 1 000 de feu contre un seuil de 10 000 : 10 % du seuil, 1 % par
        // 5 %, soit 2 %.
        let db = ModDb::new([]);
        let t = [(TypeDegats::Feu, 1000.0)];
        let r = calculer(&db, &coups(&t, 2.0), &Cible::default(), 10_000.0);
        assert_eq!(r.len(), 1);
        assert!((r[0].chance.valeur - 2.0).abs() < 1e-9);
        // 2 % × 2 coups/s × 4 s = 0,16 ignite actif ; 1000 × 20 % × 0,16 = 32.
        assert!((r[0].dps.valeur - 32.0).abs() < 1e-9);
    }

    #[test]
    fn un_seul_ignite_actif_et_resistance() {
        let db = ModDb::new([m("ignite_chance", TypeModificateur::Added, 100.0)]);
        let t = [(TypeDegats::Feu, 1000.0)];
        let mut cible = Cible::default();
        cible.resistances.insert(TypeDegats::Feu, 50.0);
        let r = calculer(&db, &coups(&t, 2.0), &cible, 1e12);
        // 1000 × 20 % × 1 actif × 50 %.
        assert!((r[0].dps.valeur - 100.0).abs() < 1e-9);
    }

    #[test]
    fn poison_sans_chance_rien() {
        let db = ModDb::new([]);
        let t = [(TypeDegats::Physique, 1000.0)];
        assert!(calculer(&db, &coups(&t, 2.0), &Cible::default(), 1e12).is_empty());
    }

    #[test]
    fn poison_empile_avec_les_mods() {
        let db = ModDb::new([
            m("poison_chance", TypeModificateur::Added, 50.0),
            m("poison_stacks", TypeModificateur::Added, 2.0),
            m("ailment_magnitude", TypeModificateur::Increased, 50.0),
        ]);
        let t = [(TypeDegats::Physique, 600.0), (TypeDegats::Chaos, 400.0)];
        let r = calculer(&db, &coups(&t, 4.0), &Cible::default(), 1e12);
        // Poison seulement (pas de feu, pas de chance de saignement).
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].ailment, Ailment::Poison);
        // 50 % × 4 × 2 s = 4 infligés, 3 au plus ; 1000 × 20 % × 1,5 × 3 = 900.
        assert!((r[0].dps.valeur - 900.0).abs() < 1e-9);
    }

    #[test]
    fn saignement_et_critiques() {
        let db = ModDb::new([m("bleed_chance", TypeModificateur::Added, 100.0)]);
        let t = [(TypeDegats::Physique, 1000.0)];
        let c = Coups {
            par_type: &t,
            chance_critique: 50.0,
            bonus_critique: 100.0,
            cadence: 1.0,
        };
        let r = calculer(&db, &c, &Cible::default(), 1e12);
        // Base moyenne 1500 (moitié de critiques ×2) × 15 % × 1 actif.
        assert!((r[0].dps.valeur - 225.0).abs() < 1e-9);
    }

    #[test]
    fn plus_rapide_et_plus_court() {
        let db = ModDb::new([
            m("ignite_chance", TypeModificateur::Added, 100.0),
            m("ignite_faster", TypeModificateur::Increased, 25.0),
        ]);
        let t = [(TypeDegats::Feu, 1000.0)];
        // Un coup toutes les 10 s : 1 × 0,1 × 4 / 1,25 = 0,32 actif.
        let r = calculer(&db, &coups(&t, 0.1), &Cible::default(), 1e12);
        assert!((r[0].dps.valeur - 1000.0 * 0.2 * 1.25 * 0.32).abs() < 1e-9);
    }
}
