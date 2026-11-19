//! Suivi d'un plan de la Forge PoE 1 par l'overlay : Ctrl+Alt+C sur l'objet en cours de craft donne
//! l'action conseillée, recalculée depuis l'objet tel qu'il est (le plan PoE 1 sait repartir de
//! n'importe quel objet, voir `depart`).

use serde::Serialize;

use super::depart::{base_de_texte, lire_objet_pour};
use super::{DemandePoe1, ErreurForgePoe1, affixes, construire_pool, planifier};
use crate::forge::Goal;
use crate::forge::enchainement::{Enchainement, Repere};
use crate::parser::parser;

/// Ce que l'overlay affiche pour l'objet en cours de craft.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConseilPoe1 {
    pub base_nom: String,
    /// Rareté lue (« Normal », « Magique », « Rare »).
    pub rarete: String,
    /// Mods voulus déjà sur l'objet, et ceux qui manquent.
    pub presents: Vec<String>,
    pub manquants: Vec<String>,
    /// Tous les mods voulus sont là.
    pub fini: bool,
    /// Action conseillée (absente si l'objet est fini ou sans plan).
    pub action: Option<String>,
    /// Coût d'une utilisation de l'action, et coût moyen restant jusqu'à l'objet fini, en Chaos.
    pub cout: Option<f64>,
    pub reste: Option<f64>,
    /// Bilan de l'essai depuis la copie précédente (rempli par l'appli, qui garde la série).
    pub enchainement: Option<Enchainement>,
}

impl ConseilPoe1 {
    /// Repère de cette copie pour la série d'essais (`texte` : texte copié en jeu).
    pub fn repere(&self, texte: &str) -> Repere {
        Repere {
            empreinte: texte.trim().to_string(),
            reste: if self.fini { Some(0.0) } else { self.reste },
            voulus: self.presents.len() as u8,
            fini: self.fini,
            // Le plan PoE 1 repart toujours de l'objet tel qu'il est.
            abandon: false,
            cout_essai: self.cout,
        }
    }
}

/// Libellés courts des mods voulus de la demande, dans l'ordre.
pub fn libelles(d: &DemandePoe1) -> Result<Vec<String>, ErreurForgePoe1> {
    let a = affixes(&d.base_id, d.ilvl, d.influence.as_deref())?;
    Ok(d.voulus
        .iter()
        .enumerate()
        .map(|(i, w)| {
            a.affixes
                .iter()
                .find(|g| g.group == w.group && g.family == w.family)
                .map(|g| g.libelle.lines().next().unwrap_or_default().to_string())
                .unwrap_or_else(|| format!("mod {}", i + 1))
        })
        .collect())
}

/// Conseil pour le texte copié en jeu. `Ok(None)` : ce n'est pas la base du plan suivi.
pub fn conseiller(d: &DemandePoe1, texte: &str) -> Result<Option<ConseilPoe1>, ErreurForgePoe1> {
    let Ok(objet) = parser(texte) else {
        return Ok(None);
    };
    let Ok(base) = base_de_texte(&objet) else {
        return Ok(None);
    };
    if base.id != d.base_id {
        return Ok(None);
    }
    let pb = construire_pool(&d.base_id, d.ilvl, d.influence.as_deref())?;
    let goal = Goal::new(&pb.pool, &d.voulus).map_err(ErreurForgePoe1::Objectif)?;
    let mods = lire_objet_pour(&pb, &goal, &objet)?;
    let noms = libelles(d)?;
    let mut presents = Vec::new();
    let mut manquants = Vec::new();
    for (i, nom) in noms.iter().enumerate() {
        if mods.iter().any(|m| m.voulu.map(usize::from) == Some(i)) {
            presents.push(nom.clone());
        } else {
            manquants.push(nom.clone());
        }
    }
    let rarete = match objet.rarete {
        crate::parser::Rarete::Magique => "Magique",
        crate::parser::Rarete::Rare => "Rare",
        _ => "Normal",
    }
    .to_string();
    let mut c = ConseilPoe1 {
        base_nom: base.nom,
        rarete,
        fini: manquants.is_empty(),
        presents,
        manquants,
        action: None,
        cout: None,
        reste: None,
        enchainement: None,
    };
    if c.fini {
        return Ok(Some(c));
    }
    let r = planifier(&DemandePoe1 {
        objet_depart: Some(texte.to_string()),
        ..d.clone()
    })?;
    if let Some(plan) = r.plan
        && let Some(e) = plan.etapes.iter().find(|e| e.id == plan.racine)
    {
        c.action = Some(e.action.clone());
        c.cout = Some(e.cout);
        c.reste = Some(e.reste);
    }
    Ok(Some(c))
}
