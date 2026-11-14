//! Données de la Forge PoE 1 (`data/forge-poe1.json.gz`, `tools/gamedata/forge_poe1_data.py`) et pool
//! d'affixes d'une base.
//!
//! Les poids viennent des tables du jeu (RePoE PoE 1) : pour une base, le poids d'un mod est celui de
//! la **première** étiquette de sa liste `spawn` que l'objet porte (règle du jeu), les étiquettes de
//! l'objet étant celles de la base plus celle de son influence (`ring_shaper`…).

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::forge::{Affix, AffixIdx, AffixPool, GroupId, Slot};

const FORGE_POE1_GZ: &[u8] = include_bytes!("../../../data/forge-poe1.json.gz");
const SCHEMA: u32 = 1;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum ErreurForgePoe1 {
    #[error("données de la Forge PoE 1 illisibles : {0}")]
    Donnees(String),
    #[error("base inconnue : {0}")]
    BaseInconnue(String),
    #[error("niveau d'objet invalide : {0} (1 à 100)")]
    Niveau(u32),
    #[error("influence inconnue pour cette base : {0}")]
    Influence(String),
    #[error("objectif invalide : {0}")]
    Objectif(String),
}

#[derive(Debug, Deserialize)]
pub struct DonneesPoe1 {
    pub schema: u32,
    pub repoe_version: String,
    pub classes: BTreeMap<String, ClassePoe1>,
    pub bases: Vec<BasePoe1>,
    pub mods: Vec<ModPoe1>,
    pub essences: Vec<EssencePoe1>,
}

#[derive(Debug, Deserialize)]
pub struct ClassePoe1 {
    /// Nom affiché de l'influence (Shaper, Elder, Crusader, Redeemer, Hunter, Warlord) → étiquette.
    pub influences: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct BasePoe1 {
    pub id: String,
    pub nom: String,
    pub classe: String,
    pub tags: Vec<String>,
    pub niveau_drop: u32,
    #[serde(default)]
    pub implicites: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModPoe1 {
    pub id: String,
    pub nom: String,
    pub slot: String,
    pub groupes: Vec<String>,
    #[serde(rename = "type")]
    pub famille: String,
    pub niveau: u32,
    pub texte: String,
    pub spawn: Vec<(String, u32)>,
    /// Mod posé seulement par une Essence (jamais tiré au hasard).
    pub essence: bool,
}

#[derive(Debug, Deserialize)]
pub struct EssencePoe1 {
    pub id: String,
    pub nom: String,
    /// 1 (Whispering) à 7 (Deafening) ; 0 pour les Essences spéciales (Delirium, Horror…).
    pub rang: u8,
    /// Niveau d'objet maximal accepté (Whispering 35, Muttering 45, Weeping 60, Wailing 75).
    pub ilvl_max: Option<u32>,
    /// Refait directement un objet Rare (Screaming et au-dessus, Essences spéciales). Sinon, un objet
    /// Normal seulement : il faut un Orb of Scouring entre deux essais.
    pub sur_rare: bool,
    /// Mod garanti par classe d'objet.
    pub cibles: BTreeMap<String, String>,
}

fn charger() -> Result<DonneesPoe1, String> {
    let mut json = String::new();
    flate2::read::GzDecoder::new(FORGE_POE1_GZ)
        .read_to_string(&mut json)
        .map_err(|e| e.to_string())?;
    let d: DonneesPoe1 = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    if d.schema != SCHEMA {
        return Err(format!("schéma {} inattendu", d.schema));
    }
    Ok(d)
}

/// Données embarquées, décompressées au premier appel.
pub fn donnees() -> Result<&'static DonneesPoe1, ErreurForgePoe1> {
    static DONNEES: OnceLock<Result<DonneesPoe1, String>> = OnceLock::new();
    DONNEES
        .get_or_init(charger)
        .as_ref()
        .map_err(|e| ErreurForgePoe1::Donnees(e.clone()))
}

/// Poids d'un mod pour un objet portant ces étiquettes : première étiquette reconnue de la liste.
pub fn poids(spawn: &[(String, u32)], tags: &[&str]) -> u32 {
    spawn
        .iter()
        .find(|(t, _)| tags.contains(&t.as_str()))
        .map_or(0, |(_, w)| *w)
}

/// Une Essence utilisable sur la base, avec l'affixe qu'elle garantit.
#[derive(Debug, Clone)]
pub struct EssenceBase {
    pub essence: &'static EssencePoe1,
    pub affixe: AffixIdx,
}

/// Pool d'une base PoE 1 à un niveau d'objet, influence comprise.
#[derive(Debug, Clone)]
pub struct PoolPoe1 {
    pub base: &'static BasePoe1,
    pub ilvl: u8,
    pub influence: Option<String>,
    pub pool: AffixPool,
    /// Essences applicables à ce niveau d'objet (mod garanti présent dans le pool).
    pub essences: Vec<EssenceBase>,
}

pub fn base(id: &str) -> Result<&'static BasePoe1, ErreurForgePoe1> {
    donnees()?
        .bases
        .iter()
        .find(|b| b.id == id)
        .ok_or_else(|| ErreurForgePoe1::BaseInconnue(id.to_string()))
}

/// Construit le pool d'une base. Les tiers sont numérotés par famille (`type` du jeu) parmi les mods
/// tirables sur la base, tous niveaux confondus (T1 = niveau requis le plus haut) ; un mod d'Essence
/// prend le tier du mod tirable de même famille au niveau égal ou juste en dessous (T1 s'il n'y en a
/// pas). Le pool garde tous les tiers ; le niveau d'objet filtre au tirage (`Affix::req_ilvl`).
pub fn construire_pool(
    base_id: &str,
    ilvl: u32,
    influence: Option<&str>,
) -> Result<PoolPoe1, ErreurForgePoe1> {
    if !(1..=100).contains(&ilvl) {
        return Err(ErreurForgePoe1::Niveau(ilvl));
    }
    let d = donnees()?;
    let b = base(base_id)?;
    let mut tags: Vec<&str> = b.tags.iter().map(String::as_str).collect();
    if let Some(inf) = influence {
        let t = d
            .classes
            .get(&b.classe)
            .and_then(|c| c.influences.get(inf))
            .ok_or_else(|| ErreurForgePoe1::Influence(inf.to_string()))?;
        tags.push(t.as_str());
    }
    let garantis: HashMap<&str, ()> = d
        .essences
        .iter()
        .filter_map(|e| e.cibles.get(&b.classe))
        .map(|m| (m.as_str(), ()))
        .collect();

    // Mods retenus : tirables sur la base, ou garantis par une Essence de la classe.
    let retenus: Vec<(&ModPoe1, u32)> = d
        .mods
        .iter()
        .filter_map(|m| {
            let w = if m.essence { 0 } else { poids(&m.spawn, &tags) };
            (w > 0 || garantis.contains_key(m.id.as_str())).then_some((m, w))
        })
        .collect();

    // Groupes d'exclusion et familles : index stables (ordre alphabétique).
    let mut groupes: BTreeMap<&str, GroupId> = BTreeMap::new();
    for (m, _) in &retenus {
        for g in &m.groupes {
            let n = groupes.len() as GroupId;
            groupes.entry(g.as_str()).or_insert(n);
        }
    }
    let mut familles: BTreeMap<(GroupId, &str), u16> = BTreeMap::new();
    let mut par_famille: HashMap<(GroupId, &str), Vec<u32>> = HashMap::new();
    for (m, w) in &retenus {
        let g = groupe_de(m, &groupes);
        let n = familles.keys().filter(|(h, _)| *h == g).count() as u16;
        familles.entry((g, m.famille.as_str())).or_insert(n);
        if *w > 0 {
            par_famille
                .entry((g, m.famille.as_str()))
                .or_default()
                .push(m.niveau);
        }
    }
    for v in par_famille.values_mut() {
        v.sort_unstable_by(|a, b| b.cmp(a));
        v.dedup();
    }

    let affixes: Vec<Affix> = retenus
        .iter()
        .map(|(m, w)| {
            let g = groupe_de(m, &groupes);
            let niveaux = par_famille
                .get(&(g, m.famille.as_str()))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let tier = niveaux
                .iter()
                .position(|&n| n <= m.niveau)
                .map_or(if niveaux.is_empty() { 1 } else { niveaux.len() }, |p| {
                    p + 1
                });
            Affix {
                id: m.id.clone(),
                name: m.nom.clone(),
                family: m.famille.clone(),
                text: m.texte.clone(),
                group: g,
                family_id: familles[&(g, m.famille.as_str())],
                slot: if m.slot == "prefix" {
                    Slot::Prefix
                } else {
                    Slot::Suffix
                },
                tier: tier as u8,
                req_ilvl: m.niveau.min(100) as u8,
                weight: *w,
                tags: 0,
                desecrated: false,
                cap_shift: (0, 0),
                extra_groups: m
                    .groupes
                    .iter()
                    .skip(1)
                    .map(|g| groupes[g.as_str()])
                    .collect(),
            }
        })
        .collect();

    let index: HashMap<&str, AffixIdx> = affixes
        .iter()
        .enumerate()
        .map(|(i, a)| (a.id.as_str(), i as AffixIdx))
        .collect();
    let essences = d
        .essences
        .iter()
        .filter(|e| e.ilvl_max.is_none_or(|m| ilvl <= m))
        .filter_map(|e| {
            let m = e.cibles.get(&b.classe)?;
            let affixe = *index.get(m.as_str())?;
            // Un mod garanti au-dessus du niveau d'objet reste posé par l'Essence (règle du jeu).
            Some(EssenceBase { essence: e, affixe })
        })
        .collect();

    Ok(PoolPoe1 {
        base: b,
        ilvl: ilvl as u8,
        influence: influence.map(str::to_string),
        pool: AffixPool::new(affixes),
        essences,
    })
}

fn groupe_de(m: &ModPoe1, groupes: &BTreeMap<&str, GroupId>) -> GroupId {
    m.groupes
        .first()
        .map_or(GroupId::MAX - 1, |g| groupes[g.as_str()])
}

/// Une base proposée dans le panneau.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseVue {
    pub id: String,
    pub nom: String,
    pub classe: String,
    pub niveau_drop: u32,
    pub implicites: Vec<String>,
    /// Influences possibles pour la classe (Shaper, Elder, Crusader, Redeemer, Hunter, Warlord).
    pub influences: Vec<String>,
}

/// Bases calculables, triées par classe puis niveau d'apparition.
pub fn bases() -> Result<Vec<BaseVue>, ErreurForgePoe1> {
    let d = donnees()?;
    Ok(d.bases
        .iter()
        .map(|b| BaseVue {
            id: b.id.clone(),
            nom: b.nom.clone(),
            classe: b.classe.clone(),
            niveau_drop: b.niveau_drop,
            implicites: b.implicites.clone(),
            influences: d
                .classes
                .get(&b.classe)
                .map(|c| c.influences.keys().cloned().collect())
                .unwrap_or_default(),
        })
        .collect())
}
