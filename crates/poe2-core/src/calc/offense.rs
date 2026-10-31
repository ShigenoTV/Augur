//! Calcul offensif : DPS d'un sort contre une cible.
//!
//! Ordre des opérations (fiche « ordre des opérations des dégâts », confiance
//! `supposé` tant que le Theorycrafter ne l'a pas validée) :
//!
//! 1. dégâts de base de la gemme + dégâts ajoutés × efficacité des dégâts ajoutés ;
//! 2. × (1 + Σ increased / 100) sur `damage`, `spell_damage`,
//!    `elemental_damage` (types élémentaires) et `<type>_damage` ;
//! 3. × Π (1 + more / 100) sur les mêmes stats ;
//! 4. mitigation de la cible : résistance moins pénétration (la pénétration ne
//!    fait pas descendre une résistance positive sous 0), armure pour le physique ;
//! 5. coup moyen × (1 + chance de critique × bonus de dégâts critiques) ;
//! 6. × incantations par seconde.
//!
//! Aucun arrondi n'est appliqué en interne : l'arrondi relève de l'affichage
//! et de la tolérance des cas golden.

use super::defense::reduction_armure;
use super::moddb::ModDb;
use super::modifier::TypeModificateur;
use super::stats::{self, TypeDegats};
use super::trace::{Trace, nombre};
use super::{Cible, Competence};

/// Bonus de dégâts critiques de base de PoE 2, en pourcentage.
pub const BONUS_CRITIQUE_BASE: f64 = 100.0;

pub struct Offense {
    pub dps: Trace,
    pub coup_moyen: Trace,
    pub coup_moyen_brut: Trace,
    pub chance_critique: Trace,
    pub bonus_critique: Trace,
    pub incantations_par_seconde: Trace,
}

fn stats_degats(t: TypeDegats) -> Vec<String> {
    let mut v = vec![stats::DAMAGE.to_string(), stats::SPELL_DAMAGE.to_string()];
    if t.est_elementaire() {
        v.push(stats::ELEMENTAL_DAMAGE.to_string());
    }
    v.push(t.stat_degats());
    v
}

fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

/// Dégâts d'un type, avant mitigation : (min, max, trace de la moyenne).
fn degats_type(db: &ModDb, comp: &Competence, t: TypeDegats) -> Option<(f64, f64, Trace)> {
    let [gmin, gmax] = comp.degats.get(&t).copied().unwrap_or([0.0, 0.0]);
    let smin = t.stat_ajoutes_min();
    let smax = t.stat_ajoutes_max();
    let amin = db.somme(TypeModificateur::Added, &[smin.as_str()]);
    let amax = db.somme(TypeModificateur::Added, &[smax.as_str()]);
    if gmin == 0.0 && gmax == 0.0 && amin.valeur == 0.0 && amax.valeur == 0.0 {
        return None;
    }
    let eff = comp.efficacite_degats_ajoutes / 100.0;
    let bmin = gmin + amin.valeur * eff;
    let bmax = gmax + amax.valeur * eff;

    let mut t_ajoutes = Trace::feuille(
        "dégâts ajoutés (moyenne, après efficacité)",
        (amin.valeur + amax.valeur) / 2.0 * eff,
    )
    .avec_formule(format!(
        "({} à {}) × {}%",
        nombre(amin.valeur),
        nombre(amax.valeur),
        nombre(comp.efficacite_degats_ajoutes)
    ));
    t_ajoutes.contributions = amin.contributions;
    t_ajoutes.contributions.extend(amax.contributions);
    let t_gemme = Trace::feuille("dégâts de la gemme (moyenne)", (gmin + gmax) / 2.0)
        .avec_formule(format!("{} à {}", nombre(gmin), nombre(gmax)));

    let s = stats_degats(t);
    let s = refs(&s);
    let t_inc = db.trace_increased("increased", &s);
    let t_more = db.trace_more("more", &s);
    let mult = t_inc.valeur * t_more.valeur;
    let min = bmin * mult;
    let max = bmax * mult;
    let trace = Trace::feuille(format!("dégâts {} (moyenne)", t.nom()), (min + max) / 2.0)
        .avec_formule(format!(
            "({} à {}) × {} × {} = {} à {}",
            nombre(bmin),
            nombre(bmax),
            nombre(t_inc.valeur),
            nombre(t_more.valeur),
            nombre(min),
            nombre(max)
        ))
        .avec_enfants([t_gemme, t_ajoutes, t_inc, t_more]);
    Some((min, max, trace))
}

/// Multiplicateur de dégâts subis par la cible pour un type donné.
fn mitigation_cible(db: &ModDb, cible: &Cible, t: TypeDegats, moyenne: f64) -> Trace {
    if t == TypeDegats::Physique {
        let dr = reduction_armure(cible.armure, moyenne);
        return Trace::feuille("multiplicateur de dégâts subis (armure)", 1.0 - dr).avec_formule(
            format!(
                "1 − réduction {}% (armure {} contre un coup de {})",
                nombre(dr * 100.0),
                nombre(cible.armure),
                nombre(moyenne)
            ),
        );
    }
    let res = cible.resistances.get(&t).copied().unwrap_or(0.0);
    let sp = t.stat_penetration();
    let pen = db.somme(TypeModificateur::Added, &[sp.as_str()]);
    let effective = if res > 0.0 {
        (res - pen.valeur).max(0.0)
    } else {
        res
    };
    let mut t_pen = Trace::feuille("pénétration", pen.valeur);
    t_pen.contributions = pen.contributions;
    Trace::feuille(
        "multiplicateur de dégâts subis (résistance)",
        1.0 - effective / 100.0,
    )
    .avec_formule(format!(
        "1 − résistance effective {}% (résistance {}%, pénétration {}%, plancher 0 si positive)",
        nombre(effective),
        nombre(res),
        nombre(pen.valeur)
    ))
    .avec_enfant(t_pen)
}

pub fn calculer(db: &ModDb, comp: &Competence, cible: &Cible) -> Offense {
    // 1 à 4 : coup moyen par type, avant et après mitigation.
    let mut brut = 0.0;
    let mut net = 0.0;
    let mut enfants_brut = Vec::new();
    let mut enfants_net = Vec::new();
    for t in TypeDegats::TOUS {
        let Some((min, max, trace_type)) = degats_type(db, comp, t) else {
            continue;
        };
        let moyenne = (min + max) / 2.0;
        let mitig = mitigation_cible(db, cible, t, moyenne);
        let apres = moyenne * mitig.valeur;
        brut += moyenne;
        net += apres;
        enfants_brut.push(trace_type.clone());
        enfants_net.push(
            Trace::feuille(format!("dégâts {} sur la cible", t.nom()), apres)
                .avec_formule(format!("{} × {}", nombre(moyenne), nombre(mitig.valeur)))
                .avec_enfants([trace_type, mitig]),
        );
    }
    let coup_moyen_brut = Trace::feuille("coup moyen avant mitigation", brut)
        .avec_formule("Σ des moyennes par type")
        .avec_enfants(enfants_brut);
    let coup_moyen = Trace::feuille("coup moyen sur la cible", net)
        .avec_formule("Σ des moyennes par type après mitigation, hors critique")
        .avec_enfants(enfants_net);

    // 5 : critique.
    let chance = db.evaluer(
        "chance de critique (brute)",
        &[stats::CRITICAL_HIT_CHANCE, stats::SPELL_CRITICAL_HIT_CHANCE],
        Some((comp.critique_base, "critique de base de la gemme")),
    );
    let chance_plafonnee = chance.valeur.clamp(0.0, 100.0);
    let chance_critique = Trace::feuille("chance de critique", chance_plafonnee)
        .avec_formule(format!("min(100, {})", nombre(chance.valeur)))
        .avec_enfant(chance);
    let bonus_critique = db.evaluer(
        "bonus de dégâts critiques",
        &[
            stats::CRITICAL_DAMAGE_BONUS,
            stats::SPELL_CRITICAL_DAMAGE_BONUS,
        ],
        Some((BONUS_CRITIQUE_BASE, "bonus critique de base")),
    );
    let mult_crit = 1.0 + chance_plafonnee / 100.0 * bonus_critique.valeur / 100.0;
    let t_crit = Trace::feuille("multiplicateur moyen des critiques", mult_crit)
        .avec_formule(format!(
            "1 + {}% × {}%",
            nombre(chance_plafonnee),
            nombre(bonus_critique.valeur)
        ))
        .avec_enfants([chance_critique.clone(), bonus_critique.clone()]);

    // 6 : cadence.
    let base_cadence = if comp.temps_incantation > 0.0 {
        1.0 / comp.temps_incantation
    } else {
        0.0
    };
    let s = [stats::CAST_SPEED];
    let t_inc = db.trace_increased("increased", &s);
    let t_more = db.trace_more("more", &s);
    let cadence = base_cadence * t_inc.valeur * t_more.valeur;
    let incantations_par_seconde = Trace::feuille("incantations par seconde", cadence)
        .avec_formule(format!(
            "1 / {} s × {} × {}",
            nombre(comp.temps_incantation),
            nombre(t_inc.valeur),
            nombre(t_more.valeur)
        ))
        .avec_enfants([t_inc, t_more]);

    let dps = Trace::feuille(format!("DPS de {}", comp.nom), net * mult_crit * cadence)
        .avec_formule(format!(
            "{} × {} × {}",
            nombre(net),
            nombre(mult_crit),
            nombre(cadence)
        ))
        .avec_enfants([coup_moyen.clone(), t_crit, incantations_par_seconde.clone()]);

    Offense {
        dps,
        coup_moyen,
        coup_moyen_brut,
        chance_critique,
        bonus_critique,
        incantations_par_seconde,
    }
}
