//! Solveur PoE 1 : l'enchaînement de monnaies au coût moyen minimal pour atteindre l'objectif.
//!
//! On cherche la meilleure décision dans chaque situation, et pas seulement la meilleure méthode de
//! spam. Les actions proposées sont :
//!
//! - les spams : Transmutation, Alteration, Alchemy, Chaos, Essences ;
//! - les ajouts : Augmentation, Regal, Exalted ;
//! - les retraits : Annulment, Scouring ;
//! - les metamods de l'établi : « Prefixes / Suffixes Cannot Be Changed » et
//!   « Cannot roll Attack / Caster Modifiers » ;
//! - le retrait du craft (1 Orb of Scouring) et le craft d'établi final.
//!
//! Règles des metamods, relevées dans le code de l'émulateur Craft of Exile pour PoE 1 :
//!
//! - **Chaos** sur un objet aux préfixes (suffixes) verrouillés : on garde ce côté, et le reste de
//!   l'objet est refait. On tire 4, 5 ou 6 mods au total, moins les mods gardés ; le metamod part.
//! - **Scouring** avec verrou : on garde le côté verrouillé et on retire le reste, metamod compris.
//!   L'objet devient Normal (0 mod), Magique (1 mod) ou Rare (2 mods ou plus).
//! - **Annulment** : retire un mod au hasard (chances égales) parmi ceux qui ne sont pas verrouillés,
//!   metamod compris.
//! - **Cannot roll Attack / Caster Modifiers** : les ajouts (Exalted, Regal, Augmentation) ne tirent
//!   plus les mods qui portent le tag `attack` / `caster`.
//!
//! On ne pose qu'un seul mod d'établi à la fois : pas de « Can have up to 3 Crafted Modifiers ».
//!
//! Comme pour les spams Rare, chaque mod voulu est suivi exactement, et les autres mods sont résumés
//! par leur nombre de chaque côté (poids restant moyen). Le problème est un plus court chemin
//! stochastique, où l'on minimise le coût moyen. Les relances (Chaos, Alteration, Essence…) mènent
//! toujours à la même loi d'objets, quel que soit l'objet de départ. Chaque loi est donc un « nœud »
//! dont la valeur est résolue exactement par itération de politique : une évaluation affine, puis un
//! système linéaire entre les nœuds. Les boucles locales (Exalted puis Annulment) sont traitées par
//! Gauss-Seidel.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::forge::{AffixPool, Class, Goal, GroupId, Rarity, Slot};

use super::calcul::{
    ALCHEMY, ALTERATION, AUGMENTATION, CHAOS, Conso, DEUX_MODS_MAGIQUE, MODS_RARE, SCOURING,
};
use super::donnees::{PoolPoe1, TAG_ATTAQUE, TAG_SORT};

/// Nombre de mods voulus au plus pour le solveur : au-delà, le calcul devient trop long pour l'appli
/// (moins d'une seconde à 4 mods ; à 5 ou 6, la recherche d'une première politique qui aboutit échoue).
pub const VOULUS_MAX: usize = 4;

pub const TRANSMUTATION: &str = "Orb of Transmutation";
pub const REGAL: &str = "Regal Orb";
pub const EXALTED: &str = "Exalted Orb";
pub const ANNULMENT: &str = "Orb of Annulment";

/// Metamods de l'établi, par id de mod RePoE.
const METAMODS: [(&str, Meta); 4] = [
    (
        "StrMasterItemGenerationCannotChangePrefixes",
        Meta::VerrouPrefixes,
    ),
    (
        "DexMasterItemGenerationCannotChangeSuffixes",
        Meta::VerrouSuffixes,
    ),
    (
        "IntMasterItemGenerationCannotRollAttackAffixes",
        Meta::SansAttaque,
    ),
    (
        "StrDexMasterItemGenerationCannotRollCasterAffixes",
        Meta::SansSort,
    ),
];

/// Plan conseillé : coût moyen minimal et ce qu'il consomme en moyenne.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPoe1 {
    /// Coût moyen jusqu'à l'objet fini, en Chaos (prix fournis par l'appli).
    pub cout_moyen: f64,
    /// Première action, sur l'objet Normal.
    pub depart: String,
    /// Actions utilisées, avec leur nombre moyen, de la plus fréquente à la moins fréquente.
    pub actions: Vec<UsageAction>,
    /// Monnaies consommées en moyenne.
    pub monnaies: Vec<Conso>,
    /// Monnaies sans prix : les actions qui les utilisent sont écartées.
    pub sans_prix: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAction {
    pub action: String,
    pub nombre: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
enum Rarete {
    Normal,
    Magique,
    Rare,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
enum Meta {
    Aucun,
    VerrouPrefixes,
    VerrouSuffixes,
    SansAttaque,
    SansSort,
}

impl Meta {
    /// Côté occupé par le metamod (0 préfixe, 1 suffixe).
    fn cote(self) -> Option<usize> {
        match self {
            Meta::Aucun => None,
            Meta::VerrouSuffixes => Some(0),
            Meta::VerrouPrefixes | Meta::SansAttaque | Meta::SansSort => Some(1),
        }
    }
    /// Côté protégé par le metamod.
    fn verrou(self) -> Option<usize> {
        match self {
            Meta::VerrouPrefixes => Some(0),
            Meta::VerrouSuffixes => Some(1),
            _ => None,
        }
    }
    fn variante(self) -> usize {
        match self {
            Meta::SansAttaque => 1,
            Meta::SansSort => 2,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
struct Etat {
    rarete: Rarete,
    /// Voulus présents.
    bons: u8,
    /// Voulus dont le groupe est pris par un mauvais tier.
    bloques: u8,
    /// Autres mods, par côté.
    autres: [u8; 2],
    meta: Meta,
}

impl Etat {
    fn vide(rarete: Rarete) -> Self {
        Etat {
            rarete,
            bons: 0,
            bloques: 0,
            autres: [0, 0],
            meta: Meta::Aucun,
        }
    }
}

fn cote(s: Slot) -> usize {
    match s {
        Slot::Prefix => 0,
        Slot::Suffix => 1,
    }
}

/// Poids tirables d'une variante (sans filtre, sans attaque, sans sort).
struct Tirage {
    /// Par voulu, sur son côté : poids accepté et poids d'un mauvais tier.
    bon: Vec<f64>,
    mauvais: Vec<f64>,
    /// Poids « autres » restant en moyenne de chaque côté, selon le nombre d'autres déjà tirés.
    autres: [Vec<f64>; 2],
}

fn tirage(pool: &AffixPool, goal: &Goal, ilvl: u8, interdit: u64) -> Tirage {
    let n = goal.len();
    let mut bon = vec![0.0; n];
    let mut mauvais = vec![0.0; n];
    let mut autres: [HashMap<GroupId, f64>; 2] = [HashMap::new(), HashMap::new()];
    for (i, a) in pool.affixes.iter().enumerate() {
        if a.weight == 0 || a.req_ilvl > ilvl || a.tags & interdit != 0 {
            continue;
        }
        let w = f64::from(a.weight);
        match goal.classify(pool, i as u16) {
            Class::Wanted(k) if cote(a.slot) == cote(goal.slots[k]) => bon[k] += w,
            Class::Wanted(k) | Class::Blocked(k) => mauvais[k] += w,
            Class::Other => *autres[cote(a.slot)].entry(a.group).or_default() += w,
        }
    }
    let reste = |m: &HashMap<GroupId, f64>| -> Vec<f64> {
        let w: Vec<f64> = m.values().copied().collect();
        survie(&w, 3)
            .iter()
            .map(|row| row.iter().zip(&w).map(|(a, b)| a * b).sum())
            .collect()
    };
    Tirage {
        bon,
        mauvais,
        autres: [reste(&autres[0]), reste(&autres[1])],
    }
}

/// P(groupe j encore absent après k tirages pondérés sans remise), k = 0..=kmax.
fn survie(w: &[f64], kmax: usize) -> Vec<Vec<f64>> {
    // Récurrence sur l'ensemble tiré, limitée à kmax tirages : on garde la probabilité d'être dedans.
    let total: f64 = w.iter().sum();
    let mut dedans = vec![vec![0.0; w.len()]; kmax + 1];
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
            if prof < kmax {
                for row in dedans.iter_mut().skip(prof + 1) {
                    for &j in pris.iter() {
                        row[j] += p;
                    }
                }
            }
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
    dfs(w, 0, kmax, &mut Vec::new(), total, 1.0, &mut dedans);
    dedans
        .into_iter()
        .map(|r| r.into_iter().map(|x| 1.0 - x).collect())
        .collect()
}

/// Coût d'un craft d'établi : (monnaie, quantité).
type CoutEtabli = Vec<(String, u32)>;
/// Coût par action, pour évaluer une politique.
type CoutAction<'a> = &'a dyn Fn(&Action) -> f64;
type FonctionCout = Box<dyn Fn(&Action) -> f64>;
/// Valeurs des états et des nœuds pour un coût.
type Valeurs = (Vec<f64>, Vec<f64>);

/// Où mène une action.
#[derive(Clone, Debug)]
enum Issue {
    Succes,
    Noeud(usize),
    Etats(Vec<(usize, f64)>),
}

/// Nature d'une action (le côté verrouillé se lit sur l'état).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Genre {
    Transmutation,
    Alteration,
    Augmentation,
    Regal,
    Alchemy,
    Chaos,
    Exalted,
    Annulment,
    Scouring,
    Essence(usize),
    Meta(Meta),
    RetirerMeta,
    Finition,
}

#[derive(Clone, Debug)]
struct Action {
    #[cfg_attr(not(test), allow(dead_code))]
    genre: Genre,
    libelle: usize,
    cout: f64,
    /// (index de monnaie, quantité).
    conso: Vec<(usize, f64)>,
    issue: Issue,
}

/// Une loi d'objets commune à plusieurs actions de relance.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Cle {
    Magique,
    Rare,
    Essence(usize),
    /// Chaos avec un côté verrouillé : (côté gardé, voulus gardés, mauvais tiers gardés, autres gardés).
    Verrou(usize, u8, u8, u8),
}

struct Modele<'a> {
    goal: &'a Goal,
    complet: u8,
    cotes: Vec<usize>,
    cap_magique: [u8; 2],
    cap_rare: [u8; 2],
    tirages: [Tirage; 3],
    prix: &'a BTreeMap<String, f64>,
    /// Essences utiles : (nom, voulu garanti, sur un Rare, mod garanti).
    #[cfg_attr(not(test), allow(dead_code))]
    essences: Vec<(String, usize, bool, u16)>,
    /// Metamods disponibles : (meta, texte, coût).
    metamods: Vec<(Meta, String, CoutEtabli)>,
    /// Craft final le moins cher par voulu : (texte, coût).
    finitions: Vec<Option<(String, CoutEtabli)>>,

    etats: Vec<Etat>,
    index: HashMap<Etat, usize>,
    noeuds: Vec<Vec<(usize, f64)>>,
    index_noeuds: HashMap<Cle, usize>,
    libelles: Vec<String>,
    index_libelles: HashMap<String, usize>,
    monnaies: Vec<String>,
    index_monnaies: HashMap<String, usize>,
    sans_prix: Vec<String>,
}

impl<'a> Modele<'a> {
    fn n(&self, e: &Etat, c: usize) -> u8 {
        let voulus = (0..self.goal.len())
            .filter(|&k| self.cotes[k] == c && (e.bons | e.bloques) & (1 << k) != 0)
            .count() as u8;
        voulus + e.autres[c] + u8::from(e.meta.cote() == Some(c))
    }

    fn cap(&self, r: Rarete) -> [u8; 2] {
        match r {
            Rarete::Magique => self.cap_magique,
            _ => self.cap_rare,
        }
    }

    /// Issues d'un mod ajouté : (poids, état). Vide si rien ne peut être ajouté.
    fn un_mod(&self, e: &Etat, variante: usize) -> Vec<(f64, Etat)> {
        let t = &self.tirages[variante];
        let cap = self.cap(e.rarete);
        let ouvert = [self.n(e, 0) < cap[0], self.n(e, 1) < cap[1]];
        let mut issues = Vec::new();
        for k in 0..self.goal.len() {
            if (e.bons | e.bloques) & (1 << k) != 0 || !ouvert[self.cotes[k]] {
                continue;
            }
            if t.bon[k] > 0.0 {
                let mut s = *e;
                s.bons |= 1 << k;
                issues.push((t.bon[k], s));
            }
            if t.mauvais[k] > 0.0 {
                let mut s = *e;
                s.bloques |= 1 << k;
                issues.push((t.mauvais[k], s));
            }
        }
        for (c, _) in ouvert.iter().enumerate().filter(|(_, o)| **o) {
            let w = t.autres[c]
                .get(e.autres[c] as usize)
                .copied()
                .unwrap_or(0.0)
                .max(0.0);
            if w > 0.0 {
                let mut s = *e;
                s.autres[c] += 1;
                issues.push((w, s));
            }
        }
        issues
    }

    /// Loi des objets après `n` mods ajoutés l'un après l'autre (arrêt si plus rien ne peut entrer).
    fn tirer(&self, e: Etat, n: u8, variante: usize, p: f64, sortie: &mut HashMap<Etat, f64>) {
        if n == 0 {
            *sortie.entry(e).or_default() += p;
            return;
        }
        let issues = self.un_mod(&e, variante);
        let total: f64 = issues.iter().map(|x| x.0).sum();
        if total <= 0.0 {
            *sortie.entry(e).or_default() += p;
            return;
        }
        for (w, s) in issues {
            self.tirer(s, n - 1, variante, p * w / total, sortie);
        }
    }

    fn loi(&self, cle: &Cle) -> HashMap<Etat, f64> {
        let mut sortie = HashMap::new();
        match *cle {
            Cle::Magique => {
                let vide = Etat::vide(Rarete::Magique);
                self.tirer(vide, 1, 0, 1.0 - DEUX_MODS_MAGIQUE, &mut sortie);
                self.tirer(vide, 2, 0, DEUX_MODS_MAGIQUE, &mut sortie);
            }
            Cle::Rare => {
                for (n, p) in MODS_RARE {
                    self.tirer(Etat::vide(Rarete::Rare), n, 0, p, &mut sortie);
                }
            }
            Cle::Essence(i) => {
                let k = self.essences[i].1;
                let mut e = Etat::vide(Rarete::Rare);
                e.bons = 1 << k;
                for (n, p) in MODS_RARE {
                    self.tirer(e, n - 1, 0, p, &mut sortie);
                }
            }
            Cle::Verrou(c, bons, bloques, autres) => {
                let mut e = Etat::vide(Rarete::Rare);
                e.bons = bons;
                e.bloques = bloques;
                e.autres[c] = autres;
                let gardes = self.n(&e, c);
                for (n, p) in MODS_RARE {
                    self.tirer(e, n.saturating_sub(gardes), 0, p, &mut sortie);
                }
            }
        }
        sortie
    }

    fn libelle(&mut self, l: &str) -> usize {
        if let Some(&i) = self.index_libelles.get(l) {
            return i;
        }
        self.libelles.push(l.to_string());
        self.index_libelles
            .insert(l.to_string(), self.libelles.len() - 1);
        self.libelles.len() - 1
    }

    /// Coût et consommation d'une liste de monnaies ; `None` si une monnaie n'a pas de prix.
    fn payer(&mut self, monnaies: &[(&str, f64)]) -> Option<(f64, Vec<(usize, f64)>)> {
        let mut cout = 0.0;
        let mut conso = Vec::new();
        for &(nom, q) in monnaies {
            let Some(&p) = self.prix.get(nom) else {
                if !self.sans_prix.iter().any(|x| x == nom) {
                    self.sans_prix.push(nom.to_string());
                }
                return None;
            };
            cout += p * q;
            let i = match self.index_monnaies.get(nom) {
                Some(&i) => i,
                None => {
                    self.monnaies.push(nom.to_string());
                    self.index_monnaies
                        .insert(nom.to_string(), self.monnaies.len() - 1);
                    self.monnaies.len() - 1
                }
            };
            conso.push((i, q));
        }
        Some((cout, conso))
    }

    fn etat(&mut self, e: Etat, a_visiter: &mut Vec<usize>) -> usize {
        if let Some(&i) = self.index.get(&e) {
            return i;
        }
        self.etats.push(e);
        let i = self.etats.len() - 1;
        self.index.insert(e, i);
        a_visiter.push(i);
        i
    }

    fn noeud(&mut self, cle: Cle, a_visiter: &mut Vec<usize>) -> usize {
        if let Some(&h) = self.index_noeuds.get(&cle) {
            return h;
        }
        let mut loi: Vec<(Etat, f64)> = self.loi(&cle).into_iter().collect();
        loi.sort_by_key(|a| a.0);
        let loi = loi
            .into_iter()
            .map(|(e, p)| (self.etat(e, a_visiter), p))
            .collect();
        self.noeuds.push(loi);
        self.index_noeuds.insert(cle, self.noeuds.len() - 1);
        self.noeuds.len() - 1
    }

    fn reussi(&self, e: &Etat) -> bool {
        e.bons == self.complet
    }

    /// Actions possibles dans un état (vide si l'objectif est atteint).
    fn actions(&mut self, i: usize, a_visiter: &mut Vec<usize>) -> Vec<Action> {
        let e = self.etats[i];
        let mut out = Vec::new();
        if self.reussi(&e) {
            return out;
        }
        let mut ajouter =
            |m: &mut Self, genre: Genre, libelle: &str, monnaies: &[(&str, f64)], issue: Issue| {
                if let Some((cout, conso)) = m.payer(monnaies) {
                    let libelle = m.libelle(libelle);
                    out.push(Action {
                        genre,
                        libelle,
                        cout,
                        conso,
                        issue,
                    });
                }
            };
        let vers = |m: &mut Self, loi: Vec<(f64, Etat)>, a_visiter: &mut Vec<usize>| -> Issue {
            let total: f64 = loi.iter().map(|x| x.0).sum();
            let mut fusion: BTreeMap<Etat, f64> = BTreeMap::new();
            for (w, s) in loi {
                *fusion.entry(s).or_default() += w / total;
            }
            Issue::Etats(
                fusion
                    .into_iter()
                    .map(|(s, p)| (m.etat(s, a_visiter), p))
                    .collect(),
            )
        };
        let nb_mods = |m: &Self, e: &Etat| m.n(e, 0) + m.n(e, 1);

        match e.rarete {
            Rarete::Normal => {
                let h = self.noeud(Cle::Magique, a_visiter);
                ajouter(
                    self,
                    Genre::Transmutation,
                    TRANSMUTATION,
                    &[(TRANSMUTATION, 1.0)],
                    Issue::Noeud(h),
                );
                let h = self.noeud(Cle::Rare, a_visiter);
                ajouter(
                    self,
                    Genre::Alchemy,
                    ALCHEMY,
                    &[(ALCHEMY, 1.0)],
                    Issue::Noeud(h),
                );
                for j in 0..self.essences.len() {
                    let nom = self.essences[j].0.clone();
                    let h = self.noeud(Cle::Essence(j), a_visiter);
                    ajouter(
                        self,
                        Genre::Essence(j),
                        &nom,
                        &[(nom.as_str(), 1.0)],
                        Issue::Noeud(h),
                    );
                }
            }
            Rarete::Magique => {
                let h = self.noeud(Cle::Magique, a_visiter);
                ajouter(
                    self,
                    Genre::Alteration,
                    ALTERATION,
                    &[(ALTERATION, 1.0)],
                    Issue::Noeud(h),
                );
                let un = self.un_mod(&e, 0);
                if !un.is_empty() {
                    let issue = vers(self, un, a_visiter);
                    ajouter(
                        self,
                        Genre::Augmentation,
                        AUGMENTATION,
                        &[(AUGMENTATION, 1.0)],
                        issue,
                    );
                }
                let mut r = e;
                r.rarete = Rarete::Rare;
                let un = self.un_mod(&r, 0);
                if !un.is_empty() {
                    let issue = vers(self, un, a_visiter);
                    ajouter(self, Genre::Regal, REGAL, &[(REGAL, 1.0)], issue);
                }
                let i0 = self.etat(Etat::vide(Rarete::Normal), a_visiter);
                ajouter(
                    self,
                    Genre::Scouring,
                    SCOURING,
                    &[(SCOURING, 1.0)],
                    Issue::Etats(vec![(i0, 1.0)]),
                );
                if nb_mods(self, &e) > 0 {
                    let issue = self.annuler(&e, a_visiter);
                    ajouter(
                        self,
                        Genre::Annulment,
                        ANNULMENT,
                        &[(ANNULMENT, 1.0)],
                        issue,
                    );
                }
            }
            Rarete::Rare => {
                // Ajout.
                let un = self.un_mod(&e, e.meta.variante());
                if !un.is_empty() {
                    let issue = vers(self, un, a_visiter);
                    ajouter(self, Genre::Exalted, EXALTED, &[(EXALTED, 1.0)], issue);
                }
                // Relance.
                match e.meta.verrou() {
                    Some(c) => {
                        let masque = self.masque(c);
                        let cle = Cle::Verrou(c, e.bons & masque, e.bloques & masque, e.autres[c]);
                        let h = self.noeud(cle, a_visiter);
                        let libelle = format!("{CHAOS} ({})", verrou_texte(c));
                        ajouter(
                            self,
                            Genre::Chaos,
                            &libelle,
                            &[(CHAOS, 1.0)],
                            Issue::Noeud(h),
                        );
                        // Scouring : garde le côté verrouillé.
                        let mut s = Etat::vide(Rarete::Normal);
                        s.bons = e.bons & masque;
                        s.bloques = e.bloques & masque;
                        s.autres[c] = e.autres[c];
                        s.rarete = match self.n(&s, c) {
                            0 => Rarete::Normal,
                            1 => Rarete::Magique,
                            _ => Rarete::Rare,
                        };
                        let is = self.etat(s, a_visiter);
                        let libelle = format!("{SCOURING} ({})", verrou_texte(c));
                        ajouter(
                            self,
                            Genre::Scouring,
                            &libelle,
                            &[(SCOURING, 1.0)],
                            Issue::Etats(vec![(is, 1.0)]),
                        );
                    }
                    None => {
                        let h = self.noeud(Cle::Rare, a_visiter);
                        ajouter(self, Genre::Chaos, CHAOS, &[(CHAOS, 1.0)], Issue::Noeud(h));
                        let i0 = self.etat(Etat::vide(Rarete::Normal), a_visiter);
                        ajouter(
                            self,
                            Genre::Scouring,
                            SCOURING,
                            &[(SCOURING, 1.0)],
                            Issue::Etats(vec![(i0, 1.0)]),
                        );
                        for j in 0..self.essences.len() {
                            if !self.essences[j].2 {
                                continue;
                            }
                            let nom = self.essences[j].0.clone();
                            let h = self.noeud(Cle::Essence(j), a_visiter);
                            ajouter(
                                self,
                                Genre::Essence(j),
                                &nom,
                                &[(nom.as_str(), 1.0)],
                                Issue::Noeud(h),
                            );
                        }
                    }
                }
                // Retrait d'un mod.
                let issue = self.annuler(&e, a_visiter);
                if let Issue::Etats(v) = &issue
                    && !v.is_empty()
                {
                    let libelle = match e.meta.verrou() {
                        Some(c) => format!("{ANNULMENT} ({})", verrou_texte(c)),
                        None => ANNULMENT.to_string(),
                    };
                    ajouter(self, Genre::Annulment, &libelle, &[(ANNULMENT, 1.0)], issue);
                }
                // Metamods.
                if e.meta == Meta::Aucun {
                    let cap = self.cap_rare;
                    for j in 0..self.metamods.len() {
                        let (meta, texte, cout) = self.metamods[j].clone();
                        let c = meta.cote().unwrap_or(1);
                        if self.n(&e, c) >= cap[c] {
                            continue;
                        }
                        let mut s = e;
                        s.meta = meta;
                        let is = self.etat(s, a_visiter);
                        let monnaies: Vec<(&str, f64)> = cout
                            .iter()
                            .map(|(n, q)| (n.as_str(), f64::from(*q)))
                            .collect();
                        ajouter(
                            self,
                            Genre::Meta(meta),
                            &format!("Établi : {texte}"),
                            &monnaies,
                            Issue::Etats(vec![(is, 1.0)]),
                        );
                    }
                } else {
                    let mut s = e;
                    s.meta = Meta::Aucun;
                    let is = self.etat(s, a_visiter);
                    ajouter(
                        self,
                        Genre::RetirerMeta,
                        "Établi : retirer le craft",
                        &[(SCOURING, 1.0)],
                        Issue::Etats(vec![(is, 1.0)]),
                    );
                }
            }
        }

        // Craft d'établi final.
        if e.rarete != Rarete::Normal && e.meta == Meta::Aucun {
            let manque = self.complet & !e.bons;
            if manque.count_ones() == 1 && e.bloques & manque == 0 {
                let k = manque.trailing_zeros() as usize;
                let c = self.cotes[k];
                if self.n(&e, c) < self.cap(e.rarete)[c]
                    && let Some((texte, cout)) = self.finitions[k].clone()
                {
                    let monnaies: Vec<(&str, f64)> = cout
                        .iter()
                        .map(|(n, q)| (n.as_str(), f64::from(*q)))
                        .collect();
                    ajouter(
                        self,
                        Genre::Finition,
                        &format!("Établi : {texte}"),
                        &monnaies,
                        Issue::Succes,
                    );
                }
            }
        }
        out
    }

    fn masque(&self, c: usize) -> u8 {
        (0..self.goal.len())
            .filter(|&k| self.cotes[k] == c)
            .fold(0, |m, k| m | (1 << k))
    }

    /// Annulment : un mod au hasard (chances égales) hors du côté verrouillé, metamod compris.
    fn annuler(&mut self, e: &Etat, a_visiter: &mut Vec<usize>) -> Issue {
        let verrou = e.meta.verrou();
        let libre = |c: usize| verrou != Some(c);
        let mut issues: Vec<Etat> = Vec::new();
        for k in 0..self.goal.len() {
            if !libre(self.cotes[k]) {
                continue;
            }
            if e.bons & (1 << k) != 0 {
                let mut s = *e;
                s.bons &= !(1 << k);
                issues.push(s);
            }
            if e.bloques & (1 << k) != 0 {
                let mut s = *e;
                s.bloques &= !(1 << k);
                issues.push(s);
            }
        }
        for c in 0..2 {
            if libre(c) {
                for _ in 0..e.autres[c] {
                    let mut s = *e;
                    s.autres[c] -= 1;
                    issues.push(s);
                }
            }
        }
        if let Some(c) = e.meta.cote()
            && libre(c)
        {
            let mut s = *e;
            s.meta = Meta::Aucun;
            issues.push(s);
        }
        let n = issues.len() as f64;
        let mut fusion: BTreeMap<Etat, f64> = BTreeMap::new();
        for s in issues {
            *fusion.entry(s).or_default() += 1.0 / n;
        }
        Issue::Etats(
            fusion
                .into_iter()
                .map(|(s, p)| (self.etat(s, a_visiter), p))
                .collect(),
        )
    }
}

fn verrou_texte(c: usize) -> &'static str {
    if c == 0 {
        "préfixes verrouillés"
    } else {
        "suffixes verrouillés"
    }
}

/// Résout un système linéaire dense (pivot partiel). `None` s'il est singulier.
/// Graphe complet : états, actions, nœuds de relance.
struct Graphe {
    actions: Vec<Vec<Action>>,
    noeuds: Vec<Vec<(usize, f64)>>,
}

impl Graphe {
    fn issue(&self, a: &Action, v: &[f64], x: &[f64]) -> f64 {
        match &a.issue {
            Issue::Succes => 0.0,
            Issue::Noeud(h) => x[*h],
            Issue::Etats(l) => l.iter().map(|(s, p)| p * v[*s]).sum(),
        }
    }

    /// Valeurs optimales pour des valeurs de nœuds fixées (Gauss-Seidel), et politique.
    fn valeurs(&self, x: &[f64], v: &mut [f64], politique: &mut [usize], passes: usize) {
        let n = v.len();
        for passe in 0..passes {
            let mut ecart: f64 = 0.0;
            let ordre: Box<dyn Iterator<Item = usize>> = if passe % 2 == 0 {
                Box::new(0..n)
            } else {
                Box::new((0..n).rev())
            };
            for s in ordre {
                if self.actions[s].is_empty() {
                    continue;
                }
                let (mut best, mut arg) = (f64::INFINITY, 0);
                for (j, a) in self.actions[s].iter().enumerate() {
                    let q = a.cout + self.issue(a, v, x);
                    if q < best {
                        best = q;
                        arg = j;
                    }
                }
                ecart = ecart.max((best - v[s]).abs() / best.abs().max(1.0));
                v[s] = best;
                politique[s] = arg;
            }
            if ecart < 1e-9 {
                break;
            }
        }
    }

    /// Meilleure action de chaque état pour des valeurs données. `true` si la politique change.
    fn ameliorer(&self, v: &[f64], x: &[f64], politique: &mut [usize]) -> bool {
        let mut change = false;
        for (s, acts) in self.actions.iter().enumerate() {
            let Some(actuelle) = acts.get(politique[s]) else {
                continue;
            };
            let q0 = actuelle.cout + self.issue(actuelle, v, x);
            let (mut best, mut arg) = (q0, politique[s]);
            for (j, a) in acts.iter().enumerate() {
                let q = a.cout + self.issue(a, v, x);
                if q < best - 1e-9 * q0.abs().max(1.0) {
                    best = q;
                    arg = j;
                }
            }
            if arg != politique[s] {
                politique[s] = arg;
                change = true;
            }
        }
        change
    }

    /// Lignes du système `(I - P) v = c` d'une politique : états, puis nœuds de relance.
    fn systeme(&self, politique: &[usize]) -> Vec<Vec<(usize, f64)>> {
        let ns = politique.len();
        let h = self.noeuds.len();
        let n = ns + h;
        let mut lignes: Vec<Vec<(usize, f64)>> = Vec::with_capacity(n);
        for (s, &choix) in politique.iter().enumerate() {
            let mut l = vec![(s, 1.0)];
            if let Some(a) = self.actions[s].get(choix) {
                match &a.issue {
                    Issue::Succes => {}
                    Issue::Noeud(j) => l.push((ns + j, -1.0)),
                    Issue::Etats(e) => {
                        for &(t, p) in e {
                            if t == s {
                                l[0].1 -= p;
                            } else {
                                l.push((t, -p));
                            }
                        }
                    }
                }
            }
            lignes.push(l);
        }
        for loi in &self.noeuds {
            let i = lignes.len();
            let mut l = vec![(i, 1.0)];
            l.extend(loi.iter().map(|&(t, p)| (t, -p)));
            lignes.push(l);
        }
        lignes
    }

    /// Nombre moyen de passages par chaque état en partant de `depart` (système transposé).
    fn visites(&self, politique: &[usize], depart: usize) -> Option<Vec<f64>> {
        let lignes = self.systeme(politique);
        let n = lignes.len();
        let mut t: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for (i, l) in lignes.iter().enumerate() {
            for &(j, v) in l {
                t[j].push((i, v));
            }
        }
        let mut b = vec![0.0; n];
        b[depart] = 1.0;
        let y = bicgstab(&t, &b)?;
        Some(y.into_iter().map(|v| v.max(0.0)).collect())
    }

    /// Évalue une politique fixée pour plusieurs coûts par action. Pour chaque coût : valeur de chaque
    /// état et de chaque nœud. `None` si la politique n'atteint pas l'objectif.
    ///
    /// Les inconnues sont les valeurs des états et des nœuds de relance ; le système creux
    /// `(I - P) v = c` est résolu par BiCGSTAB, insensible aux boucles lentes (Exalted puis
    /// Annulment, relances rares) qui freinent Gauss-Seidel.
    fn evaluer(&self, politique: &[usize], couts: &[CoutAction]) -> Option<Vec<Valeurs>> {
        let ns = politique.len();
        let n = ns + self.noeuds.len();
        let lignes = self.systeme(politique);
        let mut sortie = Vec::with_capacity(couts.len());
        for c in couts {
            let mut b = vec![0.0; n];
            for s in 0..ns {
                if let Some(a) = self.actions[s].get(politique[s]) {
                    b[s] = c(a);
                }
            }
            let x = bicgstab(&lignes, &b)?;
            let echelle = x.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
            if x.iter().any(|v| !v.is_finite() || *v < -1e-6 * echelle) {
                return None;
            }
            let x: Vec<f64> = x.into_iter().map(|v| v.max(0.0)).collect();
            sortie.push((x[..ns].to_vec(), x[ns..].to_vec()));
        }
        Some(sortie)
    }
}

/// Résout `A x = b` (A creuse par lignes) par BiCGSTAB préconditionné par la diagonale.
/// `None` si la méthode ne converge pas (système singulier : politique qui n'aboutit pas).
fn bicgstab(a: &[Vec<(usize, f64)>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    let diag: Vec<f64> = a
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let d: f64 = l.iter().filter(|(j, _)| *j == i).map(|(_, v)| v).sum();
            if d.abs() > 1e-300 { d } else { 1.0 }
        })
        .collect();
    let mul = |x: &[f64], y: &mut [f64]| {
        for (i, l) in a.iter().enumerate() {
            y[i] = l.iter().map(|&(j, v)| v * x[j]).sum();
        }
    };
    let dot = |u: &[f64], v: &[f64]| u.iter().zip(v).map(|(a, b)| a * b).sum::<f64>();
    let norme_b = dot(b, b).sqrt();
    if norme_b == 0.0 {
        return Some(vec![0.0; n]);
    }
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    // Résidu « fantôme » : pas exactement r, sinon un second membre creux (un seul état de départ)
    // fait vite tomber r0 · r à zéro.
    let r0: Vec<f64> = r
        .iter()
        .enumerate()
        .map(|(i, x)| x + norme_b * (0.5 + ((i * 7919) % 101) as f64 / 202.0) / (n as f64).sqrt())
        .collect();
    let (mut rho, mut alpha, mut omega) = (1.0, 1.0, 1.0);
    let mut v = vec![0.0; n];
    let mut p = vec![0.0; n];
    let mut y = vec![0.0; n];
    let mut z = vec![0.0; n];
    let mut t = vec![0.0; n];
    let mut sv = vec![0.0; n];
    for _ in 0..5_000 {
        let rho1 = dot(&r0, &r);
        if rho1.abs() < 1e-300 {
            return None;
        }
        let beta = (rho1 / rho) * (alpha / omega);
        rho = rho1;
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
            y[i] = p[i] / diag[i];
        }
        mul(&y, &mut v);
        let d = dot(&r0, &v);
        if d.abs() < 1e-300 {
            return None;
        }
        alpha = rho / d;
        for i in 0..n {
            sv[i] = r[i] - alpha * v[i];
        }
        if dot(&sv, &sv).sqrt() <= 1e-12 * norme_b {
            for i in 0..n {
                x[i] += alpha * y[i];
            }
            return Some(x);
        }
        for i in 0..n {
            z[i] = sv[i] / diag[i];
        }
        mul(&z, &mut t);
        let tt = dot(&t, &t);
        if tt < 1e-300 {
            return None;
        }
        omega = dot(&t, &sv) / tt;
        for i in 0..n {
            x[i] += alpha * y[i] + omega * z[i];
            r[i] = sv[i] - omega * t[i];
        }
        let res = dot(&r, &r).sqrt();
        if !res.is_finite() || res > 1e14 * norme_b {
            return None;
        }
        if res <= 1e-12 * norme_b {
            return Some(x);
        }
        if omega.abs() < 1e-300 {
            return None;
        }
    }
    None
}

/// Politique optimale et son coût moyen.
struct Solution<'a> {
    m: Modele<'a>,
    g: Graphe,
    politique: Vec<usize>,
    depart: usize,
    cout_moyen: f64,
}

/// Plan au coût moyen minimal. `None` si l'objectif est hors d'atteinte avec les prix connus.
pub fn plan(pb: &PoolPoe1, goal: &Goal, prix: &BTreeMap<String, f64>) -> Option<PlanPoe1> {
    resoudre(pb, goal, prix).map(|s| plan_depuis(&s))
}

/// Résumé d'une solution : consommation moyenne par monnaie et par action.
fn plan_depuis(sol: &Solution) -> PlanPoe1 {
    let Solution {
        m,
        g,
        politique,
        depart,
        cout_moyen,
    } = sol;
    let (depart, cout_moyen) = (*depart, *cout_moyen);

    // Consommation moyenne par monnaie et par action, en une seule évaluation de la politique.
    let nm = m.monnaies.len();
    let fonctions: Vec<FonctionCout> = (0..nm)
        .map(|j| -> FonctionCout {
            Box::new(move |a: &Action| {
                a.conso
                    .iter()
                    .filter(|(i, _)| *i == j)
                    .map(|(_, q)| q)
                    .sum()
            })
        })
        .chain(
            (0..m.libelles.len()).map(|j| -> Box<dyn Fn(&Action) -> f64> {
                Box::new(move |a: &Action| if a.libelle == j { 1.0 } else { 0.0 })
            }),
        )
        .collect();
    let visites = g.visites(politique, depart).unwrap_or_default();
    let moyennes: Vec<f64> = fonctions
        .iter()
        .map(|f| {
            visites
                .iter()
                .enumerate()
                .filter_map(|(s, y)| g.actions.get(s)?.get(politique[s]).map(|a| y * f(a)))
                .sum()
        })
        .collect();
    let mut monnaies: Vec<Conso> = moyennes
        .iter()
        .take(nm)
        .enumerate()
        .filter(|(_, q)| **q > 1e-9)
        .map(|(j, q)| Conso {
            nom: m.monnaies[j].clone(),
            quantite: *q,
        })
        .collect();
    monnaies.sort_by(|a, b| b.quantite.total_cmp(&a.quantite));
    let mut usages: Vec<UsageAction> = moyennes
        .iter()
        .skip(nm)
        .enumerate()
        .filter(|(_, q)| **q > 1e-6)
        .map(|(j, q)| UsageAction {
            action: m.libelles[j].clone(),
            nombre: *q,
        })
        .collect();
    usages.sort_by(|a, b| b.nombre.total_cmp(&a.nombre));
    let depart_libelle = g.actions[depart]
        .get(politique[depart])
        .map(|a| m.libelles[a.libelle].clone())
        .unwrap_or_default();
    let mut sans_prix = m.sans_prix.clone();
    sans_prix.sort();
    PlanPoe1 {
        cout_moyen,
        depart: depart_libelle,
        actions: usages,
        monnaies,
        sans_prix,
    }
}

fn resoudre<'a>(
    pb: &PoolPoe1,
    goal: &'a Goal,
    prix: &'a BTreeMap<String, f64>,
) -> Option<Solution<'a>> {
    let pool = &pb.pool;
    let ilvl = pb.ilvl;
    let n = goal.len();
    if n == 0 || n > VOULUS_MAX {
        return None;
    }
    let cotes: Vec<usize> = goal.slots.iter().map(|s| cote(*s)).collect();
    let (mp, ms) = pool.cap(Rarity::Magic);
    let (rp, rs) = pool.cap(Rarity::Rare);

    let essences = pb
        .essences
        .iter()
        .filter_map(|e| match goal.classify(pool, e.affixe) {
            Class::Wanted(k) => Some((e.essence.nom.clone(), k, e.essence.sur_rare, e.affixe)),
            _ => None,
        })
        .collect();
    let cout_etabli = |cout: &[(String, u32)]| -> Option<f64> {
        cout.iter()
            .map(|(n, q)| prix.get(n).map(|p| p * f64::from(*q)))
            .sum()
    };
    let mut finitions: Vec<Option<(String, CoutEtabli, f64)>> = vec![None; n];
    let mut metamods = Vec::new();
    for c in &pb.etabli {
        let a = &pool.affixes[c.affixe as usize];
        if let Some((_, meta)) = METAMODS.iter().find(|(id, _)| *id == a.id) {
            metamods.push((*meta, a.text.clone(), c.cout.clone()));
            continue;
        }
        if let Class::Wanted(k) = goal.classify(pool, c.affixe)
            && cote(a.slot) == cotes[k]
        {
            let prix_c = cout_etabli(&c.cout).unwrap_or(f64::INFINITY);
            if finitions[k].as_ref().is_none_or(|f| prix_c < f.2) {
                finitions[k] = Some((a.text.clone(), c.cout.clone(), prix_c));
            }
        }
    }
    metamods.sort_by_key(|a| a.0);

    let mut m = Modele {
        goal,
        complet: ((1u16 << n) - 1) as u8,
        cotes,
        cap_magique: [mp, ms],
        cap_rare: [rp, rs],
        tirages: [
            tirage(pool, goal, ilvl, 0),
            tirage(pool, goal, ilvl, TAG_ATTAQUE),
            tirage(pool, goal, ilvl, TAG_SORT),
        ],
        prix,
        essences,
        metamods,
        finitions: finitions
            .into_iter()
            .map(|f| f.map(|(t, c, _)| (t, c)))
            .collect(),
        etats: Vec::new(),
        index: HashMap::new(),
        noeuds: Vec::new(),
        index_noeuds: HashMap::new(),
        libelles: Vec::new(),
        index_libelles: HashMap::new(),
        monnaies: Vec::new(),
        index_monnaies: HashMap::new(),
        sans_prix: Vec::new(),
    };

    // Exploration des états atteignables depuis l'objet Normal.
    let mut a_visiter = Vec::new();
    let depart = m.etat(Etat::vide(Rarete::Normal), &mut a_visiter);
    let mut actions: Vec<Vec<Action>> = Vec::new();
    while let Some(i) = a_visiter.pop() {
        let acts = m.actions(i, &mut a_visiter);
        if actions.len() <= i {
            actions.resize(i + 1, Vec::new());
        }
        actions[i] = acts;
    }
    actions.resize(m.etats.len(), Vec::new());
    let g = Graphe {
        actions,
        noeuds: std::mem::take(&mut m.noeuds),
    };

    // Itération de politique sur les nœuds de relance. La première politique est celle d'une valeur
    // de relance fixe `borne` : on la relève tant que cette politique n'atteint pas l'objectif.
    let ns = m.etats.len();
    let mut v = vec![0.0; ns];
    let mut politique = vec![0usize; ns];
    let mut borne = 1e3;
    // Première politique qui aboutit : celle d'une valeur de relance fixe `borne`, relevée au besoin.
    let (mut vals, mut x) = loop {
        if borne > 1e12 {
            return None;
        }
        let x0 = vec![borne; g.noeuds.len()];
        g.valeurs(&x0, &mut v, &mut politique, 300);
        if let Some(r) = g.evaluer(&politique, &[&|a: &Action| a.cout]) {
            break r.into_iter().next()?;
        }
        borne *= 4.0;
    };
    // Itération de politique : meilleure action de chaque état au vu des valeurs exactes de la
    // politique en cours, puis nouvelle évaluation, jusqu'à stabilité.
    for _ in 0..200 {
        if !g.ameliorer(&vals, &x, &mut politique) {
            break;
        }
        (vals, x) = g
            .evaluer(&politique, &[&|a: &Action| a.cout])?
            .into_iter()
            .next()?;
    }
    let cout_moyen = vals[depart];
    if !cout_moyen.is_finite() || cout_moyen >= 1e8 {
        return None;
    }
    Some(Solution {
        m,
        g,
        politique,
        depart,
        cout_moyen,
    })
}

#[cfg(test)]
mod tests {
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use super::*;
    use crate::forge::WantedAffix;
    use crate::poe1::forge::calcul::chance_rare;
    use crate::poe1::forge::donnees::construire_pool;

    /// Glorious Plate.
    const ARMURE: &str = "Metadata/Items/Armours/BodyArmours/BodyStr17";

    fn voulu(pool: &AffixPool, id_mod: &str) -> WantedAffix {
        let a = pool
            .affixes
            .iter()
            .find(|a| a.id == id_mod)
            .unwrap_or_else(|| panic!("{id_mod} absent du pool"));
        WantedAffix {
            group: a.group,
            family: a.family_id,
            max_tier: a.tier,
        }
    }

    fn prix(liste: &[(&str, f64)]) -> BTreeMap<String, f64> {
        liste.iter().map(|(n, p)| (n.to_string(), *p)).collect()
    }

    /// Cote du 14/11/2026 (ligue en cours), arrondie.
    fn cote_marche() -> BTreeMap<String, f64> {
        prix(&[
            (CHAOS, 1.0),
            (ALCHEMY, 0.03),
            (TRANSMUTATION, 0.03),
            (ALTERATION, 0.07),
            (AUGMENTATION, 0.04),
            (REGAL, 0.35),
            (EXALTED, 0.45),
            (ANNULMENT, 3.0),
            (SCOURING, 0.78),
            ("Divine Orb", 90.0),
            ("Orb of Chance", 0.4),
            ("Vaal Orb", 0.58),
            ("Screaming Essence of Greed", 1.13),
            ("Screaming Essence of Anger", 1.03),
            ("Screaming Essence of Hatred", 0.85),
        ])
    }

    /// Objet exact pour la simulation.
    #[derive(Clone)]
    struct Objet {
        rarete: Rarete,
        mods: Vec<u16>,
        meta: Meta,
    }

    fn abstrait(m: &Modele, pool: &AffixPool, o: &Objet) -> Etat {
        let mut e = Etat::vide(o.rarete);
        e.meta = o.meta;
        for &i in &o.mods {
            let a = &pool.affixes[i as usize];
            match m.goal.classify(pool, i) {
                Class::Wanted(k) if cote(a.slot) == m.cotes[k] => e.bons |= 1 << k,
                Class::Wanted(k) | Class::Blocked(k) => e.bloques |= 1 << k,
                Class::Other => e.autres[cote(a.slot)] += 1,
            }
        }
        e
    }

    /// Ajoute un mod tiré sur le moteur exact (un mod par groupe, plafond par côté, tags interdits).
    fn ajouter(
        pool: &AffixPool,
        ilvl: u8,
        o: &mut Objet,
        cap: [u8; 2],
        interdit: u64,
        rng: &mut SmallRng,
    ) {
        let mut n = [0u8; 2];
        for &i in &o.mods {
            n[cote(pool.affixes[i as usize].slot)] += 1;
        }
        if let Some(c) = o.meta.cote() {
            n[c] += 1;
        }
        let ok = |j: usize| {
            let a = &pool.affixes[j];
            a.weight > 0
                && a.req_ilvl <= ilvl
                && a.tags & interdit == 0
                && n[cote(a.slot)] < cap[cote(a.slot)]
                && !o
                    .mods
                    .iter()
                    .any(|&i| pool.affixes[i as usize].conflicts(a))
        };
        let total: u64 = (0..pool.affixes.len())
            .filter(|&j| ok(j))
            .map(|j| u64::from(pool.affixes[j].weight))
            .sum();
        if total == 0 {
            return;
        }
        let mut x = rng.random_range(0..total);
        for j in (0..pool.affixes.len()).filter(|&j| ok(j)) {
            let w = u64::from(pool.affixes[j].weight);
            if x < w {
                o.mods.push(j as u16);
                return;
            }
            x -= w;
        }
    }

    fn nombre_rare(rng: &mut SmallRng) -> u8 {
        let x: f64 = rng.random();
        if x < 8.0 / 12.0 {
            4
        } else if x < 11.0 / 12.0 {
            5
        } else {
            6
        }
    }

    /// Coût moyen de la politique optimale, simulée sur le moteur exact.
    fn simuler(sol: &Solution, pb: &PoolPoe1, n: usize) -> f64 {
        let pool = &pb.pool;
        let ilvl = pb.ilvl;
        let m = &sol.m;
        let mut rng = SmallRng::seed_from_u64(11);
        let mut total = 0.0;
        for _ in 0..n {
            let mut o = Objet {
                rarete: Rarete::Normal,
                mods: Vec::new(),
                meta: Meta::Aucun,
            };
            for _pas in 0..100_000 {
                let e = abstrait(m, pool, &o);
                if m.reussi(&e) {
                    break;
                }
                let s = *m
                    .index
                    .get(&e)
                    .unwrap_or_else(|| panic!("état absent du modèle : {e:?}"));
                let a = &sol.g.actions[s][sol.politique[s]];
                total += a.cout;
                let rare = m.cap_rare;
                let magique = m.cap_magique;
                match a.genre {
                    Genre::Transmutation | Genre::Alteration => {
                        o = Objet {
                            rarete: Rarete::Magique,
                            mods: Vec::new(),
                            meta: Meta::Aucun,
                        };
                        let nb = if rng.random::<f64>() < DEUX_MODS_MAGIQUE {
                            2
                        } else {
                            1
                        };
                        for _ in 0..nb {
                            ajouter(pool, ilvl, &mut o, magique, 0, &mut rng);
                        }
                    }
                    Genre::Alchemy | Genre::Chaos => {
                        let garde: Vec<u16> = match o.meta.verrou() {
                            Some(c) => o
                                .mods
                                .iter()
                                .copied()
                                .filter(|&i| cote(pool.affixes[i as usize].slot) == c)
                                .collect(),
                            None => Vec::new(),
                        };
                        let nb = nombre_rare(&mut rng).saturating_sub(garde.len() as u8);
                        o = Objet {
                            rarete: Rarete::Rare,
                            mods: garde,
                            meta: Meta::Aucun,
                        };
                        for _ in 0..nb {
                            ajouter(pool, ilvl, &mut o, rare, 0, &mut rng);
                        }
                    }
                    Genre::Essence(j) => {
                        let nb = nombre_rare(&mut rng) - 1;
                        o = Objet {
                            rarete: Rarete::Rare,
                            mods: vec![m.essences[j].3],
                            meta: Meta::Aucun,
                        };
                        for _ in 0..nb {
                            ajouter(pool, ilvl, &mut o, rare, 0, &mut rng);
                        }
                    }
                    Genre::Exalted => {
                        let interdit = match o.meta {
                            Meta::SansAttaque => TAG_ATTAQUE,
                            Meta::SansSort => TAG_SORT,
                            _ => 0,
                        };
                        ajouter(pool, ilvl, &mut o, rare, interdit, &mut rng);
                    }
                    Genre::Regal => {
                        o.rarete = Rarete::Rare;
                        ajouter(pool, ilvl, &mut o, rare, 0, &mut rng);
                    }
                    Genre::Augmentation => ajouter(pool, ilvl, &mut o, magique, 0, &mut rng),
                    Genre::Annulment => {
                        let verrou = o.meta.verrou();
                        let libres: Vec<usize> = (0..o.mods.len())
                            .filter(|&i| {
                                Some(cote(pool.affixes[o.mods[i] as usize].slot)) != verrou
                            })
                            .collect();
                        let meta_libre = o.meta.cote().is_some_and(|c| Some(c) != verrou);
                        let nb = libres.len() + usize::from(meta_libre);
                        let x = rng.random_range(0..nb);
                        if x < libres.len() {
                            o.mods.remove(libres[x]);
                        } else {
                            o.meta = Meta::Aucun;
                        }
                    }
                    Genre::Scouring => match o.meta.verrou() {
                        Some(c) => {
                            o.mods.retain(|&i| cote(pool.affixes[i as usize].slot) == c);
                            o.meta = Meta::Aucun;
                            o.rarete = match o.mods.len() {
                                0 => Rarete::Normal,
                                1 => Rarete::Magique,
                                _ => Rarete::Rare,
                            };
                        }
                        None => {
                            o = Objet {
                                rarete: Rarete::Normal,
                                mods: Vec::new(),
                                meta: Meta::Aucun,
                            }
                        }
                    },
                    Genre::Meta(mt) => o.meta = mt,
                    Genre::RetirerMeta => o.meta = Meta::Aucun,
                    Genre::Finition => break,
                }
            }
        }
        total / n as f64
    }

    #[test]
    fn chaos_seul_retrouve_le_spam() {
        let pb = construire_pool(ARMURE, 84, None).unwrap();
        // Tiers hors de portée de l'établi : seuls Alchemy et Chaos comptent.
        let voulus = [
            voulu(&pb.pool, "IncreasedLife9"),
            voulu(&pb.pool, "FireResist6"),
        ];
        let goal = Goal::new(&pb.pool, &voulus).unwrap();
        let p = chance_rare(&pb.pool, &goal, 84, None, None);
        let prix = prix(&[(CHAOS, 1.0), (ALCHEMY, 0.5)]);
        let plan = plan(&pb, &goal, &prix).unwrap();
        // Alchemy sur l'objet Normal, puis Chaos jusqu'à réussir.
        let attendu = 0.5 + (1.0 - p) / p;
        assert!(
            (plan.cout_moyen - attendu).abs() / attendu < 1e-6,
            "{} contre {attendu}",
            plan.cout_moyen
        );
        assert_eq!(plan.depart, ALCHEMY);
    }

    #[test]
    fn plan_conforme_a_la_simulation() {
        let pb = construire_pool(ARMURE, 84, None).unwrap();
        let voulus = [
            voulu(&pb.pool, "IncreasedLife8"),
            voulu(&pb.pool, "FireResist4"),
            voulu(&pb.pool, "ColdResist4"),
        ];
        let goal = Goal::new(&pb.pool, &voulus).unwrap();
        let prix = cote_marche();
        let sol = resoudre(&pb, &goal, &prix).unwrap();
        let sim = simuler(&sol, &pb, 6_000);
        let actions: Vec<(String, f64)> = plan_depuis(&sol)
            .actions
            .into_iter()
            .map(|u| (u.action, u.nombre))
            .collect();
        eprintln!("solveur {} simulation {sim} {actions:?}", sol.cout_moyen);
        assert!(
            (sol.cout_moyen - sim).abs() / sim < 0.05,
            "solveur {} contre simulation {sim}",
            sol.cout_moyen
        );
    }

    #[test]
    #[ignore = "simulation longue en debug : cargo test --release -p poe2-core -- --ignored metamods"]
    fn metamods_et_exalted_chers() {
        // Metamods bon marché, Exalted cher : le plan passe par les verrous, et la simulation exacte
        // de cette politique (Chaos, Scouring et Annulment verrouillés) doit retrouver son coût.
        let pb = construire_pool(ARMURE, 84, None).unwrap();
        let voulus = [
            voulu(&pb.pool, "IncreasedLife9"),
            voulu(&pb.pool, "FireResist6"),
            voulu(&pb.pool, "ColdResist6"),
            voulu(&pb.pool, "LightningResist6"),
        ];
        let goal = Goal::new(&pb.pool, &voulus).unwrap();
        let mut prix = cote_marche();
        prix.insert(EXALTED.into(), 15.0);
        prix.insert("Divine Orb".into(), 1.0);
        let sol = resoudre(&pb, &goal, &prix).unwrap();
        let plan = plan_depuis(&sol);
        eprintln!("{:?}", plan.actions);
        assert!(
            plan.actions
                .iter()
                .any(|u| u.action.contains("verrouillés")),
            "aucun verrou dans le plan"
        );
        let sim = simuler(&sol, &pb, 20_000);
        eprintln!("solveur {} simulation {sim}", sol.cout_moyen);
        assert!(
            (sol.cout_moyen - sim).abs() / sim < 0.05,
            "solveur {} contre simulation {sim}",
            sol.cout_moyen
        );
    }
}
