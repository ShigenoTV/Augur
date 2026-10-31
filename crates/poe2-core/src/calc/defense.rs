//! Calcul défensif : réserves, résistances, armure et coup maximal encaissable.
//!
//! Formules (confiance `supposé`, à confirmer par les fiches du Theorycrafter) :
//!
//! - résistance = min(Σ résistances, résistance maximale) ; maximale = 75 % +
//!   bonus, plafond absolu 90 % ;
//! - réduction de l'armure contre un coup physique `D` :
//!   `A / (A + 10 × D)`, plafonnée à 90 % ;
//! - coup maximal encaissable (« EHP par coup ») : plus gros coup d'un type
//!   qui ne vide pas vie + bouclier d'énergie.

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
    if armure <= 0.0 || degats <= 0.0 {
        return 0.0;
    }
    (armure / (armure + FACTEUR_ARMURE * degats)).min(REDUCTION_ARMURE_MAX)
}

/// Plus gros coup physique encaissable avec `reserve` points et `armure`.
///
/// Les dégâts subis `D × (1 − réduction(D))` croissent avec `D` : on résout
/// `10 D² − 10 R D − R A = 0` hors plafond, sinon `D = R / 0,1`.
pub fn coup_physique_max(reserve: f64, armure: f64) -> f64 {
    if reserve <= 0.0 {
        return 0.0;
    }
    let plafonne = reserve / (1.0 - REDUCTION_ARMURE_MAX);
    if reduction_armure(armure, plafonne) >= REDUCTION_ARMURE_MAX {
        return plafonne;
    }
    let a = armure.max(0.0);
    let k = FACTEUR_ARMURE;
    (k * reserve + (k * k * reserve * reserve + 4.0 * k * reserve * a).sqrt()) / (2.0 * k)
}

pub struct Defense {
    pub vie: Trace,
    pub bouclier_energie: Trace,
    pub armure: Trace,
    pub resistances: Vec<(TypeDegats, Trace)>,
    pub reduction_armure: Option<Trace>,
    pub coups_max: Vec<(TypeDegats, Trace)>,
}

fn resistance(db: &ModDb, t: TypeDegats) -> Trace {
    let sr = t.stat_resistance();
    let mut s = vec![sr.as_str()];
    if t.est_elementaire() {
        s.push(stats::ELEMENTAL_RESISTANCE);
    }
    let brute = db.somme(TypeModificateur::Added, &s);
    let smax = t.stat_resistance_max();
    let mut s_max = vec![smax.as_str()];
    if t.est_elementaire() {
        s_max.push(stats::ALL_MAXIMUM_ELEMENTAL_RESISTANCE);
    }
    let bonus_max = db.somme(TypeModificateur::Added, &s_max);
    let max = (RESISTANCE_MAX_BASE + bonus_max.valeur).min(RESISTANCE_MAX_ABSOLUE);
    let valeur = brute.valeur.min(max);

    let mut t_brute = Trace::feuille("résistance non plafonnée", brute.valeur);
    t_brute.contributions = brute.contributions;
    let mut t_max = Trace::feuille("résistance maximale", max).avec_formule(format!(
        "min({}, {} + {})",
        nombre(RESISTANCE_MAX_ABSOLUE),
        nombre(RESISTANCE_MAX_BASE),
        nombre(bonus_max.valeur)
    ));
    t_max.contributions = bonus_max.contributions;
    Trace::feuille(format!("résistance {}", t.nom()), valeur)
        .avec_formule(format!("min({}, {})", nombre(brute.valeur), nombre(max)))
        .avec_enfants([t_brute, t_max])
}

pub fn calculer(db: &ModDb, coup_subi: Option<f64>) -> Defense {
    let vie = db.evaluer("vie", &[stats::LIFE], None);
    let bouclier_energie = db.evaluer("bouclier d'énergie", &[stats::ENERGY_SHIELD], None);
    let armure = db.evaluer("armure", &[stats::ARMOUR], None);
    let reserve = vie.valeur.max(0.0) + bouclier_energie.valeur.max(0.0);
    let t_reserve = Trace::feuille("réserve (vie + bouclier d'énergie)", reserve)
        .avec_enfants([vie.clone(), bouclier_energie.clone()]);

    let resistances: Vec<_> = TypeDegats::TOUS
        .into_iter()
        .filter(|t| *t != TypeDegats::Physique)
        .map(|t| (t, resistance(db, t)))
        .collect();

    let reduction = coup_subi.map(|d| {
        let r = reduction_armure(armure.valeur, d);
        Trace::feuille("réduction de l'armure", r * 100.0)
            .avec_formule(format!(
                "min(90, {} / ({} + {} × {}))",
                nombre(armure.valeur),
                nombre(armure.valeur),
                nombre(FACTEUR_ARMURE),
                nombre(d)
            ))
            .avec_enfant(armure.clone())
    });

    let mut coups_max = vec![(
        TypeDegats::Physique,
        Trace::feuille(
            "coup physique maximal",
            coup_physique_max(reserve, armure.valeur),
        )
        .avec_formule("plus grand D tel que D × (1 − réduction(D)) ≤ réserve")
        .avec_enfants([t_reserve.clone(), armure.clone()]),
    )];
    for (t, res) in &resistances {
        let mult = 1.0 - res.valeur / 100.0;
        let valeur = if mult > 0.0 {
            reserve / mult
        } else {
            f64::INFINITY
        };
        coups_max.push((
            *t,
            Trace::feuille(format!("coup {} maximal", t.nom()), valeur)
                .avec_formule(format!(
                    "{} / (1 − {}%)",
                    nombre(reserve),
                    nombre(res.valeur)
                ))
                .avec_enfants([t_reserve.clone(), res.clone()]),
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
