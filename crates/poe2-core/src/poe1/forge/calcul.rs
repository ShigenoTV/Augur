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
//!   mod garanti compris : hypothèse, à confirmer en jeu) ;
//! - des **fossiles** (dans un Chaotic Resonator de 1 à 4 places) refont l'objet en Rare comme un Chaos
//!   Orb, avec des poids changés par étiquette de mod (règle de Craft of Exile, voir [`pool_fossiles`]).
//!
//! Le cas Magique est exact. Le cas Rare suit chaque mod voulu exactement et résume les autres mods par
//! côté (poids restant moyen après k tirages sans remise, même approximation que le solveur PoE 2) ;
//! un test le compare à une simulation complète sur le moteur exact.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::forge::{
    AffixIdx, AffixPool, Class, DrawFilter, Goal, GroupId, ItemState, Mod, Rarity, Slot,
    WantedAffix,
};

use super::donnees::{ErreurForgePoe1, FossilePoe1, PoolPoe1, bit_etiquette};

/// Loi du nombre de mods d'un Rare refait : (nombre, chance).
pub const MODS_RARE: [(u8, f64); 3] = [(4, 8.0 / 12.0), (5, 3.0 / 12.0), (6, 1.0 / 12.0)];
/// Chance qu'un objet Magique refait ait deux mods.
pub const DEUX_MODS_MAGIQUE: f64 = 0.5;

pub const ALTERATION: &str = "Orb of Alteration";
pub const AUGMENTATION: &str = "Orb of Augmentation";
pub const CHAOS: &str = "Chaos Orb";
pub const ALCHEMY: &str = "Orb of Alchemy";
pub const SCOURING: &str = "Orb of Scouring";
/// Chaotic Resonators par nombre de places (1 à 4).
pub const RESONATEURS: [&str; 4] = [
    "Primitive Chaotic Resonator",
    "Potent Chaotic Resonator",
    "Powerful Chaotic Resonator",
    "Prime Chaotic Resonator",
];
/// Fossiles essayés au plus dans les combinaisons (les plus utiles seuls d'abord).
const FOSSILES_CANDIDATS: usize = 10;

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
    /// `alteration`, `chaos`, `alchimie`, `essence` ou `fossile`.
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
    /// Mods de l'objet de départ tels que la Forge les a compris (`None` : objet neuf).
    pub depart: Option<Vec<crate::forge::suivi::ModLu>>,
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

/// Poids « autres » de chaque côté : celui du groupe le plus lourd, suivi à part (le champ moyen se
/// trompe quand un groupe pèse l'essentiel du côté, Pristine Fossil sur un bouclier), et le poids
/// restant en moyenne des autres groupes après k tirages de ce côté.
fn reste_autres(
    p: &Poids,
    exclu: Option<(GroupId, usize)>,
    kmax: usize,
) -> ([f64; 2], [Vec<f64>; 2]) {
    let mut dominant = [0.0; 2];
    let mut r: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    for (c, rc) in r.iter_mut().enumerate() {
        let mut w: Vec<(GroupId, f64)> = p
            .autres
            .iter()
            .filter(|((g, cc), _)| *cc == c && exclu.is_none_or(|(eg, _)| eg != *g))
            .map(|((g, _), w)| (*g, *w))
            .collect();
        w.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        dominant[c] = w.first().map_or(0.0, |x| x.1);
        let w: Vec<f64> = w.iter().skip(1).map(|x| x.1).collect();
        let s = survie(&w, kmax);
        *rc = s
            .iter()
            .map(|row| row.iter().zip(&w).map(|(a, b)| a * b).sum())
            .collect();
    }
    (dominant, r)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Etat {
    bons: u8,
    n: [u8; 2],
    /// Autres mods de chaque côté, hors groupe dominant.
    autres: [u8; 2],
    /// Côtés (bit par côté) où le groupe « autre » dominant est sur l'objet.
    dominant: u8,
    reste: u8,
}

struct Rare<'a> {
    p: &'a Poids,
    dominant: [f64; 2],
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
            let d = self.dominant[c];
            if e.dominant >> c & 1 == 0 && d > 0.0 {
                total += d;
                let mut s = e;
                s.n[c] += 1;
                s.dominant |= 1 << c;
                s.reste -= 1;
                issues.push((d, Some(s)));
            }
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
        dominant: 0,
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
    let (dominant, autres) = reste_autres(&p, exclu, 3);
    let mut r = Rare {
        p: &p,
        dominant,
        autres,
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

/// Poids d'un pool sous des fossiles, multipliés par 100 pour garder les fractions.
///
/// Règle de l'émulateur Craft of Exile pour PoE 1 (mode par défaut, « additif ») :
///
/// - par étiquette, un multiplicateur nul d'un fossile la bloque ; les multiplicateurs au-dessus de 1
///   s'**additionnent** d'un fossile à l'autre, ceux en dessous de 1 se multiplient ;
/// - par mod, une étiquette bloquée annule son poids ; sinon le poids est multiplié par la **somme** des
///   bonus de ses étiquettes (1 sans bonus) et par le **produit** de leurs malus.
pub fn pool_fossiles(pool: &AffixPool, fossiles: &[&FossilePoe1]) -> AffixPool {
    let mut bloque = 0u64;
    let mut plus: HashMap<u64, f64> = HashMap::new();
    let mut moins: HashMap<u64, f64> = HashMap::new();
    for f in fossiles {
        for (tag, w) in &f.poids {
            let bit = bit_etiquette(tag);
            let m = f64::from(*w) / 100.0;
            if *w == 0 {
                bloque |= bit;
            } else if m > 1.0 {
                *plus.entry(bit).or_default() += m;
            } else if m < 1.0 {
                *moins.entry(bit).or_insert(1.0) *= m;
            }
        }
    }
    let mut p = pool.clone();
    for a in &mut p.affixes {
        if a.tags & bloque != 0 {
            a.weight = 0;
            continue;
        }
        let bonus: f64 = plus
            .iter()
            .filter(|(b, _)| a.tags & **b != 0)
            .map(|(_, m)| m)
            .sum();
        let malus: f64 = moins
            .iter()
            .filter(|(b, _)| a.tags & **b != 0)
            .map(|(_, m)| m)
            .product();
        let bonus = if bonus > 0.0 { bonus } else { 1.0 };
        a.weight = (f64::from(a.weight) * 100.0 * bonus * malus).round() as u32;
    }
    p
}

/// Pool d'une base sous des fossiles, avec leurs effets spéciaux (règles de l'émulateur Craft of Exile
/// pour PoE 1, `fossils`, `fossilsParameters`, `getFilteredModPool`), et le mod qu'ils forcent :
///
/// - **Faceted** : ses mods `delve` (niveau des gemmes) entrent dans le tirage avec leur vrai poids sur
///   la base, puis les poids par étiquette s'appliquent comme aux autres mods ;
/// - **Sanctified** : chaque poids devient `poids × (1 + (niveau du mod − 40) / 100)` avant les
///   étiquettes (les mods de haut niveau sortent plus souvent, ceux sous le niveau 40 moins) ;
/// - **Hollow** : l'Abyssal Socket est posé d'abord, le reste de l'objet est tiré ensuite (comme le mod
///   garanti d'une Essence). Seulement sur une base de ses étiquettes permises (arme, casque, bottes,
///   gants, armure).
///
/// `None` si un fossile ne s'utilise pas sur cette base.
pub fn pool_fossiles_base(
    pb: &PoolPoe1,
    fossiles: &[&FossilePoe1],
) -> Option<(AffixPool, Option<AffixIdx>)> {
    let tags = &pb.base.tags;
    if fossiles
        .iter()
        .any(|f| !f.permis.is_empty() && !f.permis.iter().any(|t| tags.contains(t)))
    {
        return None;
    }
    let mut pool = pb.pool.clone();
    let mut force = None;
    for f in fossiles {
        for (i, w) in &pb.mods_fossiles {
            let a = &mut pool.affixes[*i as usize];
            if f.ajoutes.contains(&a.id) {
                a.weight = *w;
            }
            if f.forces.contains(&a.id) {
                force = Some(*i);
            }
        }
        if !f.forces.is_empty() && force.is_none() {
            return None;
        }
    }
    if fossiles.iter().any(|f| f.chanceux) {
        for a in &mut pool.affixes {
            let m = 1.0 + (f64::from(a.req_ilvl) - 40.0) / 100.0;
            a.weight = (f64::from(a.weight) * m).round().max(0.0) as u32;
        }
    }
    Some((pool_fossiles(&pool, fossiles), force))
}

/// Méthode Alteration (+ Augmentation quand elle peut finir l'objet) : chance par essai et nombre
/// moyen d'Augmentations par essai. `None` si l'objectif ne tient pas sur un objet Magique.
pub fn chance_alteration(pool: &AffixPool, goal: &Goal, ilvl: u8) -> Option<(f64, f64)> {
    if pool.cap_delta != (0, 0) {
        return chance_alteration_plafond(pool, goal, ilvl);
    }
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

/// Alteration + Augmentation sur une base à plafond décalé (bases expérimentales de Heist : Simplex
/// Amulet sans aucun mod Magique, Composite Ring à deux préfixes…), par énumération exacte des
/// tirages (`AffixPool::distribution`, qui suit le plafond de la base).
pub(super) fn chance_alteration_plafond(
    pool: &AffixPool,
    goal: &Goal,
    ilvl: u8,
) -> Option<(f64, f64)> {
    if goal.len() > 2 {
        return None;
    }
    let f = DrawFilter::default();
    let vide = ItemState::new(Rarity::Magic, ilvl);
    let avec = |item: &ItemState, j: AffixIdx| {
        let mut x = *item;
        x.push(Mod {
            idx: j,
            ..Mod::default()
        });
        x
    };
    // Chance que le second mod finisse l'objet ; sans place pour un second mod, l'objet reste à un.
    let suivants = |item: &ItemState| -> f64 {
        let d = pool.distribution(item, &f);
        if d.is_empty() {
            return if goal.is_met(item) { 1.0 } else { 0.0 };
        }
        d.into_iter()
            .filter(|&(j, _)| goal.is_met(&avec(item, j)))
            .map(|(_, p)| p)
            .sum()
    };
    let un = 1.0 - DEUX_MODS_MAGIQUE;
    let (mut ok, mut augs) = (0.0, 0.0);
    for (i, p1) in pool.distribution(&vide, &f) {
        let item = avec(&vide, i);
        let s = suivants(&item);
        if goal.is_met(&item) {
            ok += un * p1;
        } else if s > 0.0 {
            augs += un * p1;
            ok += un * p1 * s;
        }
        ok += DEUX_MODS_MAGIQUE * p1 * s;
    }
    Some((ok, augs))
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
        // Objet Magique : un préfixe et un suffixe au plus (plafond décalé sur une base
        // expérimentale de Heist).
        let (cp, cs) = pool.cap(Rarity::Magic);
        let cap = if cote_etabli == Slot::Prefix { cp } else { cs };
        let place = pool.count(item, cote_etabli) < cap;
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
    /// Fossiles dans un Chaotic Resonator, avec le pool aux poids changés et le mod forcé.
    Fossiles(Vec<&'a FossilePoe1>, AffixPool, Option<AffixIdx>),
}

/// Combinaisons de `k` éléments parmi `n` (indices croissants).
fn combinaisons(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn rec(n: usize, k: usize, debut: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in debut..n {
            cur.push(i);
            rec(n, k, i + 1, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    rec(n, k, 0, &mut Vec::new(), &mut out);
    out
}

/// Combinaison de fossiles retenue : fossiles, pool aux poids changés, mod forcé, chance d'un essai.
pub(super) type Choix<'a> = (Vec<&'a FossilePoe1>, AffixPool, Option<AffixIdx>, f64);

/// Meilleure combinaison de fossiles pour chaque taille de résonateur (1 à 4 places) : coût moyen
/// minimal quand tous les prix sont connus, sinon chance maximale. Seuls les fossiles qui, seuls,
/// améliorent la chance du Chaos Orb sont combinés (les `FOSSILES_CANDIDATS` meilleurs).
pub(super) fn meilleurs_fossiles<'a>(
    pb: &PoolPoe1,
    goal: &Goal,
    fossiles: &'a [FossilePoe1],
    prix: &BTreeMap<String, f64>,
) -> Vec<Choix<'a>> {
    let (pool, ilvl) = (&pb.pool, pb.ilvl);
    let base = chance_rare(pool, goal, ilvl, None, None);
    let essai = |fs: &[&FossilePoe1]| -> Option<(AffixPool, Option<AffixIdx>, f64)> {
        let (p, force) = pool_fossiles_base(pb, fs)?;
        let chance = chance_rare(&p, goal, ilvl, force, None);
        Some((p, force, chance))
    };
    let mut seuls: Vec<(&FossilePoe1, f64)> = fossiles
        .iter()
        .filter_map(|f| Some((f, essai(&[f])?.2)))
        .filter(|(_, p)| *p > base * 1.0001)
        .collect();
    seuls.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.nom.cmp(&b.0.nom)));
    seuls.truncate(FOSSILES_CANDIDATS);
    let mut sortie = Vec::new();
    for (k, reso) in RESONATEURS.iter().enumerate() {
        let mut meilleur: Option<((u8, f64), Choix)> = None;
        for c in combinaisons(seuls.len(), k + 1) {
            let fs: Vec<&FossilePoe1> = c.iter().map(|&i| seuls[i].0).collect();
            let Some((p, force, chance)) = essai(&fs) else {
                continue;
            };
            if chance <= 0.0 {
                continue;
            }
            let cout: Option<f64> = fs
                .iter()
                .map(|f| prix.get(&f.nom).copied())
                .chain([prix.get(*reso).copied()])
                .sum();
            // Combinaisons aux prix connus d'abord, au coût moyen ; les autres à la chance.
            let note = (
                u8::from(cout.is_none()),
                cout.map_or(-chance, |c| c / chance),
            );
            if meilleur
                .as_ref()
                .is_none_or(|(m, _)| note.0 < m.0 || (note.0 == m.0 && note.1 < m.1))
            {
                meilleur = Some((note, (fs, p, force, chance)));
            }
        }
        if let Some((_, choix)) = meilleur {
            sortie.push(choix);
        }
    }
    sortie
}

/// Toutes les méthodes de spam applicables, avec et sans finition à l'établi, de la plus probable à
/// la moins probable.
pub fn methodes(
    pb: &PoolPoe1,
    voulus: &[WantedAffix],
    prix: &BTreeMap<String, f64>,
) -> Result<ResultatPoe1, ErreurForgePoe1> {
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
            Spam::Fossiles(_, p, force) => Some((chance_rare(p, &goal, ilvl, *force, etabli), 0.0)),
        }
    };

    let mut spams = vec![Spam::Alteration, Spam::Rare];
    spams.extend(
        pb.essences
            .iter()
            .filter(|e| matches!(goal.classify(pool, e.affixe), Class::Wanted(_)))
            .map(Spam::Essence),
    );
    let fossiles = &super::donnees::donnees()?.fossiles;
    spams.extend(
        meilleurs_fossiles(pb, &goal, fossiles, prix)
            .into_iter()
            .map(|(fs, p, force, _)| Spam::Fossiles(fs, p, force)),
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
            Spam::Fossiles(fs, _, _) => {
                let noms: Vec<&str> = fs.iter().map(|f| f.nom.as_str()).collect();
                (
                    format!("fossiles:{}", noms.join("+")),
                    noms.iter()
                        .map(|n| n.trim_end_matches(" Fossil"))
                        .collect::<Vec<_>>()
                        .join(" + ")
                        + if fs.len() > 1 { " (fossiles)" } else { " (fossile)" },
                    "fossile",
                    format!(
                        "{} avec {} : refait l'objet (Normal ou Rare) en Rare, 4 à 6 mods.{}",
                        RESONATEURS[fs.len() - 1],
                        noms.join(", "),
                        if fs.iter().any(|f| !f.forces.is_empty()) {
                            " L'Abyssal Socket est posé d'abord."
                        } else {
                            ""
                        }
                    ),
                )
            }
        };
        let monnaies = |aug: f64| -> Vec<Conso> {
            match spam {
                Spam::Alteration => vec![conso(ALTERATION, 1.0), conso(AUGMENTATION, aug)],
                Spam::Rare => vec![conso(CHAOS, 1.0)],
                Spam::Essence(e) if e.essence.sur_rare => vec![conso(&e.essence.nom, 1.0)],
                Spam::Essence(e) => vec![conso(&e.essence.nom, 1.0), conso(SCOURING, 1.0)],
                Spam::Fossiles(fs, _, _) => fs
                    .iter()
                    .map(|f| conso(&f.nom, 1.0))
                    .chain([conso(RESONATEURS[fs.len() - 1], 1.0)])
                    .collect(),
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
        depart: None,
    })
}
