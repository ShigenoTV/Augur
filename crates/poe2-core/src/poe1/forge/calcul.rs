//! Méthodes de « spam » de la Forge PoE 1 : chance de réussite d'un essai.
//!
//! Règles de tirage (code de Craft of Exile pour PoE 1, relevé du 14/11/2026) :
//!
//! - un objet **Magique** refait (Orb of Alteration) reçoit 1 ou 2 mods à 50/50 ; avec 1 mod, le côté
//!   (préfixe ou suffixe) suit les poids ; avec 2 mods, un préfixe et un suffixe ;
//! - un objet **Rare** refait (Chaos Orb, Orb of Alchemy, Essence) reçoit 4, 5 ou 6 mods avec les
//!   chances 8/12, 3/12 et 1/12, tirés l'un après l'autre (au plus 3 préfixes et 3 suffixes, un mod
//!   par groupe d'exclusion) ;
//! - une **Essence** pose d'abord son mod garanti, puis complète au hasard (même loi du nombre de mods,
//!   mod garanti compris : hypothèse, à confirmer en jeu).
//!
//! Le cas Magique est exact. Le cas Rare suit chaque mod voulu exactement et résume les autres mods par
//! côté (poids restant moyen après k tirages sans remise, même approximation que le solveur PoE 2) ;
//! un test le compare à une simulation complète sur le moteur exact.

use std::collections::HashMap;

use serde::Serialize;

use crate::forge::{AffixIdx, AffixPool, Class, Goal, GroupId, Slot, WantedAffix};

use super::donnees::{ErreurForgePoe1, PoolPoe1};

/// Loi du nombre de mods d'un Rare refait : (nombre, chance).
pub const MODS_RARE: [(u8, f64); 3] = [(4, 8.0 / 12.0), (5, 3.0 / 12.0), (6, 1.0 / 12.0)];
/// Chance qu'un objet Magique refait ait deux mods.
pub const DEUX_MODS_MAGIQUE: f64 = 0.5;

pub const ALTERATION: &str = "Orb of Alteration";
pub const AUGMENTATION: &str = "Orb of Augmentation";
pub const CHAOS: &str = "Chaos Orb";
pub const ALCHEMY: &str = "Orb of Alchemy";
pub const SCOURING: &str = "Orb of Scouring";

/// Une monnaie consommée par essai, en moyenne.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conso {
    /// Nom anglais de la monnaie (celui de poe.ninja).
    pub nom: String,
    pub quantite: f64,
}

/// Une méthode de spam et sa chance par essai.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodePoe1 {
    pub id: String,
    pub nom: String,
    /// `alteration`, `chaos`, `alchimie` ou `essence`.
    pub genre: String,
    /// Chance de réussite d'un essai (0 à 1).
    pub chance: f64,
    /// Monnaies d'un essai, en moyenne (l'Augmentation n'est utilisée que quand elle peut finir l'objet).
    pub monnaies: Vec<Conso>,
    /// Précision utile au joueur (comment enchaîner).
    pub note: String,
}

/// Méthode écartée et pourquoi.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ecartee {
    pub nom: String,
    pub raison: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultatPoe1 {
    pub methodes: Vec<MethodePoe1>,
    pub ecartees: Vec<Ecartee>,
}

fn conso(nom: &str, quantite: f64) -> Conso {
    Conso {
        nom: nom.to_string(),
        quantite,
    }
}

/// Poids tirables d'un objectif à ce niveau d'objet.
struct Poids {
    /// Par voulu : (slot, poids accepté, poids bloquant) par côté.
    bon: Vec<[f64; 2]>,
    mauvais: Vec<[f64; 2]>,
    /// Autres groupes : poids par (groupe, côté).
    autres: HashMap<(GroupId, usize), f64>,
}

fn cote(s: Slot) -> usize {
    match s {
        Slot::Prefix => 0,
        Slot::Suffix => 1,
    }
}

fn poids(pool: &AffixPool, goal: &Goal, ilvl: u8) -> Poids {
    let n = goal.len();
    let mut p = Poids {
        bon: vec![[0.0; 2]; n],
        mauvais: vec![[0.0; 2]; n],
        autres: HashMap::new(),
    };
    for (i, a) in pool.affixes.iter().enumerate() {
        if a.weight == 0 || a.req_ilvl > ilvl {
            continue;
        }
        let w = f64::from(a.weight);
        let c = cote(a.slot);
        match goal.classify(pool, i as AffixIdx) {
            Class::Wanted(k) => p.bon[k][c] += w,
            Class::Blocked(k) => p.mauvais[k][c] += w,
            Class::Other => *p.autres.entry((a.group, c)).or_default() += w,
        }
    }
    p
}

/// P(groupe j encore absent après k tirages pondérés sans remise), k = 0..=kmax.
fn survie(w: &[f64], kmax: usize) -> Vec<Vec<f64>> {
    fn dfs(
        w: &[f64],
        prof: usize,
        kmax: usize,
        pris: &mut Vec<usize>,
        reste: f64,
        p: f64,
        dedans: &mut [Vec<f64>],
    ) {
        for &j in pris.iter() {
            dedans[prof][j] += p;
        }
        if prof == kmax || reste <= 0.0 {
            return;
        }
        for j in 0..w.len() {
            if pris.contains(&j) || w[j] <= 0.0 {
                continue;
            }
            pris.push(j);
            dfs(
                w,
                prof + 1,
                kmax,
                pris,
                reste - w[j],
                p * w[j] / reste,
                dedans,
            );
            pris.pop();
        }
    }
    let mut dedans = vec![vec![0.0; w.len()]; kmax + 1];
    dfs(
        w,
        0,
        kmax,
        &mut Vec::new(),
        w.iter().sum(),
        1.0,
        &mut dedans,
    );
    dedans
        .into_iter()
        .map(|r| r.into_iter().map(|x| 1.0 - x).collect())
        .collect()
}

/// Poids « autres » restant en moyenne de chaque côté après k autres tirages de ce côté.
fn reste_autres(p: &Poids, exclu: Option<(GroupId, usize)>, kmax: usize) -> [Vec<f64>; 2] {
    let mut r: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    for (c, rc) in r.iter_mut().enumerate() {
        let w: Vec<f64> = p
            .autres
            .iter()
            .filter(|((g, cc), _)| *cc == c && exclu.is_none_or(|(eg, _)| eg != *g))
            .map(|(_, w)| *w)
            .collect();
        let s = survie(&w, kmax);
        *rc = s
            .iter()
            .map(|row| row.iter().zip(&w).map(|(a, b)| a * b).sum())
            .collect();
    }
    r
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Etat {
    bons: u8,
    n: [u8; 2],
    autres: [u8; 2],
    reste: u8,
}

struct Rare<'a> {
    p: &'a Poids,
    autres: [Vec<f64>; 2],
    cap: [u8; 2],
    complet: u8,
    memo: HashMap<Etat, f64>,
}

impl Rare<'_> {
    fn chance(&mut self, e: Etat) -> f64 {
        if e.bons == self.complet {
            return 1.0;
        }
        if e.reste == 0 {
            return 0.0;
        }
        if let Some(&v) = self.memo.get(&e) {
            return v;
        }
        let manquants = (self.complet & !e.bons).count_ones() as u8;
        if manquants > e.reste {
            return 0.0;
        }
        let ouvert = [e.n[0] < self.cap[0], e.n[1] < self.cap[1]];
        let mut total = 0.0;
        let mut issues: Vec<(f64, Option<Etat>)> = Vec::new();
        for k in 0..self.p.bon.len() {
            if e.bons & (1 << k) != 0 {
                continue;
            }
            for (c, _) in ouvert.iter().enumerate().filter(|(_, o)| **o) {
                let (b, m) = (self.p.bon[k][c], self.p.mauvais[k][c]);
                total += b + m;
                if b > 0.0 {
                    let mut s = e;
                    s.bons |= 1 << k;
                    s.n[c] += 1;
                    s.reste -= 1;
                    issues.push((b, Some(s)));
                }
                // un mauvais tier occupe le groupe : l'objectif est manqué
            }
        }
        for (c, _) in ouvert.iter().enumerate().filter(|(_, o)| **o) {
            let w = self.autres[c]
                .get(e.autres[c] as usize)
                .copied()
                .unwrap_or(0.0)
                .max(0.0);
            if w > 0.0 {
                total += w;
                let mut s = e;
                s.n[c] += 1;
                s.autres[c] += 1;
                s.reste -= 1;
                issues.push((w, Some(s)));
            }
        }
        let v = if total <= 0.0 {
            0.0
        } else {
            issues
                .into_iter()
                .map(|(w, s)| w / total * s.map_or(0.0, |s| self.chance(s)))
                .sum()
        };
        self.memo.insert(e, v);
        v
    }
}

/// Chance qu'un objet Rare refait (Chaos, Alchemy, ou Essence si `garanti`) atteigne l'objectif.
pub fn chance_rare(pool: &AffixPool, goal: &Goal, ilvl: u8, garanti: Option<AffixIdx>) -> f64 {
    let p = poids(pool, goal, ilvl);
    let mut depart = Etat {
        bons: 0,
        n: [0, 0],
        autres: [0, 0],
        reste: 0,
    };
    let mut exclu = None;
    if let Some(g) = garanti {
        let a = &pool.affixes[g as usize];
        let c = cote(a.slot);
        depart.n[c] += 1;
        match goal.classify(pool, g) {
            Class::Wanted(k) => depart.bons |= 1 << k,
            Class::Blocked(_) => return 0.0,
            Class::Other => exclu = Some((a.group, c)),
        }
    }
    let (cp, cs) = pool.cap(crate::forge::Rarity::Rare);
    let mut r = Rare {
        p: &p,
        autres: reste_autres(&p, exclu, 3),
        cap: [cp, cs],
        complet: ((1u16 << goal.len()) - 1) as u8,
        memo: HashMap::new(),
    };
    MODS_RARE
        .iter()
        .map(|&(n, pn)| {
            let mut e = depart;
            e.reste = n - garanti.map_or(0, |_| 1);
            pn * r.chance(e)
        })
        .sum()
}

/// Méthode Alteration (+ Augmentation quand elle peut finir l'objet) : chance par essai et nombre
/// moyen d'Augmentations par essai. `None` si l'objectif ne tient pas sur un objet Magique.
pub fn chance_alteration(pool: &AffixPool, goal: &Goal, ilvl: u8) -> Option<(f64, f64)> {
    let p = poids(pool, goal, ilvl);
    let mut voulu: [Option<f64>; 2] = [None, None];
    for k in 0..goal.len() {
        let c = cote(goal.slots[k]);
        if voulu[c].is_some() {
            return None; // deux préfixes (ou deux suffixes) voulus : pas sur un Magique
        }
        voulu[c] = Some(p.bon[k][c]);
    }
    let total_cote = |c: usize| -> f64 {
        p.bon.iter().map(|b| b[c]).sum::<f64>()
            + p.mauvais.iter().map(|m| m[c]).sum::<f64>()
            + p.autres
                .iter()
                .filter(|((_, cc), _)| *cc == c)
                .map(|(_, w)| w)
                .sum::<f64>()
    };
    let (tp, ts) = (total_cote(0), total_cote(1));
    let t = tp + ts;
    if t <= 0.0 {
        return Some((0.0, 0.0));
    }
    let frac = |a: f64, b: f64| if b > 0.0 { a / b } else { 0.0 };
    let un = 1.0 - DEUX_MODS_MAGIQUE;
    let (direct, aug_q, aug_ok) = match (voulu[0], voulu[1]) {
        (Some(gp), None) => (un * gp / t, 0.0, 0.0),
        (None, Some(gs)) => (un * gs / t, 0.0, 0.0),
        (Some(gp), Some(gs)) => (
            0.0,
            un * (gp + gs) / t,
            un * (gp / t * frac(gs, ts) + gs / t * frac(gp, tp)),
        ),
        (None, None) => return None,
    };
    // Un seul mod voulu : un mod de l'autre côté, puis l'Augmentation peut encore poser le voulu.
    let (aug_q, aug_ok) = match (voulu[0], voulu[1]) {
        (Some(gp), None) => (un * ts / t, un * ts / t * frac(gp, tp)),
        (None, Some(gs)) => (un * tp / t, un * tp / t * frac(gs, ts)),
        _ => (aug_q, aug_ok),
    };
    let deux = DEUX_MODS_MAGIQUE
        * voulu[0].map_or(1.0, |g| frac(g, tp))
        * voulu[1].map_or(1.0, |g| frac(g, ts));
    Some((direct + aug_ok + deux, aug_q))
}

/// Toutes les méthodes de spam applicables, de la plus probable à la moins probable.
pub fn methodes(pb: &PoolPoe1, voulus: &[WantedAffix]) -> Result<ResultatPoe1, ErreurForgePoe1> {
    let goal = Goal::new(&pb.pool, voulus).map_err(ErreurForgePoe1::Objectif)?;
    let pool = &pb.pool;
    let ilvl = pb.ilvl;
    let mut methodes = Vec::new();
    let mut ecartees = Vec::new();

    match chance_alteration(pool, &goal, ilvl) {
        Some((chance, aug)) if chance > 0.0 => methodes.push(MethodePoe1 {
            id: "alteration".into(),
            nom: "Alteration + Augmentation".into(),
            genre: "alteration".into(),
            chance,
            monnaies: vec![conso(ALTERATION, 1.0), conso(AUGMENTATION, aug)],
            note: "Objet Magique : Alteration jusqu'au bon mod, Augmentation quand il reste une place qui peut finir l'objet.".into(),
        }),
        Some(_) => ecartees.push(Ecartee {
            nom: "Alteration + Augmentation".into(),
            raison: "aucun mod voulu ne peut apparaître à ce niveau d'objet".into(),
        }),
        None => ecartees.push(Ecartee {
            nom: "Alteration + Augmentation".into(),
            raison: "un objet Magique n'a qu'un préfixe et un suffixe".into(),
        }),
    }

    let rare = chance_rare(pool, &goal, ilvl, None);
    if rare > 0.0 {
        methodes.push(MethodePoe1 {
            id: "chaos".into(),
            nom: "Chaos Orb".into(),
            genre: "chaos".into(),
            chance: rare,
            monnaies: vec![conso(CHAOS, 1.0)],
            note: "Objet Rare : un Chaos Orb refait tous les mods (4 à 6).".into(),
        });
        methodes.push(MethodePoe1 {
            id: "alchimie".into(),
            nom: "Alchemy + Scouring".into(),
            genre: "alchimie".into(),
            chance: rare,
            monnaies: vec![conso(ALCHEMY, 1.0), conso(SCOURING, 1.0)],
            note: "Orb of Alchemy sur l'objet Normal, Orb of Scouring pour recommencer.".into(),
        });
    } else {
        ecartees.push(Ecartee {
            nom: "Chaos Orb".into(),
            raison: "objectif impossible en un tirage à ce niveau d'objet".into(),
        });
    }

    for e in &pb.essences {
        if !matches!(goal.classify(pool, e.affixe), Class::Wanted(_)) {
            continue;
        }
        let chance = chance_rare(pool, &goal, ilvl, Some(e.affixe));
        if chance <= 0.0 {
            continue;
        }
        let mut monnaies = vec![conso(&e.essence.nom, 1.0)];
        let note = if e.essence.sur_rare {
            "Refait directement l'objet Rare, en gardant le mod de l'Essence.".to_string()
        } else {
            monnaies.push(conso(SCOURING, 1.0));
            "Objet Normal seulement : Orb of Scouring entre deux essais.".to_string()
        };
        methodes.push(MethodePoe1 {
            id: format!("essence:{}", e.essence.id),
            nom: e.essence.nom.clone(),
            genre: "essence".into(),
            chance,
            monnaies,
            note,
        });
    }

    methodes.sort_by(|a, b| b.chance.total_cmp(&a.chance).then(a.nom.cmp(&b.nom)));
    Ok(ResultatPoe1 { methodes, ecartees })
}
