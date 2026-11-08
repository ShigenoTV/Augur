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
    construire_pool, construire_pool_avec,
};
use super::moteur::{Currency, CurrencyKind, GroupId, ItemState, Rarity, Slot};
use super::objectif::{Class, Goal, WantedAffix};
use super::prix::{BASE_NEUVE, REVENTE_BASE, SourcePrix, TablePrix};
use super::solveur::{
    Action, ActionKind, CraftPlan, GoalItem, MacroState, Model, PlanConfig, PlanInputs,
    SolveConfig, build_plan, solve, verify_policy,
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
    /// Essences et leur mod garanti par classe d'objet (Craft of Exile, `essences-coe.json`).
    #[serde(default)]
    pub essences: Vec<EssenceForge>,
}

/// Une Essence : mod garanti par classe d'objet (identifiant RePoE).
#[derive(Clone, Debug, Deserialize)]
pub struct EssenceForge {
    /// Identifiant poe.ninja, qui sert aussi de clé de prix.
    pub id: String,
    pub nom: String,
    /// `lesser`, `normal`, `greater` (objet Magique, qui devient Rare) ; `perfect`, `corrupted` (objet
    /// Rare : un mod retiré, puis le mod garanti posé).
    #[serde(rename = "type")]
    pub genre: String,
    pub cibles: BTreeMap<String, String>,
}

impl EssenceForge {
    pub fn sur_rare(&self) -> bool {
        matches!(self.genre.as_str(), "perfect" | "corrupted")
    }
}

/// Identifiant qui exclut toutes les Essences d'un calcul.
pub const ESSENCES: &str = "essences";

/// Au plus ce nombre d'Essences « remplisseuses » (mod garanti hors objectif) par côté et par genre :
/// les moins chères ; les autres ne changent pas le plan et alourdissent le calcul.
const MAX_ESSENCES_INUTILES: usize = 2;

/// Identifiants des Essences des données embarquées (aucun si elles sont illisibles).
pub fn ids_essences() -> impl Iterator<Item = &'static str> {
    donnees()
        .ok()
        .into_iter()
        .flat_map(|d| d.essences.iter().map(|e| e.id.as_str()))
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
    /// Source du complément Craft of Exile (poids des mods que poe2db n'a pas mesurés).
    pub complement: Option<String>,
    pub date_complement: Option<String>,
}

fn attribution(d: &DonneesForge) -> Attribution {
    Attribution {
        source: d.releve.source.clone(),
        licence: d.releve.licence.clone(),
        date_releve: d.releve.date_releve.clone(),
        repoe_version: d.repoe_version.clone(),
        complement: d.releve.complement.as_ref().map(|c| c.source.clone()),
        date_complement: d.releve.complement.as_ref().map(|c| c.date_releve.clone()),
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

/// Bases écartées malgré des poids : leur pool ne correspond pas à celui de Craft of Exile (contrôle
/// du 08/11/2026). Energy Blade est créée par une compétence et existe en deux classes ; Craft of
/// Exile donne à Grasping Mail des mods de bijou que les tables du jeu ne lui autorisent pas.
/// Fists of Stone (gants d'Ascendance, et leur version Runeforged) ne se modifient pas : Craft of
/// Exile les marque « unmodifiable » (contrôle du 08/11/2026, seules bases concernées).
fn ecartee(nom: &str) -> bool {
    nom.starts_with("[DNT]")
        || nom == "Energy Blade"
        || nom.ends_with("Grasping Mail")
        || nom.ends_with("Fists of Stone")
}

/// Bases calculables (tous leurs mods ont un poids, chez poe2db ou en complément), triées par
/// classe puis niveau d'apparition.
pub fn bases() -> Result<Vec<BaseForge>, ErreurAtelier> {
    static BASES: OnceLock<Vec<BaseForge>> = OnceLock::new();
    let d = donnees()?;
    Ok(BASES.get_or_init(|| calculables(d)).clone())
}

fn calculables(d: &DonneesForge) -> Vec<BaseForge> {
    let mut v: Vec<BaseForge> = d
        .bases
        .bases
        .iter()
        .filter(|b| !ecartee(&b.nom))
        .filter_map(|b| {
            let page = d.releve.page_de(b)?;
            let complete = d.releve.page_utilisable(&page)
                && construire_pool(&d.mods, b, 100, &d.releve)
                    .is_ok_and(|pb| pb.sans_poids.is_empty());
            complete.then(|| BaseForge {
                id: b.id.clone(),
                nom: b.nom.clone(),
                classe: b.classe.clone(),
                niveau_drop: b.niveau_drop,
                page,
            })
        })
        .collect();
    v.sort_by(|a, b| (&a.classe, a.niveau_drop, &a.nom).cmp(&(&b.classe, b.niveau_drop, &b.nom)));
    v
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
    let cibles: Vec<&str> = d
        .essences
        .iter()
        .filter_map(|e| e.cibles.get(&b.classe).map(String::as_str))
        .collect();
    Ok((
        d,
        construire_pool_avec(&d.mods, b, ilvl, &d.releve, &cibles)?,
    ))
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
    /// Aucun palier tirable à ce niveau d'objet : seule une Essence le pose.
    pub essence: bool,
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
    /// Poids venus du complément Craft of Exile.
    pub poids_complement: usize,
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
                essence: false,
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
            g.essence = g.paliers.iter().all(|p| p.poids == 0);
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
        poids_complement: pb.poids_complement,
        attribution: attribution(d),
    })
}

/// Une entrée de la table de prix, avec son nom lisible.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntreeCatalogue {
    pub id: String,
    pub nom: String,
    /// `base`, `monnaie`, `omen` ou `essence`.
    pub genre: String,
}

/// Monnaies, Omens, Essences et prix de base, dans l'ordre d'affichage.
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
    .chain(
        donnees()
            .ok()
            .into_iter()
            .flat_map(|d| d.essences.iter())
            .map(|x| e(&x.id, &x.nom, "essence")),
    )
    .collect()
}

/// Ce que le panneau demande.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandePlan {
    pub base_id: String,
    pub ilvl: u32,
    pub voulus: Vec<WantedAffix>,
    /// Monnaies, Omens et Essences que le joueur ne veut pas utiliser ([`ESSENCES`] : toutes les
    /// Essences).
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
    /// Poids venus du complément Craft of Exile.
    pub poids_complement: usize,
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

/// Essences utilisables sur la classe : un seul mod garanti, présent dans le pool, et un prix (relevé
/// poe.ninja ou saisie ; jamais de prix inventé). Celles dont le mod garanti est hors objectif ne sont
/// gardées que pour les [`MAX_ESSENCES_INUTILES`] moins chères de chaque côté.
fn actions_essences(
    essences: &[EssenceForge],
    classe: &str,
    pool: &super::moteur::AffixPool,
    goal: &Goal,
    table: &TablePrix,
    exclues: &[String],
) -> Vec<Action> {
    if exclues.iter().any(|e| e == ESSENCES) {
        return Vec::new();
    }
    let mut v: Vec<(Action, Option<(Slot, bool)>)> = essences
        .iter()
        .filter(|e| !exclues.contains(&e.id))
        .filter_map(|e| {
            let cible = e.cibles.get(classe)?;
            let idx = pool.affixes.iter().position(|a| &a.id == cible)?;
            let prix = *table
                .prix
                .get(&e.id)
                .filter(|p| p.is_finite() && **p >= 0.0)?;
            let idx = u16::try_from(idx).ok()?;
            let inutile = matches!(goal.classify(pool, idx), Class::Other)
                .then(|| (pool.affixes[idx as usize].slot, e.sur_rare()));
            let c = Currency {
                id: e.id.clone(),
                label: e.nom.clone(),
                kind: CurrencyKind::Essence,
                min_mod_level: 0,
                add_slot: None,
                remove_slot: None,
                target: Some(idx),
                alt_target: None,
                require_tag: None,
                remove_desecrated_only: false,
                remove_lowest_level: false,
                requires_rare: e.sur_rare(),
                unit_cost: prix,
            };
            Some((
                Action {
                    id: c.id.clone(),
                    label: c.label.clone(),
                    cost: prix,
                    kind: ActionKind::Currency(c),
                },
                inutile,
            ))
        })
        .collect();
    v.sort_by(|a, b| a.0.cost.total_cmp(&b.0.cost).then(a.0.id.cmp(&b.0.id)));
    let mut gardees: BTreeMap<(u8, bool), usize> = BTreeMap::new();
    v.into_iter()
        .filter(|(_, inutile)| match inutile {
            None => true,
            Some((slot, rare)) => {
                let n = gardees.entry((rang(*slot), *rare)).or_default();
                *n += 1;
                *n <= MAX_ESSENCES_INUTILES
            }
        })
        .map(|(a, _)| a)
        .collect()
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
    let mut acts =
        actions(&table.prix, Some(&actives)).map_err(|e| ErreurAtelier::Prix(e.to_string()))?;
    let classe = &base(d, &demande.base_id)?.classe;
    let abandon = acts.pop();
    acts.extend(actions_essences(
        &d.essences,
        classe,
        &pool,
        &goal,
        table,
        &demande.exclues,
    ));
    acts.extend(abandon);
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
        poids_complement: pb.poids_complement,
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
        // dagues complétées par Craft of Exile ; bases [DNT] et à pool douteux écartées
        assert!(b.iter().any(|b| b.classe == "Dagger"));
        assert!(!b.iter().any(|b| ecartee(&b.nom)));
        assert!(!b.iter().any(|b| b.nom.contains("Fists of Stone")));
        assert_eq!(catalogue().len(), 26 + d.essences.len());
        assert!(d.essences.len() > 60);
        // chaque mod garanti est dans les tables embarquées
        for e in &d.essences {
            for m in e.cibles.values() {
                assert!(d.mods.mods.iter().any(|x| &x.id == m), "{} : {m}", e.id);
            }
        }
    }

    /// Toutes les Essences cotées (relevé poe.ninja réel du 08/11/2026) : le calcul reste rapide et le
    /// moteur exact confirme le coût du solveur.
    #[test]
    fn plan_avec_les_essences_cotees() {
        let chemin = format!(
            "{}/tests/donnees/forge/ninja-essences.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let doc = serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap();
        let releve = super::super::ninja::releve("L", 1, &[("Essences", doc)], None).unwrap();
        let mut table = TablePrix::indicative();
        table.appliquer_marche(releve);
        let id = bases()
            .unwrap()
            .into_iter()
            .find(|b| b.nom == "Suede Bracers")
            .unwrap()
            .id;
        let a = affixes(&id, 75).unwrap();
        let voulu = |texte: &str| {
            let g = a
                .affixes
                .iter()
                .find(|g| !g.essence && g.libelle.contains(texte))
                .unwrap();
            WantedAffix {
                group: g.group,
                family: g.family,
                max_tier: 2,
            }
        };
        let demande = DemandePlan {
            base_id: id,
            ilvl: 75,
            voulus: vec![voulu("maximum Life"), voulu("Fire Resistance")],
            exclues: vec![],
        };
        let debut = std::time::Instant::now();
        let (r, _) = planifier(&demande, &table, &AtomicBool::new(false)).unwrap();
        let mc = r.plan.mc.as_ref().unwrap();
        assert!(
            (mc.mean_cost - r.plan.expected_cost).abs() < 0.05 * r.plan.expected_cost,
            "solveur {} / moteur {}",
            r.plan.expected_cost,
            mc.mean_cost
        );
        eprintln!(
            "{} ex en {:?}, achats {:?}",
            r.plan.expected_cost,
            debut.elapsed(),
            r.plan.shopping.iter().map(|s| &s.id).collect::<Vec<_>>()
        );
    }

    /// Armure de corps : le % de vie n'existe que par la Perfect Essence of the Body (mod réservé aux
    /// Essences). Le plan passe par elle dès qu'elle a un prix, et jamais sans.
    #[test]
    fn plan_avec_une_essence_perfect() {
        let b = bases()
            .unwrap()
            .into_iter()
            .find(|b| b.classe == "Body Armour" && b.page == "Body_Armours_int")
            .unwrap();
        let a = affixes(&b.id, 80).unwrap();
        let vie = a
            .affixes
            .iter()
            .find(|g| g.essence && g.libelle.contains("increased maximum Life"))
            .unwrap();
        assert_eq!(vie.part, 0.0);
        let demande = DemandePlan {
            base_id: b.id.clone(),
            ilvl: 80,
            voulus: vec![WantedAffix {
                group: vie.group,
                family: vie.family,
                max_tier: u8::MAX,
            }],
            exclues: vec![],
        };
        let annuler = AtomicBool::new(false);
        let mut table = TablePrix::indicative();
        assert!(matches!(
            planifier(&demande, &table, &annuler),
            Err(ErreurAtelier::Solveur(_))
        ));
        table
            .saisir(&[("perfect-essence-of-the-body".to_string(), 50.0)].into())
            .unwrap();
        let (r, _) = planifier(&demande, &table, &annuler).unwrap();
        let achats: Vec<&str> = r.plan.shopping.iter().map(|s| s.id.as_str()).collect();
        assert!(
            achats.contains(&"perfect-essence-of-the-body"),
            "{achats:?}"
        );
        let mc = r.plan.mc.as_ref().unwrap();
        assert!(
            (mc.mean_cost - r.plan.expected_cost).abs() < 0.05 * r.plan.expected_cost,
            "solveur {} / moteur {}",
            r.plan.expected_cost,
            mc.mean_cost
        );
        let exclues = DemandePlan {
            exclues: vec![ESSENCES.to_string()],
            ..demande
        };
        assert!(planifier(&exclues, &table, &annuler).is_err());
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
