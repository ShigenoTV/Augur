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
use super::suivi::{self, ErreurSuivi, ModLu, PlanActif, lire_objet};
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
    /// Essences et Alloys, et leurs mods possibles par classe d'objet (Craft of Exile,
    /// `essences-coe.json`).
    #[serde(default)]
    pub essences: Vec<EssenceForge>,
}

/// Une Essence ou un Alloy : mods possibles par classe d'objet (identifiants RePoE). Plusieurs mods pour
/// une classe : un seul est posé, tiré à chances égales parmi ceux sans conflit de groupe.
#[derive(Clone, Debug, Deserialize)]
pub struct EssenceForge {
    /// Identifiant poe.ninja, qui sert aussi de clé de prix.
    pub id: String,
    pub nom: String,
    /// `lesser`, `normal`, `greater` (objet Magique, qui devient Rare) ; `perfect`, `corrupted`,
    /// `alloy` (objet Rare : un mod retiré, puis le mod garanti posé).
    #[serde(rename = "type")]
    pub genre: String,
    pub cibles: BTreeMap<String, Vec<String>>,
}

impl EssenceForge {
    pub fn sur_rare(&self) -> bool {
        matches!(self.genre.as_str(), "perfect" | "corrupted" | "alloy")
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
    #[error("objet de départ : {0}")]
    ObjetDepart(String),
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
        .filter_map(|e| e.cibles.get(&b.classe))
        .flatten()
        .map(String::as_str)
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
    /// Texte d'un objet déjà en possession (Ctrl+Alt+C en jeu) d'où repartir, au lieu d'une base
    /// neuve. Absent ou vide : base neuve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objet_depart: Option<String>,
}

impl DemandePlan {
    fn texte_depart(&self) -> Option<&str> {
        self.objet_depart
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
    }
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
    /// Mods de l'objet de départ tels que Forge les a compris (`None` : base neuve).
    pub depart: Option<Vec<ModLu>>,
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

/// Essences et Alloys utilisables sur la classe : tous leurs mods possibles présents dans le pool, et un
/// prix (relevé poe.ninja ou saisie ; jamais de prix inventé). Celles à un seul mod hors objectif ne
/// sont gardées que pour les [`MAX_ESSENCES_INUTILES`] moins chères de chaque côté ; celles à plusieurs
/// mods tous hors objectif sont écartées (elles ne servent pas le plan).
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
            let idx: Vec<u16> = e
                .cibles
                .get(classe)?
                .iter()
                .map(|cible| {
                    let i = pool.affixes.iter().position(|a| &a.id == cible)?;
                    u16::try_from(i).ok()
                })
                .collect::<Option<_>>()?;
            let (&premier, autres) = idx.split_first()?;
            let prix = *table
                .prix
                .get(&e.id)
                .filter(|p| p.is_finite() && **p >= 0.0)?;
            let hors = |i: &u16| matches!(goal.classify(pool, *i), Class::Other);
            // Alloy hybride (mod à plusieurs groupes d'exclusion : Swift Alloy sur anneau et gants,
            // Transcendent Alloy sur arme d'attaque) : proposé seulement quand son mod est voulu. Hors
            // objectif, il ne servirait que de remplisseur, que l'état abstrait du solveur ne sait pas
            // suivre (il bloque des groupes que des mauvais affixes anonymes peuvent déjà occuper).
            let hybride_inutile = |i: &u16| {
                !pool.affixes[*i as usize].extra_groups.is_empty()
                    && (!matches!(goal.classify(pool, *i), Class::Wanted(_))
                        || goal.zones(pool, *i).len() > 1)
            };
            if autres.iter().chain([&premier]).any(hybride_inutile) {
                return None;
            }
            if autres.iter().chain([&premier]).all(hors) && !autres.is_empty() {
                return None;
            }
            let inutile = (autres.is_empty() && hors(&premier))
                .then(|| (pool.affixes[premier as usize].slot, e.sur_rare()));
            let c = Currency {
                id: e.id.clone(),
                label: e.nom.clone(),
                kind: CurrencyKind::Essence,
                min_mod_level: 0,
                add_slot: None,
                remove_slot: None,
                target: Some(premier),
                alt_target: None,
                more_targets: autres.to_vec(),
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

/// Objet déjà en possession, lu pour le panneau (bouton « Analyser »).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjetDepart {
    pub base_id: String,
    pub base_nom: String,
    pub classe: String,
    /// Niveau d'objet lu sur l'objet (absent de certaines copies).
    pub ilvl: Option<u32>,
    pub rarete: Rarity,
    pub mods: Vec<ModLu>,
}

fn lire_texte(texte: &str) -> Result<crate::parser::Objet, ErreurAtelier> {
    crate::parser::parser(texte).map_err(|e| ErreurAtelier::ObjetDepart(e.to_string()))
}

/// Base de Forge de l'objet : nom exact, ou pour un objet magique le nom de base le plus long contenu
/// dans son nom (« Athlete's Ornate Cuffs of the Fox »).
fn base_de(objet: &crate::parser::Objet) -> Result<BaseForge, ErreurAtelier> {
    let nom = objet.base.as_str();
    let magique = objet.rarete == crate::parser::Rarete::Magique;
    bases()?
        .into_iter()
        .filter(|b| b.nom == nom || (magique && nom.contains(b.nom.as_str())))
        .max_by_key(|b| b.nom.len())
        .ok_or_else(|| {
            ErreurAtelier::ObjetDepart(format!(
                "base « {nom} » inconnue de Forge (ou sans poids connus)"
            ))
        })
}

fn erreur_suivi(e: ErreurSuivi) -> ErreurAtelier {
    ErreurAtelier::ObjetDepart(e.to_string())
}

/// Lit un objet collé (Ctrl+Alt+C) : sa base, son niveau d'objet et ses mods retrouvés dans le pool. Les
/// valeurs des mods ne servent pas (seuls le nom d'affixe, le texte et le tier) : peu importe que la copie
/// affiche les valeurs avant ou après qualité.
pub fn analyser_objet(texte: &str) -> Result<ObjetDepart, ErreurAtelier> {
    let objet = lire_texte(texte)?;
    let b = base_de(&objet)?;
    let ilvl = objet.niveau_objet.filter(|n| (1..=100).contains(n));
    let (_, pb) = pool(&b.id, ilvl.unwrap_or(100))?;
    let lu = lire_objet(&pb.pool, None, ilvl.unwrap_or(100) as u8, &objet).map_err(erreur_suivi)?;
    Ok(ObjetDepart {
        base_id: b.id,
        base_nom: b.nom,
        classe: b.classe,
        ilvl,
        rarete: lu.item.rarity,
        mods: lu.mods,
    })
}

/// Objet de départ de la demande, rapproché du modèle : état abstrait de départ, objet pour la
/// vérification sur le moteur exact (`None` si un mod inconnu ne s'y remplace pas), mods lus.
fn depart(
    demande: &DemandePlan,
    texte: &str,
    model: &Model,
) -> Result<(MacroState, Option<ItemState>, Vec<ModLu>), ErreurAtelier> {
    let objet = lire_texte(texte)?;
    let b = base_de(&objet)?;
    if b.id != demande.base_id {
        return Err(ErreurAtelier::ObjetDepart(format!(
            "c'est un objet {}, pas la base choisie",
            b.nom
        )));
    }
    if let Some(n) = objet.niveau_objet
        && n != demande.ilvl
    {
        return Err(ErreurAtelier::ObjetDepart(format!(
            "niveau d'objet {n}, différent du niveau choisi ({})",
            demande.ilvl
        )));
    }
    let lu =
        lire_objet(&model.pool, Some(&model.goal), model.ilvl, &objet).map_err(erreur_suivi)?;
    let etat = suivi::etat(model, &lu).ok_or_else(|| {
        ErreurAtelier::ObjetDepart(
            "un mod fracturé hors objectif : l'objectif est inatteignable sur cet objet".into(),
        )
    })?;
    let item = remplacer_inconnus(model, &lu, &etat);
    Ok((etat, item, lu.mods))
}

/// Objet pour le moteur exact : chaque mod inconnu (sans poids, désacralisé, artisanal) remplacé par un
/// mod hors objectif du même côté, sans conflit de groupe. `None` si l'objet obtenu ne se projette pas
/// sur le même état que l'objet lu.
fn remplacer_inconnus(model: &Model, lu: &suivi::ObjetLu, etat: &MacroState) -> Option<ItemState> {
    let mut item = lu.item;
    for (slot, n) in [(Slot::Prefix, lu.inconnus.0), (Slot::Suffix, lu.inconnus.1)] {
        for _ in 0..n {
            let pris: BTreeSet<GroupId> = item
                .mods()
                .iter()
                .flat_map(|m| {
                    let a = &model.pool.affixes[m.idx as usize];
                    std::iter::once(a.group).chain(a.extra_groups.iter().copied())
                })
                .collect();
            let i = model.pool.affixes.iter().enumerate().position(|(k, a)| {
                a.slot == slot
                    && a.weight > 0
                    && !a.desecrated
                    && a.cap_shift == (0, 0)
                    && a.extra_groups.is_empty()
                    && !pris.contains(&a.group)
                    && u16::try_from(k)
                        .is_ok_and(|k| matches!(model.goal.classify(&model.pool, k), Class::Other))
            })?;
            if item.mods().len() >= 6 {
                return None;
            }
            item.push(super::moteur::Mod {
                idx: u16::try_from(i).ok()?,
                fractured: false,
                crafted: false,
            });
        }
    }
    (model.project(&item).as_ref() == Some(etat)).then_some(item)
}

/// Calcule le plan de craft optimal depuis une base neuve (ou l'objet de départ de la demande), avec les prix effectifs de `table`.
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
    let (depart, objet_mc, mods_depart) = match demande.texte_depart() {
        Some(texte) => {
            let (etat, item, mods) = self::depart(demande, texte, &model)?;
            (etat, item, Some(mods))
        }
        None => (
            MacroState::empty(Rarity::Normal),
            Some(ItemState::new(Rarity::Normal, ilvl)),
            None,
        ),
    };
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
    if mods_depart.is_some() {
        // L'objet de départ est déjà payé : seules les bases neuves après un abandon s'achètent.
        for l in plan.shopping.iter_mut().filter(|l| l.id == "__base") {
            l.expected_count -= 1.0;
            l.expected_cost -= prix_base;
            l.label = "Bases neuves (après un abandon)".into();
        }
        plan.shopping
            .retain(|l| l.id != "__base" || l.expected_count > 1e-9);
    }
    plan.mc = objet_mc.and_then(|item| {
        verify_policy(
            &model,
            &sol,
            item,
            ESSAIS_VERIFICATION,
            10_000,
            GRAINE_VERIFICATION,
            |_, _| true,
        )
    });

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
        depart: mods_depart,
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
        assert!(d.essences.len() > 90);
        assert_eq!(d.essences.iter().filter(|e| e.genre == "alloy").count(), 13);
        // chaque mod garanti est dans les tables embarquées
        for e in &d.essences {
            for m in e.cibles.values().flatten() {
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
            objet_depart: None,
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
            objet_depart: None,
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

    /// Plan sur une Essence ou un Alloy à prix saisi, dont l'objectif est un mod que seule elle pose ;
    /// le moteur exact confirme le coût du solveur.
    fn plan_par(classe: &str, ilvl: u32, texte: &str, id: &str, prix: f64) -> ResultatPlan {
        let b = bases()
            .unwrap()
            .into_iter()
            .find(|b| b.classe == classe)
            .unwrap();
        let a = affixes(&b.id, ilvl).unwrap();
        let g = a
            .affixes
            .iter()
            .find(|g| g.essence && g.libelle.contains(texte))
            .unwrap_or_else(|| panic!("{texte} absent"));
        let demande = DemandePlan {
            base_id: b.id.clone(),
            ilvl,
            voulus: vec![WantedAffix {
                group: g.group,
                family: g.family,
                max_tier: u8::MAX,
            }],
            exclues: vec![],
            objet_depart: None,
        };
        let mut table = TablePrix::indicative();
        table.saisir(&[(id.to_string(), prix)].into()).unwrap();
        let (r, _) = planifier(&demande, &table, &AtomicBool::new(false)).unwrap();
        let achats: Vec<&str> = r.plan.shopping.iter().map(|s| s.id.as_str()).collect();
        assert!(achats.contains(&id), "{achats:?}");
        let mc = r.plan.mc.as_ref().unwrap();
        assert!(
            (mc.mean_cost - r.plan.expected_cost).abs() < 0.05 * r.plan.expected_cost,
            "solveur {} / moteur {}",
            r.plan.expected_cost,
            mc.mean_cost
        );
        r
    }

    /// Amulette : le % de Force n'existe que par la Perfect Essence of the Infinite, qui pose % de Force,
    /// de Dextérité ou d'Intelligence à chances égales (Craft of Exile) : environ trois Essences par succès.
    #[test]
    fn plan_avec_une_essence_a_plusieurs_mods() {
        let r = plan_par(
            "Amulet",
            80,
            "increased Strength",
            "perfect-essence-of-the-infinite",
            30.0,
        );
        let n: f64 = r
            .plan
            .shopping
            .iter()
            .filter(|s| s.id == "perfect-essence-of-the-infinite")
            .map(|s| s.expected_count)
            .sum();
        assert!((2.5..4.0).contains(&n), "{n} Essences");
    }

    /// Anneau : le Runic Ward maximum n'existe que par le Runic Alloy (objet Rare : un mod retiré, puis
    /// le mod posé).
    #[test]
    fn plan_avec_un_alloy() {
        plan_par("Ring", 80, "Runic Ward", "runic-alloy", 20.0);
    }

    /// Mêmes tirages que l'émulateur de Craft of Exile (`poe2_essence`, 3 000 essais par cas le 08/11/2026,
    /// relevés dans la PR) : mod posé à chances égales parmi les mods libres, mod retiré du côté du mod
    /// posé quand ce côté est plein, sinon au hasard.
    #[test]
    fn memes_tirages_que_craft_of_exile() {
        use crate::forge::moteur::{ItemState, Mod, Outcome};
        use rand::SeedableRng;
        let d = donnees().unwrap();
        let essai = |base: &str, rarete, mods: &[&str], essence: &str| {
            let b = bases()
                .unwrap()
                .into_iter()
                .find(|b| b.nom == base)
                .unwrap();
            let (_, pb) = pool(&b.id, 80).unwrap();
            let p = &pb.pool;
            let idx = |id: &str| {
                p.affixes
                    .iter()
                    .position(|a| a.id == id)
                    .unwrap_or_else(|| panic!("{id} absent")) as u16
            };
            let e = d.essences.iter().find(|e| e.nom == essence).unwrap();
            let cibles: Vec<u16> = e.cibles[&b.classe].iter().map(|c| idx(c)).collect();
            let c = Currency {
                id: e.id.clone(),
                label: e.nom.clone(),
                kind: CurrencyKind::Essence,
                min_mod_level: 0,
                add_slot: None,
                remove_slot: None,
                target: Some(cibles[0]),
                alt_target: None,
                more_targets: cibles[1..].to_vec(),
                require_tag: None,
                remove_desecrated_only: false,
                remove_lowest_level: false,
                requires_rare: e.sur_rare(),
                unit_cost: 1.0,
            };
            let mut rng = rand::rngs::SmallRng::seed_from_u64(8);
            let (mut poses, mut retires) = (BTreeMap::new(), BTreeMap::new());
            for _ in 0..3000 {
                let mut it = ItemState::new(rarete, 80);
                for m in mods {
                    it.push(Mod {
                        idx: idx(m),
                        fractured: false,
                        crafted: false,
                    });
                }
                assert_eq!(p.apply(&mut it, &c, &mut rng), Outcome::Applied);
                let ids: Vec<&str> = it
                    .mods()
                    .iter()
                    .map(|m| p.affixes[m.idx as usize].id.as_str())
                    .collect();
                let pose = it.mods().iter().find(|m| m.crafted).unwrap().idx;
                *poses
                    .entry(p.affixes[pose as usize].id.clone())
                    .or_insert(0u32) += 1;
                let parti = mods
                    .iter()
                    .find(|m| !ids.contains(m))
                    .copied()
                    .unwrap_or("-");
                *retires.entry(parti.to_string()).or_insert(0u32) += 1;
            }
            (poses, retires)
        };
        // part de chaque issue : 1/k à ±5 points
        let egal = |m: &BTreeMap<String, u32>, ids: &[&str]| {
            assert_eq!(m.len(), ids.len(), "{m:?}");
            for id in ids {
                let f = f64::from(m[*id]) / 3000.0;
                assert!((f - 1.0 / ids.len() as f64).abs() < 0.05, "{id} {f} {m:?}");
            }
        };
        let (poses, retires) = essai(
            "Shortsword",
            Rarity::Magic,
            &["LocalAddedPhysicalDamage1"],
            "Essence of the Infinite",
        );
        egal(&poses, &["Strength4", "Dexterity4", "Intelligence4"]);
        egal(&retires, &["-"]);
        let (poses, _) = essai(
            "Shortsword",
            Rarity::Magic,
            &["Strength1"],
            "Essence of the Infinite",
        );
        egal(&poses, &["Dexterity4", "Intelligence4"]);
        let pleins = [
            "IncreasedLife1",
            "FireResist1",
            "ColdResist1",
            "LightningResist1",
        ];
        let (poses, retires) = essai(
            "Gold Amulet",
            Rarity::Rare,
            &pleins,
            "Perfect Essence of the Infinite",
        );
        egal(
            &poses,
            &[
                "EssencePercentStrength1",
                "EssencePercentDexterity1",
                "EssencePercentIntelligence1",
            ],
        );
        egal(
            &retires,
            &["FireResist1", "ColdResist1", "LightningResist1"],
        );
        let (_, retires) = essai(
            "Gold Amulet",
            Rarity::Rare,
            &["IncreasedLife1", "FireResist1"],
            "Perfect Essence of the Infinite",
        );
        egal(&retires, &["IncreasedLife1", "FireResist1"]);
        let (poses, retires) = essai(
            "Iron Ring",
            Rarity::Rare,
            &[
                "IncreasedLife1",
                "IncreasedMana1",
                "AddedFireDamage1",
                "FireResist1",
            ],
            "Runic Alloy",
        );
        egal(&poses, &["AlloyMaximumRunicWard1"]);
        egal(
            &retires,
            &["IncreasedLife1", "IncreasedMana1", "AddedFireDamage1"],
        );
        let (_, retires) = essai(
            "Iron Ring",
            Rarity::Rare,
            &["IncreasedLife1", "FireResist1"],
            "Runic Alloy",
        );
        egal(&retires, &["IncreasedLife1", "FireResist1"]);
    }

    /// Alloys hybrides (mod à plusieurs groupes d'exclusion), mêmes issues que l'émulateur de Craft of
    /// Exile (`poe2_essence` puis Exalt, 3 000 essais par cas le 09/11/2026, relevés dans la PR) : refusé dès
    /// qu'un mod de l'objet partage un de ses groupes, sinon posé (un mod retiré au hasard) ; une fois posé,
    /// aucun mod de ses groupes n'est plus tiré.
    #[test]
    fn alloys_hybrides_comme_craft_of_exile() {
        use crate::forge::moteur::{DrawFilter, ItemState, Mod, Outcome};
        use rand::SeedableRng;
        let d = donnees().unwrap();
        let pool_de = |base: &str| {
            let b = bases()
                .unwrap()
                .into_iter()
                .find(|b| b.nom == base)
                .unwrap();
            let (_, pb) = pool(&b.id, 80).unwrap();
            (b.classe, pb.pool)
        };
        let idx = |p: &crate::forge::moteur::AffixPool, id: &str| {
            p.affixes
                .iter()
                .position(|a| a.id == id)
                .unwrap_or_else(|| panic!("{id} absent")) as u16
        };
        let objet = |p: &crate::forge::moteur::AffixPool, mods: &[&str], crafted: &[&str]| {
            let mut it = ItemState::new(Rarity::Rare, 80);
            for (m, c) in mods
                .iter()
                .map(|m| (m, false))
                .chain(crafted.iter().map(|m| (m, true)))
            {
                it.push(Mod {
                    idx: idx(p, m),
                    fractured: false,
                    crafted: c,
                });
            }
            it
        };
        // (refus, retraits) sur 3 000 essais
        let essai = |base: &str, mods: &[&str], alloy: &str| {
            let (classe, p) = pool_de(base);
            let e = d.essences.iter().find(|e| e.nom == alloy).unwrap();
            let cible = idx(&p, &e.cibles[&classe][0]);
            assert!(!p.affixes[cible as usize].extra_groups.is_empty());
            let c = Currency {
                id: e.id.clone(),
                label: e.nom.clone(),
                kind: CurrencyKind::Essence,
                min_mod_level: 0,
                add_slot: None,
                remove_slot: None,
                target: Some(cible),
                alt_target: None,
                more_targets: vec![],
                require_tag: None,
                remove_desecrated_only: false,
                remove_lowest_level: false,
                requires_rare: e.sur_rare(),
                unit_cost: 1.0,
            };
            let mut rng = rand::rngs::SmallRng::seed_from_u64(9);
            let (mut refus, mut retires) = (0u32, BTreeMap::new());
            for _ in 0..3000 {
                let mut it = objet(&p, mods, &[]);
                if p.apply(&mut it, &c, &mut rng) == Outcome::NotApplicable {
                    refus += 1;
                    continue;
                }
                assert!(it.mods().iter().any(|m| m.crafted && m.idx == cible));
                let ids: Vec<&str> = it
                    .mods()
                    .iter()
                    .map(|m| p.affixes[m.idx as usize].id.as_str())
                    .collect();
                let parti = mods.iter().find(|m| !ids.contains(m)).unwrap();
                *retires.entry(parti.to_string()).or_insert(0u32) += 1;
            }
            (refus, retires)
        };
        let egal = |m: &BTreeMap<String, u32>, ids: &[&str]| {
            assert_eq!(m.len(), ids.len(), "{m:?}");
            for id in ids {
                let f = f64::from(m[*id]) / 3000.0;
                assert!((f - 1.0 / ids.len() as f64).abs() < 0.05, "{id} {f} {m:?}");
            }
        };
        // Transcendent Alloy (Force, Dextérité et Intelligence) : refusé sur un arc à Dextérité
        let t = "Transcendent Alloy";
        let (refus, _) = essai(
            "Crude Bow",
            &["Dexterity1", "LocalCriticalStrikeChance1"],
            t,
        );
        assert_eq!(refus, 3000);
        let suffixes = [
            "LocalIncreasedAttackSpeed1",
            "LocalCriticalStrikeChance1",
            "LifeGainedFromEnemyDeath1",
        ];
        let (refus, retires) = essai("Crude Bow", &suffixes, t);
        assert_eq!(refus, 0);
        egal(&retires, &suffixes);
        // Swift Alloy : vitesse d'attaque sur anneau (refusé avec la vitesse d'incantation), vitesse
        // d'incantation sur gants (refusé avec la vitesse d'attaque)
        let s = "Swift Alloy";
        let (refus, _) = essai("Iron Ring", &["IncreasedLife1", "CastSpeedJewellery1"], s);
        assert_eq!(refus, 3000);
        let (refus, retires) = essai("Iron Ring", &["IncreasedLife1", "FireResist1"], s);
        assert_eq!(refus, 0);
        egal(&retires, &["IncreasedLife1", "FireResist1"]);
        let (refus, _) = essai(
            "Suede Bracers",
            &["IncreasedLife1", "IncreasedAttackSpeed1"],
            s,
        );
        assert_eq!(refus, 3000);
        let (refus, retires) = essai("Suede Bracers", &["IncreasedLife1", "FireResist1"], s);
        assert_eq!(refus, 0);
        egal(&retires, &["IncreasedLife1", "FireResist1"]);

        // Exalt sur un arc qui porte le mod du Transcendent Alloy : jamais de Dextérité (8 % des tirages
        // sinon) ; parts des autres familles comme chez Craft of Exile (à 2 points près)
        let (_, p) = pool_de("Crude Bow");
        let it = objet(
            &p,
            &[],
            &["AlloyAttributeIncreasedLocalPhysicalDamageHybrid1"],
        );
        let mut part: BTreeMap<String, f64> = BTreeMap::new();
        for (i, q) in p.distribution(&it, &DrawFilter::default()) {
            // famille au sens de Craft of Exile : identifiant du mod sans son numéro de palier
            let id = p.affixes[i as usize]
                .id
                .trim_end_matches(|c: char| c.is_ascii_digit());
            *part.entry(id.to_string()).or_default() += q;
        }
        assert!(!part.keys().any(|f| f.contains("Dexterity")), "{part:?}");
        for (famille, n) in [
            ("LocalAddedLightningDamage", 292.0),
            ("LocalAddedPhysicalDamage", 211.0),
            ("ManaGainedFromEnemyDeath", 218.0),
            ("LocalIncreasedAttackSpeed", 131.0),
            ("LocalIncreasedPhysicalDamagePercent", 130.0),
        ] {
            let q = part.get(famille).copied().unwrap_or(0.0);
            assert!((q - n / 3000.0).abs() < 0.02, "{famille} {q} {part:?}");
        }
    }

    /// Alloys hybrides dans un plan : le mod voulu n'existe que par l'Alloy.
    #[test]
    fn plan_avec_les_alloys_hybrides() {
        plan_par("Ring", 80, "increased Attack Speed", "swift-alloy", 20.0);
        plan_par("Gloves", 80, "increased Cast Speed", "swift-alloy", 20.0);
        plan_par("Bow", 80, "to all Attributes", "transcendent-alloy", 20.0);
    }

    /// Alloy hybride avec d'autres mods voulus : les mods tirés de ses autres groupes (Dextérité sur un
    /// arc, vitesse d'attaque sur des gants) le bloquent ; le moteur exact confirme le coût du solveur.
    #[test]
    fn plan_alloy_hybride_et_autres_mods() {
        let cas = |base: &str, textes: &[&str], alloy: &str| {
            let id = bases()
                .unwrap()
                .into_iter()
                .find(|b| b.nom == base)
                .unwrap()
                .id;
            let a = affixes(&id, 80).unwrap();
            let voulus = textes
                .iter()
                .map(|t| {
                    let g = a
                        .affixes
                        .iter()
                        .find(|g| g.libelle.contains(t))
                        .unwrap_or_else(|| panic!("{t} absent"));
                    WantedAffix {
                        group: g.group,
                        family: g.family,
                        max_tier: 4,
                    }
                })
                .collect();
            let demande = DemandePlan {
                base_id: id,
                ilvl: 80,
                voulus,
                exclues: vec![],
                objet_depart: None,
            };
            let mut table = TablePrix::indicative();
            table.saisir(&[(alloy.to_string(), 20.0)].into()).unwrap();
            let (r, _) = planifier(&demande, &table, &AtomicBool::new(false)).unwrap();
            let achats: Vec<&str> = r.plan.shopping.iter().map(|s| s.id.as_str()).collect();
            assert!(achats.contains(&alloy), "{achats:?}");
            let mc = r.plan.mc.as_ref().unwrap();
            assert!(
                (mc.mean_cost - r.plan.expected_cost).abs() < 0.05 * r.plan.expected_cost,
                "{base} : solveur {} / moteur {}",
                r.plan.expected_cost,
                mc.mean_cost
            );
        };
        cas(
            "Crude Bow",
            &["to all Attributes", "increased Physical Damage"],
            "transcendent-alloy",
        );
        cas(
            "Suede Bracers",
            &["increased Cast Speed", "maximum Life"],
            "swift-alloy",
        );

        // le mod du Transcendent Alloy et la Dextérité s'excluent : objectif refusé
        let id = bases()
            .unwrap()
            .into_iter()
            .find(|b| b.nom == "Crude Bow")
            .unwrap()
            .id;
        let a = affixes(&id, 80).unwrap();
        let voulu = |t: &str| {
            let g = a.affixes.iter().find(|g| g.libelle.contains(t)).unwrap();
            WantedAffix {
                group: g.group,
                family: g.family,
                max_tier: u8::MAX,
            }
        };
        let demande = DemandePlan {
            base_id: id,
            ilvl: 80,
            voulus: vec![voulu("to all Attributes"), voulu("to Dexterity")],
            exclues: vec![],
            objet_depart: None,
        };
        assert!(matches!(
            planifier(&demande, &TablePrix::indicative(), &AtomicBool::new(false)),
            Err(ErreurAtelier::Objectif(_))
        ));
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
            objet_depart: None,
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
