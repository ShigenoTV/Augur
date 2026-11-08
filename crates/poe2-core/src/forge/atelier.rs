//! Forge côté application : données embarquées, bases prises en charge, affixes d'une base et
//! plan de craft (solveur + vérification sur le moteur exact), prêts à sérialiser pour le
//! panneau.
//!
//! Données : `data/forge.json.gz`, produit par `tools/gamedata/forge_data.py` (tables RePoE et
//! relevé de poids poe2db, estimations de Krakenbul sous CC BY-NC-SA 3.0 : l'attribution
//! accompagne chaque réponse, voir [`Attribution`]).

use super::catalogue::{MONNAIES, OMENS, actions, id_combinee};
use super::donnees::{
    BaseTable, ErreurPool, PoolBase, RelevePoids, SansPoids, TableBases, TableMods,
    construire_pool, page_poe2db,
};
use super::moteur::{GroupId, ItemState, Rarity, Slot};
use super::objectif::{Goal, WantedAffix};
use super::prix::{BASE_NEUVE, REVENTE_BASE, SourcePrix, TablePrix};
use super::solveur::{
    CraftPlan, GoalItem, MacroState, Model, PlanConfig, PlanInputs, SolveConfig, build_plan, solve,
    verify_policy,
};
use super::suivi::PlanActif;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};

/// Version du fichier `forge.json.gz` comprise par ce module.
pub const SCHEMA_DONNEES: u32 = 1;

const FORGE_GZ: &[u8] = include_bytes!("../../data/forge.json.gz");

/// Essais Monte-Carlo pour le P50 / P90 (moteur exact, graine fixe : résultat reproductible).
const ESSAIS_VERIFICATION: u64 = 4_000;
const GRAINE_VERIFICATION: u64 = 7;

/// Données embarquées.
#[derive(Debug, Deserialize)]
pub struct DonneesForge {
    pub schema: u32,
    pub repoe_version: String,
    pub mods: TableMods,
    pub bases: TableBases,
    pub releve: RelevePoids,
}

#[derive(Debug, thiserror::Error)]
pub enum ErreurAtelier {
    #[error("données de Forge illisibles : {0}")]
    Donnees(String),
    #[error("base inconnue : {0}")]
    BaseInconnue(String),
    #[error("niveau d'objet invalide : {0} (1 à 100)")]
    Niveau(u32),
    #[error(transparent)]
    Pool(#[from] ErreurPool),
    #[error("objectif invalide : {0}")]
    Objectif(String),
    #[error("prix : {0}")]
    Prix(String),
    #[error("calcul impossible : {0}")]
    Solveur(String),
}

fn charger() -> Result<DonneesForge, String> {
    let mut json = String::new();
    flate2::read::GzDecoder::new(FORGE_GZ)
        .read_to_string(&mut json)
        .map_err(|e| e.to_string())?;
    let d: DonneesForge = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    if d.schema != SCHEMA_DONNEES {
        return Err(format!("schéma {} inattendu", d.schema));
    }
    Ok(d)
}

/// Données embarquées, décompressées au premier appel.
pub fn donnees() -> Result<&'static DonneesForge, ErreurAtelier> {
    static DONNEES: OnceLock<Result<DonneesForge, String>> = OnceLock::new();
    DONNEES
        .get_or_init(charger)
        .as_ref()
        .map_err(|e| ErreurAtelier::Donnees(e.clone()))
}

/// Origine des poids, à afficher avec chaque probabilité ou coût.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribution {
    pub source: String,
    pub licence: String,
    pub date_releve: String,
    pub repoe_version: String,
}

fn attribution(d: &DonneesForge) -> Attribution {
    Attribution {
        source: d.releve.source.clone(),
        licence: d.releve.licence.clone(),
        date_releve: d.releve.date_releve.clone(),
        repoe_version: d.repoe_version.clone(),
    }
}

/// Une base proposée dans le panneau.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseForge {
    pub id: String,
    pub nom: String,
    pub classe: String,
    pub niveau_drop: u32,
    /// Page de poids poe2db utilisée.
    pub page: String,
}

/// Bases dont la page poe2db porte des poids (les autres ne sont pas calculables), triées par
/// classe puis niveau d'apparition.
pub fn bases() -> Result<Vec<BaseForge>, ErreurAtelier> {
    let d = donnees()?;
    let mut v: Vec<BaseForge> = d
        .bases
        .bases
        .iter()
        .filter_map(|b| {
            let page = page_poe2db(b)?;
            let poids = d
                .releve
                .pages
                .get(&page)
                .is_some_and(|p| !p.poids.is_empty());
            poids.then(|| BaseForge {
                id: b.id.clone(),
                nom: b.nom.clone(),
                classe: b.classe.clone(),
                niveau_drop: b.niveau_drop,
                page,
            })
        })
        .collect();
    v.sort_by(|a, b| (&a.classe, a.niveau_drop, &a.nom).cmp(&(&b.classe, b.niveau_drop, &b.nom)));
    Ok(v)
}

fn base<'a>(d: &'a DonneesForge, id: &str) -> Result<&'a BaseTable, ErreurAtelier> {
    d.bases
        .bases
        .iter()
        .find(|b| b.id == id)
        .ok_or_else(|| ErreurAtelier::BaseInconnue(id.to_string()))
}

fn pool(id: &str, ilvl: u32) -> Result<(&'static DonneesForge, PoolBase), ErreurAtelier> {
    if !(1..=100).contains(&ilvl) {
        return Err(ErreurAtelier::Niveau(ilvl));
    }
    let d = donnees()?;
    let b = base(d, id)?;
    Ok((d, construire_pool(&d.mods, b, ilvl, &d.releve)?))
}

/// Un palier d'un affixe.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PalierAffixe {
    pub tier: u8,
    pub niveau: u8,
    pub texte: String,
    pub poids: u32,
}

/// Un affixe visable (groupe d'exclusion + famille) sur la base.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupeAffixe {
    pub group: GroupId,
    pub family: u16,
    pub slot: Slot,
    /// Texte du meilleur palier accessible.
    pub libelle: String,
    /// Chance que le prochain mod d'un Rare vide de ce côté (préfixe ou suffixe) soit celui-ci,
    /// tous paliers confondus (poids estimés).
    pub part: f64,
    /// Paliers accessibles, du meilleur (T1) au plus bas.
    pub paliers: Vec<PalierAffixe>,
}

/// Affixes d'une base à un niveau d'objet.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AffixesBase {
    pub base_id: String,
    pub ilvl: u32,
    pub page: String,
    pub affixes: Vec<GroupeAffixe>,
    /// Mods éligibles écartés faute de poids : les probabilités sont alors incomplètes.
    pub sans_poids: Vec<SansPoids>,
    pub attribution: Attribution,
}

/// Préfixes d'abord.
fn rang(s: Slot) -> u8 {
    match s {
        Slot::Prefix => 0,
        Slot::Suffix => 1,
    }
}

/// Affixes que le panneau propose comme objectif.
pub fn affixes(base_id: &str, ilvl: u32) -> Result<AffixesBase, ErreurAtelier> {
    let (d, pb) = pool(base_id, ilvl)?;
    let mut par_cle: BTreeMap<(u8, GroupId, u16), GroupeAffixe> = BTreeMap::new();
    let mut total: BTreeMap<u8, f64> = BTreeMap::new();
    for a in &pb.pool.affixes {
        *total.entry(rang(a.slot)).or_default() += f64::from(a.weight);
        let g = par_cle
            .entry((rang(a.slot), a.group, a.family_id))
            .or_insert_with(|| GroupeAffixe {
                group: a.group,
                family: a.family_id,
                slot: a.slot,
                libelle: String::new(),
                part: 0.0,
                paliers: Vec::new(),
            });
        g.part += f64::from(a.weight);
        g.paliers.push(PalierAffixe {
            tier: a.tier,
            niveau: a.req_ilvl,
            texte: a.text.clone(),
            poids: a.weight,
        });
    }
    let mut affixes: Vec<GroupeAffixe> = par_cle
        .into_values()
        .map(|mut g| {
            g.paliers.sort_by_key(|p| p.tier);
            g.libelle = g
                .paliers
                .first()
                .map(|p| p.texte.clone())
                .unwrap_or_default();
            let t = total.get(&rang(g.slot)).copied().unwrap_or(0.0);
            g.part = if t > 0.0 { g.part / t } else { 0.0 };
            g
        })
        .collect();
    affixes.sort_by(|a, b| {
        rang(a.slot)
            .cmp(&rang(b.slot))
            .then(b.part.total_cmp(&a.part))
    });
    Ok(AffixesBase {
        base_id: base_id.to_string(),
        ilvl,
        page: pb.page.clone(),
        affixes,
        sans_poids: pb.sans_poids,
        attribution: attribution(d),
    })
}

/// Une entrée de la table de prix, avec son nom lisible.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntreeCatalogue {
    pub id: String,
    pub nom: String,
    /// `base`, `monnaie` ou `omen`.
    pub genre: String,
}

/// Monnaies, Omens et prix de base, dans l'ordre d'affichage.
pub fn catalogue() -> Vec<EntreeCatalogue> {
    let e = |id: &str, nom: &str, genre: &str| EntreeCatalogue {
        id: id.into(),
        nom: nom.into(),
        genre: genre.into(),
    };
    [
        e(BASE_NEUVE, "Base neuve", "base"),
        e(REVENTE_BASE, "Revente d'une base ratée", "base"),
    ]
    .into_iter()
    .chain(MONNAIES.iter().map(|m| e(m.id, m.nom, "monnaie")))
    .chain(OMENS.iter().map(|o| e(o.id, o.nom, "omen")))
    .collect()
}

/// Ce que le panneau demande.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandePlan {
    pub base_id: String,
    pub ilvl: u32,
    pub voulus: Vec<WantedAffix>,
    /// Monnaies et Omens que le joueur ne veut pas utiliser.
    #[serde(default)]
    pub exclues: Vec<String>,
}

/// Prix d'une monnaie, d'un Omen ou de la base achetés par le plan, avec sa source.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrixUtilise {
    pub id: String,
    pub prix: f64,
    pub source: SourcePrix,
}

/// Plan prêt pour le panneau.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultatPlan {
    pub plan: CraftPlan,
    /// Faux si des mods sans poids ont été écartés du pool : coût sous-estimé ou surestimé.
    pub complet: bool,
    pub sans_poids: usize,
    pub prix: Vec<PrixUtilise>,
    pub ligue: Option<String>,
    pub attribution: Attribution,
}

fn actives(exclues: &[String]) -> BTreeSet<String> {
    let ok = |id: &str| !exclues.iter().any(|e| e == id);
    let mut a = BTreeSet::new();
    for m in MONNAIES.iter().filter(|m| ok(m.id)) {
        a.insert(m.id.to_string());
        for o in OMENS
            .iter()
            .filter(|o| o.s_applique_a == m.genre && ok(o.id))
        {
            a.insert(id_combinee(m.id, o.id));
        }
    }
    a
}

/// Résumé des sources de prix, affiché avec le plan.
fn source_des_prix(table: &TablePrix) -> String {
    let n = |s: SourcePrix| table.sources.values().filter(|v| **v == s).count();
    let mut parts = Vec::new();
    if n(SourcePrix::Joueur) > 0 {
        parts.push(format!("{} saisis", n(SourcePrix::Joueur)));
    }
    if n(SourcePrix::PoeNinja) > 0 {
        let ligue = table
            .marche
            .as_ref()
            .map(|m| m.ligue.as_str())
            .unwrap_or("");
        parts.push(format!("{} poe.ninja ({ligue})", n(SourcePrix::PoeNinja)));
    }
    if n(SourcePrix::Indicatif) > 0 {
        parts.push(format!("{} indicatifs", n(SourcePrix::Indicatif)));
    }
    format!("Prix en Exalted : {}", parts.join(", "))
}

/// Calcule le plan de craft optimal depuis une base neuve, avec les prix effectifs de `table`.
/// Rend aussi le plan « actif », que l'overlay suit sur l'objet en cours de craft
/// ([`super::suivi`]).
pub fn planifier(
    demande: &DemandePlan,
    table: &TablePrix,
    annuler: &AtomicBool,
) -> Result<(ResultatPlan, PlanActif), ErreurAtelier> {
    let (d, pb) = pool(&demande.base_id, demande.ilvl)?;
    let ilvl = u8::try_from(demande.ilvl).map_err(|_| ErreurAtelier::Niveau(demande.ilvl))?;
    let pool = Arc::new(pb.pool.clone());
    let goal = Arc::new(Goal::new(&pool, &demande.voulus).map_err(ErreurAtelier::Objectif)?);

    let actives = actives(&demande.exclues);
    let acts =
        actions(&table.prix, Some(&actives)).map_err(|e| ErreurAtelier::Prix(e.to_string()))?;
    let (prix_base, revente) = table.base();
    let goal_items: Vec<GoalItem> = demande
        .voulus
        .iter()
        .map(|w| {
            let meilleur = pool
                .affixes
                .iter()
                .filter(|a| a.group == w.group && a.family_id == w.family)
                .min_by_key(|a| a.tier);
            GoalItem {
                label: meilleur.map(|a| a.text.clone()).unwrap_or_default(),
                slot: meilleur.map(|a| a.slot).unwrap_or(Slot::Prefix),
                group: w.group,
                family_id: w.family,
                max_tier: w.max_tier,
            }
        })
        .collect();

    let labels: Vec<String> = goal_items.iter().map(|g| g.label.clone()).collect();
    let model = Model::new(pool, goal, ilvl, acts, prix_base, revente);
    let depart = MacroState::empty(Rarity::Normal);
    let sol = solve(&model, &[depart], &SolveConfig::default(), annuler)
        .map_err(ErreurAtelier::Solveur)?;
    let mut plan = build_plan(
        &PlanInputs {
            model: &model,
            sol: &sol,
            start: depart,
            base_id: &demande.base_id,
            base_cost: prix_base,
            salvage: revente,
            goal_items,
            prices_source: source_des_prix(table),
        },
        &PlanConfig::default(),
    )
    .map_err(ErreurAtelier::Solveur)?;
    plan.mc = verify_policy(
        &model,
        &sol,
        ItemState::new(Rarity::Normal, ilvl),
        ESSAIS_VERIFICATION,
        10_000,
        GRAINE_VERIFICATION,
        |_, _| true,
    );

    // Prix réellement utilisés par le plan (achats moyens), avec leur source.
    let mut utilises: BTreeSet<&str> = BTreeSet::new();
    for ligne in &plan.shopping {
        if ligne.id == "__base" {
            utilises.extend([BASE_NEUVE, REVENTE_BASE]);
        } else {
            utilises.extend(ligne.id.split('+'));
        }
    }
    let prix = utilises
        .into_iter()
        .filter_map(|id| {
            Some(PrixUtilise {
                id: id.to_string(),
                prix: *table.prix.get(id)?,
                source: *table.sources.get(id)?,
            })
        })
        .collect();

    let resultat = ResultatPlan {
        complet: pb.complet(),
        sans_poids: pb.sans_poids.len(),
        prix,
        ligue: table.marche.as_ref().map(|m| m.ligue.clone()),
        attribution: attribution(d),
        plan,
    };
    let base_nom = base(d, &demande.base_id)?.nom.clone();
    let actif = PlanActif {
        base_id: demande.base_id.clone(),
        base_nom,
        ilvl,
        labels,
        expected_cost: resultat.plan.expected_cost,
        model,
        sol,
    };
    Ok((resultat, actif))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn donnees_embarquees_lisibles() {
        let d = donnees().unwrap();
        assert!(d.mods.mods.len() > 1000);
        assert_eq!(d.releve.licence, "CC BY-NC-SA 3.0");
        let b = bases().unwrap();
        assert!(b.len() > 100, "{} bases", b.len());
        assert!(b.iter().any(|b| b.nom == "Suede Bracers"));
        // pas de page de poids pour les dagues : base absente
        assert!(!b.iter().any(|b| b.classe == "Dagger"));
        assert_eq!(catalogue().len(), 26);
    }

    #[test]
    fn affixes_d_une_base() {
        let id = &bases()
            .unwrap()
            .into_iter()
            .find(|b| b.nom == "Suede Bracers")
            .unwrap()
            .id;
        let a = affixes(id, 45).unwrap();
        assert!(a.sans_poids.is_empty());
        for slot in [Slot::Prefix, Slot::Suffix] {
            let s: f64 = a
                .affixes
                .iter()
                .filter(|g| g.slot == slot)
                .map(|g| g.part)
                .sum();
            assert!((s - 1.0).abs() < 1e-9, "{slot:?} {s}");
        }
        let vie = a
            .affixes
            .iter()
            .find(|g| g.libelle.contains("maximum Life"))
            .unwrap();
        assert_eq!(
            vie.paliers[0].tier,
            vie.paliers.iter().map(|p| p.tier).min().unwrap()
        );
        assert!(matches!(affixes(id, 0), Err(ErreurAtelier::Niveau(0))));
        assert!(matches!(
            affixes("inconnue", 45),
            Err(ErreurAtelier::BaseInconnue(_))
        ));
    }

    #[test]
    fn plan_vie_et_feu() {
        let id = bases()
            .unwrap()
            .into_iter()
            .find(|b| b.nom == "Suede Bracers")
            .unwrap()
            .id;
        let a = affixes(&id, 45).unwrap();
        let voulu = |texte: &str| {
            let g = a
                .affixes
                .iter()
                .find(|g| g.libelle.contains(texte))
                .unwrap();
            WantedAffix {
                group: g.group,
                family: g.family,
                max_tier: u8::MAX,
            }
        };
        let demande = DemandePlan {
            base_id: id,
            ilvl: 45,
            voulus: vec![voulu("maximum Life"), voulu("Fire Resistance")],
            exclues: vec![],
        };
        let (r, actif) =
            planifier(&demande, &TablePrix::indicative(), &AtomicBool::new(false)).unwrap();
        assert_eq!(actif.base_nom, "Suede Bracers");
        assert!(r.complet);
        assert!(r.plan.expected_cost.is_finite() && r.plan.expected_cost > 0.0);
        let mc = r.plan.mc.as_ref().unwrap();
        assert!(mc.p90_cost >= mc.median_cost);
        let ids: Vec<&str> = r.prix.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&BASE_NEUVE), "{ids:?}");
        assert!(r.prix.len() < 26, "seuls les prix des achats : {ids:?}");
        assert!(r.prix.iter().all(|p| p.source == SourcePrix::Indicatif));
        assert!(r.plan.prices_source.contains("indicatifs"));
    }
}
