//! Forge pour Path of Exile 1 : premier jalon, méthodes de « spam ».
//!
//! Le moteur de tirage (`crate::forge::moteur`) et l'objectif (`crate::forge::objectif`) sont ceux de
//! la Forge PoE 2 ; les données (bases, mods et vrais poids du jeu, Essences) et les règles de tirage
//! sont propres à PoE 1. Cadrage : `docs/forge/poe1.md`.
//!
//! - [`donnees`] : données embarquées et pool d'une base (influence comprise) ;
//! - [`calcul`] : chance de réussite d'un essai pour Alteration, Chaos, Alchemy et chaque Essence.
//!
//! Le coût (prix poe.ninja en Chaos) est appliqué par l'appli : la chance ne dépend pas des prix.

pub mod calcul;
pub mod donnees;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::forge::atelier::{GroupeAffixe, PalierAffixe};
use crate::forge::{GroupId, Slot, WantedAffix};

pub use calcul::{MethodePoe1, ResultatPoe1};
pub use donnees::{BaseVue, ErreurForgePoe1, bases, construire_pool, donnees};

/// Affixes d'une base PoE 1 à un niveau d'objet.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AffixesPoe1 {
    pub base_id: String,
    pub ilvl: u32,
    pub influence: Option<String>,
    pub affixes: Vec<GroupeAffixe>,
}

fn rang(s: Slot) -> u8 {
    match s {
        Slot::Prefix => 0,
        Slot::Suffix => 1,
    }
}

/// Affixes que le panneau propose comme objectif : tiers tirables à ce niveau d'objet, plus les mods
/// garantis par une Essence utilisable (marqués `essence` s'ils ne sont jamais tirés au hasard).
pub fn affixes(
    base_id: &str,
    ilvl: u32,
    influence: Option<&str>,
) -> Result<AffixesPoe1, ErreurForgePoe1> {
    let pb = construire_pool(base_id, ilvl, influence)?;
    let garantis: Vec<u16> = pb.essences.iter().map(|e| e.affixe).collect();
    let mut par_cle: BTreeMap<(u8, GroupId, u16), GroupeAffixe> = BTreeMap::new();
    let mut total: BTreeMap<u8, f64> = BTreeMap::new();
    for (i, a) in pb.pool.affixes.iter().enumerate() {
        let tirable = a.weight > 0 && u32::from(a.req_ilvl) <= ilvl;
        if !tirable && !garantis.contains(&(i as u16)) {
            continue;
        }
        let w = if tirable { a.weight } else { 0 };
        *total.entry(rang(a.slot)).or_default() += f64::from(w);
        let g = par_cle
            .entry((rang(a.slot), a.group, a.family_id))
            .or_insert_with(|| GroupeAffixe {
                group: a.group,
                family: a.family_id,
                slot: a.slot,
                libelle: String::new(),
                part: 0.0,
                essence: false,
                paliers: Vec::new(),
            });
        g.part += f64::from(w);
        g.paliers.push(PalierAffixe {
            tier: a.tier,
            niveau: a.req_ilvl,
            texte: a.text.clone(),
            poids: w,
        });
    }
    let mut affixes: Vec<GroupeAffixe> = par_cle
        .into_values()
        .map(|mut g| {
            g.paliers
                .sort_by_key(|p| (p.tier, std::cmp::Reverse(p.niveau)));
            g.libelle = g
                .paliers
                .first()
                .map(|p| p.texte.clone())
                .unwrap_or_default();
            let t = total.get(&rang(g.slot)).copied().unwrap_or(0.0);
            g.part = if t > 0.0 { g.part / t } else { 0.0 };
            g.essence = g.paliers.iter().all(|p| p.poids == 0);
            g
        })
        .collect();
    affixes.sort_by(|a, b| {
        rang(a.slot)
            .cmp(&rang(b.slot))
            .then(b.part.total_cmp(&a.part))
    });
    Ok(AffixesPoe1 {
        base_id: base_id.to_string(),
        ilvl,
        influence: influence.map(str::to_string),
        affixes,
    })
}

/// Demande de l'appli : base, niveau d'objet, influence et mods voulus.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandePoe1 {
    pub base_id: String,
    pub ilvl: u32,
    #[serde(default)]
    pub influence: Option<String>,
    pub voulus: Vec<WantedAffix>,
}

/// Méthodes de spam pour la demande, de la plus probable à la moins probable.
pub fn planifier(d: &DemandePoe1) -> Result<ResultatPoe1, ErreurForgePoe1> {
    let pb = construire_pool(&d.base_id, d.ilvl, d.influence.as_deref())?;
    calcul::methodes(&pb, &d.voulus)
}

#[cfg(test)]
mod tests;
