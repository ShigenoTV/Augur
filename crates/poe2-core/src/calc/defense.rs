//! Calcul défensif : réserves, résistances, armure et coup maximal encaissable.
//!
//! Formules (confiance `supposé`, à confirmer par les fiches du Theorycrafter) :
//!
//! - résistance = min(Σ résistances, résistance maximale) ; maximale = 75 % +
//!   bonus, plafond absolu 90 % ;
//! - réduction de l'armure contre un coup physique `D` :
//!   `A / (A + 10 × D)` + réduction additionnelle, plafonnée à 90 % (fiche 05) ;
//! - dégâts subis : × max(0, 1 + Σ increased) × Π more (`damage_taken`) ;
//! - pénalité de campagne : paramètre explicite du personnage, ajouté aux
//!   résistances élémentaires avant plafonnement (−60 % en fin de campagne,
//!   0 pour le chaos ; inconnue I-02 du Theorycrafter) ;
//! - coup maximal encaissable (« EHP par coup ») : plus gros coup d'un type
//!   qui ne vide pas la réserve. Réserve = vie + bouclier d'énergie, sauf
//!   contre le chaos qui retire le bouclier d'énergie au double :
//!   vie + ES / 2 (fiche 07, inconnue I-01).

use super::moddb::ModDb;
use super::modifier::TypeModificateur;
use super::stats::{self, TypeDegats};
use super::trace::{Trace, nombre};

pub const RESISTANCE_MAX_BASE: f64 = 75.0;
pub const RESISTANCE_MAX_ABSOLUE: f64 = 90.0;
pub const FACTEUR_ARMURE: f64 = 10.0;
pub const REDUCTION_ARMURE_MAX: f64 = 0.9;

/// Réduction (0 à 0,9) qu'apporte `armure` contre un coup physique `degats`.
pub fn reduction_armure(armure: f64, degats: f64) -> f64 {
    reduction_physique(armure, degats, 0.0)
}

/// Réduction physique totale (0 à 0,9) : armure + `additionnelle_pct` points.
pub fn reduction_physique(armure: f64, degats: f64, additionnelle_pct: f64) -> f64 {
    let base = if armure > 0.0 && degats > 0.0 {
        armure / (armure + FACTEUR_ARMURE * degats)
    } else {
        0.0
    };
    (base + additionnelle_pct / 100.0).clamp(0.0, REDUCTION_ARMURE_MAX)
}

/// Plus gros coup physique encaissable avec `reserve` points et `armure`.
pub fn coup_physique_max(reserve: f64, armure: f64) -> f64 {
    coup_physique_max_complet(reserve, armure, 0.0, 1.0)
}

/// Plus gros coup physique encaissable, avec réduction additionnelle `r_pct`
/// et multiplicateur de dégâts subis `m`.
///
/// Les dégâts subis `m × D × (1 − réduction(D))` croissent avec `D`. Hors
/// plafond, on résout `10m(1−r) D² − (m r A + 10 R) D − R A = 0` ; au plafond,
/// `D = R / (0,1 m)`.
pub fn coup_physique_max_complet(reserve: f64, armure: f64, r_pct: f64, m: f64) -> f64 {
    if reserve <= 0.0 {
        return 0.0;
    }
    if m <= 0.0 {
        return f64::INFINITY;
    }
    let plafonne = reserve / ((1.0 - REDUCTION_ARMURE_MAX) * m);
    if reduction_physique(armure, plafonne, r_pct) >= REDUCTION_ARMURE_MAX {
        return plafonne;
    }
    let a = armure.max(0.0);
    let r = (r_pct / 100.0).max(0.0);
    let k = FACTEUR_ARMURE;
    let qa = k * m * (1.0 - r);
    let qb = m * r * a + k * reserve;
    let qc = reserve * a;
    (qb + (qb * qb + 4.0 * qa * qc).sqrt()) / (2.0 * qa)
}

pub struct Defense {
    pub vie: Trace,
    pub bouclier_energie: Trace,
    pub armure: Trace,
    pub resistances: Vec<(TypeDegats, Trace)>,
    pub reduction_armure: Option<Trace>,
    pub coups_max: Vec<(TypeDegats, Trace)>,
}

fn resistance(db: &ModDb, t: TypeDegats, penalite_campagne: f64) -> Trace {
    let sr = t.stat_resistance();
    let mut s = vec![sr.as_str()];
    if t.est_elementaire() {
        s.push(stats::ELEMENTAL_RESISTANCE);
    }
    let brute = db.somme(TypeModificateur::Added, &s);
    let penalite = if t.est_elementaire() {
        penalite_campagne
    } else {
        0.0
    };
    let total = brute.valeur + penalite;
    let smax = t.stat_resistance_max();
    let mut s_max = vec![smax.as_str()];
    if t.est_elementaire() {
        s_max.push(stats::ALL_MAXIMUM_ELEMENTAL_RESISTANCE);
    }
    let bonus_max = db.somme(TypeModificateur::Added, &s_max);
    let max = (RESISTANCE_MAX_BASE + bonus_max.valeur).min(RESISTANCE_MAX_ABSOLUE);
    let valeur = total.min(max);

    let mut t_brute = Trace::feuille("résistance non plafonnée", total).avec_formule(format!(
        "Σ {} + pénalité de campagne {}",
        nombre(brute.valeur),
        nombre(penalite)
    ));
    t_brute.contributions = brute.contributions;
    let mut t_max = Trace::feuille("résistance maximale", max).avec_formule(format!(
        "min({}, {} + {})",
        nombre(RESISTANCE_MAX_ABSOLUE),
        nombre(RESISTANCE_MAX_BASE),
        nombre(bonus_max.valeur)
    ));
    t_max.contributions = bonus_max.contributions;
    Trace::feuille(format!("résistance {}", t.nom()), valeur)
        .avec_formule(format!("min({}, {})", nombre(total), nombre(max)))
        .avec_enfants([t_brute, t_max])
}

pub fn calculer(db: &ModDb, coup_subi: Option<f64>, penalite_campagne: f64) -> Defense {
    let vie = db.evaluer("vie", &[stats::LIFE], None);
    let bouclier_energie = db.evaluer("bouclier d'énergie", &[stats::ENERGY_SHIELD], None);
    let armure = db.evaluer("armure", &[stats::ARMOUR], None);
    let reserve = vie.valeur.max(0.0) + bouclier_energie.valeur.max(0.0);
    let t_reserve = Trace::feuille("réserve (vie + bouclier d'énergie)", reserve)
        .avec_enfants([vie.clone(), bouclier_energie.clone()]);
    let reserve_chaos = vie.valeur.max(0.0) + bouclier_energie.valeur.max(0.0) / 2.0;
    let t_reserve_chaos = Trace::feuille("réserve contre le chaos (vie + ES / 2)", reserve_chaos)
        .avec_formule("le chaos retire le bouclier d'énergie au double")
        .avec_enfants([vie.clone(), bouclier_energie.clone()]);

    let resistances: Vec<_> = TypeDegats::TOUS
        .into_iter()
        .filter(|t| *t != TypeDegats::Physique)
        .map(|t| (t, resistance(db, t, penalite_campagne)))
        .collect();

    let add = db.somme(TypeModificateur::Added, &[stats::PHYSICAL_DAMAGE_REDUCTION]);
    let mut t_add = Trace::feuille("réduction physique additionnelle", add.valeur);
    t_add.contributions = add.contributions;

    let t_inc_subi = db.trace_increased("increased", &[stats::DAMAGE_TAKEN]);
    let t_more_subi = db.trace_more("more", &[stats::DAMAGE_TAKEN]);
    let m = t_inc_subi.valeur * t_more_subi.valeur;
    let t_subi = Trace::feuille("multiplicateur de dégâts subis", m)
        .avec_formule(format!(
            "{} × {}",
            nombre(t_inc_subi.valeur),
            nombre(t_more_subi.valeur)
        ))
        .avec_enfants([t_inc_subi, t_more_subi]);

    let reduction = coup_subi.map(|d| {
        let r = reduction_physique(armure.valeur, d, add.valeur);
        Trace::feuille("réduction de l'armure", r * 100.0)
            .avec_formule(format!(
                "min(90, {} / ({} + {} × {}) + {}%)",
                nombre(armure.valeur),
                nombre(armure.valeur),
                nombre(FACTEUR_ARMURE),
                nombre(d),
                nombre(add.valeur)
            ))
            .avec_enfants([armure.clone(), t_add.clone()])
    });

    let mut coups_max = vec![(
        TypeDegats::Physique,
        Trace::feuille(
            "coup physique maximal",
            coup_physique_max_complet(reserve, armure.valeur, add.valeur, m),
        )
        .avec_formule("plus grand D tel que m × D × (1 − réduction(D)) ≤ réserve")
        .avec_enfants([t_reserve.clone(), armure.clone(), t_add, t_subi.clone()]),
    )];
    for (t, res) in &resistances {
        let (reserve, t_reserve) = if *t == TypeDegats::Chaos {
            (reserve_chaos, &t_reserve_chaos)
        } else {
            (reserve, &t_reserve)
        };
        let mult = (1.0 - res.valeur / 100.0) * m;
        let valeur = if mult > 0.0 {
            reserve / mult
        } else {
            f64::INFINITY
        };
        coups_max.push((
            *t,
            Trace::feuille(format!("coup {} maximal", t.nom()), valeur)
                .avec_formule(format!(
                    "{} / ((1 − {}%) × {})",
                    nombre(reserve),
                    nombre(res.valeur),
                    nombre(m)
                ))
                .avec_enfants([t_reserve.clone(), res.clone(), t_subi.clone()]),
        ));
    }

    Defense {
        vie,
        bouclier_energie,
        armure,
        resistances,
        reduction_armure: reduction,
        coups_max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armure_plafonnee() {
        assert_eq!(reduction_armure(1e9, 1.0), REDUCTION_ARMURE_MAX);
        assert_eq!(reduction_armure(0.0, 100.0), 0.0);
        assert!((reduction_armure(1000.0, 100.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn coup_max_complet_coherent() {
        for (r, a, add, m) in [
            (4000.0, 10000.0, 0.0, 0.9),
            (4000.0, 10000.0, 10.0, 1.2),
            (1000.0, 500.0, 50.0, 1.0),
        ] {
            let d = coup_physique_max_complet(r, a, add, m);
            let subi = m * d * (1.0 - reduction_physique(a, d, add));
            assert!(
                (subi - r).abs() < 1e-6 * r,
                "r={r} a={a} add={add} m={m} d={d} subi={subi}"
            );
        }
    }

    #[test]
    fn coup_max_coherent() {
        for (r, a) in [
            (1000.0, 0.0),
            (1000.0, 5000.0),
            (3000.0, 20000.0),
            (500.0, 1e7),
        ] {
            let d = coup_physique_max(r, a);
            let subi = d * (1.0 - reduction_armure(a, d));
            assert!((subi - r).abs() < 1e-6 * r, "r={r} a={a} d={d} subi={subi}");
        }
    }
}
