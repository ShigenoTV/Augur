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

use crate::forge::{
    AffixIdx, AffixPool, Class, DrawFilter, Goal, GroupId, ItemState, Mod, Rarity, Slot,
    WantedAffix,
};

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
    /// Craft d'établi qui finit l'objet, payé une fois (pas à chaque essai).
    pub finition: Option<Finition>,
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
    /// Plan au coût moyen minimal (Exalted, Annulment, metamods…), si les prix sont fournis.
    pub plan: Option<super::solveur::PlanPoe1>,
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
    /// Voulu finissable à l'établi : (index du voulu, côté du mod artisanal).
    etabli: Option<(usize, usize)>,
    memo: HashMap<Etat, f64>,
}

impl Rare<'_> {
    /// Objet final réussi : tous les voulus, ou tous sauf celui de l'établi avec une place libre de
    /// son côté (son groupe est libre puisqu'un mauvais tier de ce groupe fait échouer plus tôt).
    fn reussi(&self, e: &Etat) -> bool {
        e.bons == self.complet
            || self
                .etabli
                .is_some_and(|(k, c)| e.bons == self.complet & !(1 << k) && e.n[c] < self.cap[c])
    }

    fn chance(&mut self, e: Etat) -> f64 {
        if e.bons == self.complet {
            return 1.0;
        }
        if e.reste == 0 {
            return if self.reussi(&e) { 1.0 } else { 0.0 };
        }
        if let Some(&v) = self.memo.get(&e) {
            return v;
        }
        let a_tirer = self.complet & !e.bons & !self.etabli.map_or(0, |(k, _)| 1 << k);
        if a_tirer.count_ones() as u8 > e.reste {
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
            if self.reussi(&e) { 1.0 } else { 0.0 }
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

/// Chance qu'un objet Rare refait (Chaos, Alchemy, ou Essence si `garanti`) atteigne l'objectif,
/// éventuellement fini par le craft d'établi `etabli` (mod artisanal accepté pour un des voulus).
pub fn chance_rare(
    pool: &AffixPool,
    goal: &Goal,
    ilvl: u8,
    garanti: Option<AffixIdx>,
    etabli: Option<AffixIdx>,
) -> f64 {
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
    let etabli = match etabli.map(|e| (goal.classify(pool, e), &pool.affixes[e as usize])) {
        None => None,
        Some((Class::Wanted(k), a)) => Some((k, cote(a.slot))),
        Some(_) => return 0.0,
    };
    let (cp, cs) = pool.cap(crate::forge::Rarity::Rare);
    let mut r = Rare {
        p: &p,
        autres: reste_autres(&p, exclu, 3),
        cap: [cp, cs],
        complet: ((1u16 << goal.len()) - 1) as u8,
        etabli,
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

/// Alteration + Augmentation avec finition à l'établi, par énumération exacte des tirages
/// (`AffixPool::distribution`) : chance par essai et Augmentations par essai. L'Augmentation est
/// utilisée quand l'objet n'a qu'un mod et qu'un second mod peut le finir (établi compris).
pub fn chance_alteration_etabli(
    pool: &AffixPool,
    goal: &Goal,
    ilvl: u8,
    etabli: AffixIdx,
) -> Option<(f64, f64)> {
    let mut cotes = [0u8; 2];
    for s in &goal.slots {
        cotes[cote(*s)] += 1;
    }
    if cotes.iter().any(|&c| c > 1) {
        return None;
    }
    let Class::Wanted(kb) = goal.classify(pool, etabli) else {
        return Some((0.0, 0.0));
    };
    let cote_etabli = pool.affixes[etabli as usize].slot;
    let reussi = |item: &ItemState| -> bool {
        if goal.is_met(item) {
            return true;
        }
        let tenus: Vec<Class> = item
            .mods()
            .iter()
            .map(|m| goal.classify(pool, m.idx))
            .collect();
        let manque_seulement_kb = (0..goal.len())
            .all(|k| k == kb || tenus.contains(&Class::Wanted(k)))
            && !tenus.contains(&Class::Blocked(kb));
        // Objet Magique : un préfixe et un suffixe au plus.
        let place = pool.count(item, cote_etabli) == 0;
        manque_seulement_kb && place
    };
    let f = DrawFilter::default();
    let vide = ItemState::new(Rarity::Magic, ilvl);
    let suivants = |item: &ItemState| -> f64 {
        pool.distribution(item, &f)
            .into_iter()
            .filter(|&(j, _)| {
                let mut x = *item;
                x.push(Mod {
                    idx: j,
                    ..Mod::default()
                });
                reussi(&x)
            })
            .map(|(_, p)| p)
            .sum()
    };
    let un = 1.0 - DEUX_MODS_MAGIQUE;
    let (mut ok, mut augs) = (0.0, 0.0);
    for (i, p1) in pool.distribution(&vide, &f) {
        let mut item = vide;
        item.push(Mod {
            idx: i,
            ..Mod::default()
        });
        let s = suivants(&item);
        if reussi(&item) {
            ok += un * p1;
        } else if s > 0.0 {
            augs += un * p1;
            ok += un * p1 * s;
        }
        ok += DEUX_MODS_MAGIQUE * p1 * s;
    }
    Some((ok, augs))
}

/// Craft d'établi qui finit l'objet : mod posé et son coût.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finition {
    /// Texte du mod artisanal.
    pub texte: String,
    /// Monnaies du craft, payées une fois.
    pub monnaies: Vec<Conso>,
}

/// Les méthodes de spam d'un objet, avant finition.
enum Spam<'a> {
    Alteration,
    Rare,
    Essence(&'a super::donnees::EssenceBase),
}

/// Toutes les méthodes de spam applicables, avec et sans finition à l'établi, de la plus probable à
/// la moins probable.
pub fn methodes(pb: &PoolPoe1, voulus: &[WantedAffix]) -> Result<ResultatPoe1, ErreurForgePoe1> {
    let goal = Goal::new(&pb.pool, voulus).map_err(ErreurForgePoe1::Objectif)?;
    let pool = &pb.pool;
    let ilvl = pb.ilvl;
    let mut methodes = Vec::new();
    let mut ecartees = Vec::new();

    // Crafts d'établi acceptés pour un voulu (un seul mod artisanal par objet).
    let crafts: Vec<&super::donnees::EtabliBase> = pb
        .etabli
        .iter()
        .filter(|e| matches!(goal.classify(pool, e.affixe), Class::Wanted(_)))
        .collect();

    let chance = |m: &Spam, etabli: Option<AffixIdx>| -> Option<(f64, f64)> {
        match m {
            Spam::Alteration => match etabli {
                None => chance_alteration(pool, &goal, ilvl),
                Some(e) => chance_alteration_etabli(pool, &goal, ilvl, e),
            },
            Spam::Rare => Some((chance_rare(pool, &goal, ilvl, None, etabli), 0.0)),
            Spam::Essence(e) => Some((chance_rare(pool, &goal, ilvl, Some(e.affixe), etabli), 0.0)),
        }
    };

    let mut spams = vec![Spam::Alteration, Spam::Rare];
    spams.extend(
        pb.essences
            .iter()
            .filter(|e| matches!(goal.classify(pool, e.affixe), Class::Wanted(_)))
            .map(Spam::Essence),
    );

    for spam in &spams {
        let (id, nom, genre, note) = match spam {
            Spam::Alteration => (
                "alteration".to_string(),
                "Alteration + Augmentation".to_string(),
                "alteration",
                "Objet Magique : Alteration jusqu'au bon mod, Augmentation quand il reste une place qui peut finir l'objet.".to_string(),
            ),
            Spam::Rare => (
                "chaos".to_string(),
                "Chaos Orb".to_string(),
                "chaos",
                "Objet Rare : un Chaos Orb refait tous les mods (4 à 6).".to_string(),
            ),
            Spam::Essence(e) => (
                format!("essence:{}", e.essence.id),
                e.essence.nom.clone(),
                "essence",
                if e.essence.sur_rare {
                    "Refait directement l'objet Rare, en gardant le mod de l'Essence.".to_string()
                } else {
                    "Objet Normal seulement : Orb of Scouring entre deux essais.".to_string()
                },
            ),
        };
        let monnaies = |aug: f64| -> Vec<Conso> {
            match spam {
                Spam::Alteration => vec![conso(ALTERATION, 1.0), conso(AUGMENTATION, aug)],
                Spam::Rare => vec![conso(CHAOS, 1.0)],
                Spam::Essence(e) if e.essence.sur_rare => vec![conso(&e.essence.nom, 1.0)],
                Spam::Essence(e) => vec![conso(&e.essence.nom, 1.0), conso(SCOURING, 1.0)],
            }
        };

        let sans = chance(spam, None);
        match sans {
            None => ecartees.push(Ecartee {
                nom: nom.clone(),
                raison: "un objet Magique n'a qu'un préfixe et un suffixe".into(),
            }),
            Some((p, aug)) if p > 0.0 => methodes.push(MethodePoe1 {
                id: id.clone(),
                nom: nom.clone(),
                genre: genre.into(),
                chance: p,
                monnaies: monnaies(aug),
                note: note.clone(),
                finition: None,
            }),
            Some(_) => ecartees.push(Ecartee {
                nom: nom.clone(),
                raison: "objectif impossible en un tirage à ce niveau d'objet".into(),
            }),
        }

        // Meilleure finition à l'établi pour cette méthode, si elle augmente la chance.
        let essence = match spam {
            Spam::Essence(e) => Some(e.affixe),
            _ => None,
        };
        let meilleure = crafts
            .iter()
            .filter(|c| {
                essence.is_none_or(|g| goal.classify(pool, g) != goal.classify(pool, c.affixe))
            })
            .filter_map(|c| chance(spam, Some(c.affixe)).map(|r| (c, r)))
            .max_by(|a, b| a.1.0.total_cmp(&b.1.0));
        if let Some((c, (p, aug))) = meilleure
            && p > sans.map_or(0.0, |s| s.0) * 1.0001
        {
            let a = &pool.affixes[c.affixe as usize];
            methodes.push(MethodePoe1 {
                id: format!("{id}+etabli:{}", a.id),
                nom: format!("{nom} + établi"),
                genre: genre.into(),
                chance: p,
                monnaies: monnaies(aug),
                note: format!("{note} Puis craft à l'établi : {}.", a.text),
                finition: Some(Finition {
                    texte: a.text.clone(),
                    monnaies: c
                        .cout
                        .iter()
                        .map(|(n, q)| conso(n, f64::from(*q)))
                        .collect(),
                }),
            });
        }

        // Alchemy + Scouring : même chance que le Chaos, autres monnaies.
        if matches!(spam, Spam::Rare) {
            let alch: Vec<MethodePoe1> = methodes
                .iter()
                .filter(|m| m.genre == "chaos")
                .map(|m| MethodePoe1 {
                    id: m.id.replacen("chaos", "alchimie", 1),
                    nom: m.nom.replacen("Chaos Orb", "Alchemy + Scouring", 1),
                    genre: "alchimie".into(),
                    monnaies: vec![conso(ALCHEMY, 1.0), conso(SCOURING, 1.0)],
                    note: m.note.replacen(
                        "Objet Rare : un Chaos Orb refait tous les mods (4 à 6).",
                        "Orb of Alchemy sur l'objet Normal, Orb of Scouring pour recommencer.",
                        1,
                    ),
                    ..m.clone()
                })
                .collect();
            methodes.extend(alch);
        }
    }

    methodes.sort_by(|a, b| b.chance.total_cmp(&a.chance).then(a.nom.cmp(&b.nom)));
    Ok(ResultatPoe1 {
        methodes,
        ecartees,
        plan: None,
    })
}
