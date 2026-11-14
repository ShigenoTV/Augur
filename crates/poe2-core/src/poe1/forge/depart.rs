//! Objet de départ de la Forge PoE 1 : un objet déjà en possession, copié avec Ctrl+Alt+C en jeu,
//! d'où le plan conseillé repart au lieu d'un objet Normal neuf.
//!
//! La lecture des mods est celle du suivi PoE 2 (`crate::forge::suivi::lire_objet`) : en-tête avancé
//! (`{ Prefix Modifier "Hale" (Tier: 7) — Life }`), nom d'affixe, texte, tier. Les mods artisanaux
//! comptent comme des mods qui prennent une place, sauf un metamod (« Prefixes Cannot Be Changed »…),
//! que le plan connaît.

use serde::Serialize;

use super::donnees::{ErreurForgePoe1, PoolPoe1, bases, construire_pool, donnees};
use super::solveur::{ObjetSolveur, textes_metamods};
use crate::forge::suivi::{ErreurSuivi, ModLu, gabarit, lire_objet};
use crate::forge::{Goal, Rarity};
use crate::parser::{GenreMod, Objet, Rarete, parser};

/// Objet de départ tel que la Forge l'a compris, pour le panneau.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjetDepartPoe1 {
    pub base_id: String,
    pub base_nom: String,
    pub classe: String,
    pub ilvl: Option<u32>,
    /// Influence lue sur l'objet (Shaper, Elder…), si la classe la connaît.
    pub influence: Option<String>,
    pub rarete: Rarity,
    pub mods: Vec<ModLu>,
    /// Metamod d'établi présent (texte).
    pub metamod: Option<String>,
}

fn erreur(m: impl Into<String>) -> ErreurForgePoe1 {
    ErreurForgePoe1::ObjetDepart(m.into())
}

fn erreur_suivi(e: ErreurSuivi) -> ErreurForgePoe1 {
    erreur(e.to_string())
}

/// Base de l'objet : nom exact (pour un objet magique, le nom de base le plus long contenu dans son
/// nom) ; entre bases homonymes (Two-Stone Ring…), celle dont l'implicite est sur l'objet.
fn base_de(objet: &Objet) -> Result<super::BaseVue, ErreurForgePoe1> {
    let nom = objet.base.as_str();
    let magique = objet.rarete == Rarete::Magique;
    let mut candidates: Vec<super::BaseVue> = bases()?
        .into_iter()
        .filter(|b| b.nom == nom || (magique && nom.contains(b.nom.as_str())))
        .collect();
    let longueur = candidates.iter().map(|b| b.nom.len()).max().unwrap_or(0);
    candidates.retain(|b| b.nom.len() == longueur);
    let implicites: Vec<String> = objet
        .mods_de_genre(GenreMod::Implicite)
        .map(|m| gabarit(&m.texte))
        .collect();
    let rang = |b: &super::BaseVue| {
        b.implicites
            .iter()
            .filter(|i| implicites.contains(&gabarit(i)))
            .count()
    };
    candidates
        .into_iter()
        .max_by_key(rang)
        .ok_or_else(|| erreur(format!("base « {nom} » inconnue de la Forge PoE 1")))
}

/// Influence de l'objet (« Shaper Item » → « Shaper »), si la classe la connaît.
fn influence_de(objet: &Objet, classe: &str) -> Result<Option<String>, ErreurForgePoe1> {
    let connues = donnees()?
        .classes
        .get(classe)
        .map(|c| c.influences.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(objet
        .influences
        .iter()
        .map(|i| i.trim_end_matches(" Item").to_string())
        .find(|i| connues.contains(i)))
}

/// Objet lu sur le pool : mods pour le panneau, objet pour le solveur.
fn lire(
    pb: &PoolPoe1,
    goal: Option<&Goal>,
    objet: &Objet,
) -> Result<(Vec<ModLu>, ObjetSolveur), ErreurForgePoe1> {
    if objet.non_identifie {
        return Err(erreur("objet non identifié"));
    }
    let mut lu = lire_objet(&pb.pool, goal, pb.ilvl, objet).map_err(erreur_suivi)?;
    if lu.mods.iter().any(|m| m.fracture) {
        return Err(erreur(
            "objet fracturé : pas encore pris en compte par la Forge PoE 1",
        ));
    }
    let mut inconnus = [lu.inconnus.0, lu.inconnus.1];
    // Un metamod artisanal ne compte pas comme un mod inconnu : le plan le connaît.
    let metamods = textes_metamods(pb);
    let mut metamod = None;
    for m in objet.mods_de_genre(GenreMod::Artisanal) {
        let g = gabarit(&m.texte);
        if let Some(t) = metamods.iter().find(|t| gabarit(t) == g) {
            let cote = match m.entete.as_ref().map(|e| &e.affixe) {
                Some(crate::parser::Affixe::Prefixe) => 0,
                _ => 1,
            };
            inconnus[cote] = inconnus[cote].saturating_sub(1);
            metamod = Some(t.clone());
            // Le panneau l'affiche comme un mod reconnu, pas comme un mauvais mod.
            if let Some(l) = lu.mods.iter_mut().find(|l| gabarit(&l.texte) == g) {
                l.reconnu = true;
            }
        }
    }
    let objet_solveur = ObjetSolveur {
        rarete: lu.item.rarity,
        mods: lu.item.mods().iter().map(|m| m.idx).collect(),
        inconnus,
        metamod,
    };
    Ok((lu.mods, objet_solveur))
}

/// Lit un objet collé : sa base, son niveau d'objet, son influence et ses mods.
pub fn analyser_objet(texte: &str) -> Result<ObjetDepartPoe1, ErreurForgePoe1> {
    let objet = parser(texte).map_err(|e| erreur(e.to_string()))?;
    let b = base_de(&objet)?;
    let ilvl = objet.niveau_objet.filter(|n| (1..=100).contains(n));
    let influence = influence_de(&objet, &b.classe)?;
    let pb = construire_pool(&b.id, ilvl.unwrap_or(100), influence.as_deref())?;
    let (mods, o) = lire(&pb, None, &objet)?;
    Ok(ObjetDepartPoe1 {
        base_id: b.id,
        base_nom: b.nom,
        classe: b.classe,
        ilvl,
        influence,
        rarete: o.rarete,
        mods,
        metamod: o.metamod,
    })
}

/// Objet de départ d'une demande, vérifié contre la base et le niveau choisis.
pub fn objet_depart(
    pb: &PoolPoe1,
    goal: &Goal,
    texte: &str,
) -> Result<(Vec<ModLu>, ObjetSolveur), ErreurForgePoe1> {
    let objet = parser(texte).map_err(|e| erreur(e.to_string()))?;
    let b = base_de(&objet)?;
    if b.id != pb.base.id {
        return Err(erreur(format!(
            "c'est un objet {}, pas la base choisie",
            b.nom
        )));
    }
    if let Some(n) = objet.niveau_objet
        && n != u32::from(pb.ilvl)
    {
        return Err(erreur(format!(
            "niveau d'objet {n}, différent du niveau choisi ({})",
            pb.ilvl
        )));
    }
    let (mods, o) = lire(pb, Some(goal), &objet)?;
    if mods.iter().filter(|m| m.voulu.is_some()).count() >= goal.len() {
        return Err(erreur("ton objet a déjà tous les mods voulus"));
    }
    Ok((mods, o))
}
