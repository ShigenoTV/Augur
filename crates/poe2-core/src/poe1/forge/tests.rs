use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use super::calcul::*;
use super::*;
use crate::forge::{AffixPool, DrawFilter, Goal, ItemState, Mod, Rarity, Slot};

/// Two-Stone Ring (feu et foudre).
const ANNEAU: &str = "Metadata/Items/Rings/Ring12";

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

fn manquants(pool: &AffixPool, goal: &Goal, item: &ItemState) -> Vec<usize> {
    (0..goal.len())
        .filter(|&k| {
            !item
                .mods()
                .iter()
                .any(|m| goal.classify(pool, m.idx) == crate::forge::Class::Wanted(k))
        })
        .collect()
}

/// Tirage rapide (le moteur exact est trop lent sans optimisation pour 200 000 objets) : mêmes règles
/// que `AffixPool::draw` sans filtre, un mod par groupe, 3 préfixes et 3 suffixes au plus.
struct Tireur {
    eligibles: Vec<(u16, u32, u16, Slot)>,
}

impl Tireur {
    fn new(pool: &AffixPool, ilvl: u8) -> Self {
        Self {
            eligibles: pool
                .affixes
                .iter()
                .enumerate()
                .filter(|(_, a)| a.weight > 0 && a.req_ilvl <= ilvl)
                .map(|(i, a)| (i as u16, a.weight, a.group, a.slot))
                .collect(),
        }
    }

    fn tirer(&self, pris: &[(u16, Slot)], rng: &mut SmallRng) -> Option<u16> {
        let plein = |s: Slot| pris.iter().filter(|(_, t)| *t == s).count() >= 3;
        let (pp, ps) = (plein(Slot::Prefix), plein(Slot::Suffix));
        let ok = |g: u16, s: Slot| {
            !(match s {
                Slot::Prefix => pp,
                Slot::Suffix => ps,
            }) && !pris.iter().any(|(h, _)| *h == g)
        };
        let total: u64 = self
            .eligibles
            .iter()
            .filter(|e| ok(e.2, e.3))
            .map(|e| u64::from(e.1))
            .sum();
        if total == 0 {
            return None;
        }
        let mut x = rng.random_range(0..total);
        for e in self.eligibles.iter().filter(|e| ok(e.2, e.3)) {
            if x < u64::from(e.1) {
                return Some(e.0);
            }
            x -= u64::from(e.1);
        }
        None
    }
}

fn simuler_rare(pool: &AffixPool, goal: &Goal, ilvl: u8, garanti: Option<u16>, n: usize) -> f64 {
    let mut rng = SmallRng::seed_from_u64(7);
    let tireur = Tireur::new(pool, ilvl);
    let mut ok = 0;
    for _ in 0..n {
        let x: f64 = rng.random();
        let nb = if x < 8.0 / 12.0 {
            4
        } else if x < 11.0 / 12.0 {
            5
        } else {
            6
        };
        let mut item = ItemState::new(Rarity::Rare, ilvl);
        let mut pris: Vec<(u16, Slot)> = Vec::with_capacity(6);
        if let Some(g) = garanti {
            item.push(Mod {
                idx: g,
                ..Mod::default()
            });
            let a = &pool.affixes[g as usize];
            pris.push((a.group, a.slot));
        }
        while item.len() < nb {
            match tireur.tirer(&pris, &mut rng) {
                Some(i) => {
                    let a = &pool.affixes[i as usize];
                    pris.push((a.group, a.slot));
                    item.push(Mod {
                        idx: i,
                        ..Mod::default()
                    })
                }
                None => break,
            }
        }
        if goal.is_met(&item) {
            ok += 1;
        }
    }
    ok as f64 / n as f64
}

/// Méthode Alteration + Augmentation par énumération exacte sur le moteur (`AffixPool::distribution`).
fn enumerer_alteration(pool: &AffixPool, goal: &Goal, ilvl: u8) -> (f64, f64) {
    let f = DrawFilter::default();
    let vide = ItemState::new(Rarity::Magic, ilvl);
    let (mut ok, mut augs) = (0.0, 0.0);
    for (i, p1) in pool.distribution(&vide, &f) {
        let mut un = vide;
        un.push(Mod {
            idx: i,
            ..Mod::default()
        });
        // Un seul mod (une fois sur deux), Augmentation si elle peut finir l'objet.
        if goal.is_met(&un) {
            ok += 0.5 * p1;
        } else {
            let m = manquants(pool, goal, &un);
            if m.len() == 1 && goal.slots[m[0]] != pool.affixes[i as usize].slot {
                augs += 0.5 * p1;
                ok += 0.5 * p1 * chance_suivante(pool, goal, &un);
            }
        }
        // Deux mods (une fois sur deux).
        ok += 0.5 * p1 * chance_suivante(pool, goal, &un);
    }
    (ok, augs)
}

fn chance_suivante(pool: &AffixPool, goal: &Goal, item: &ItemState) -> f64 {
    pool.distribution(item, &DrawFilter::default())
        .into_iter()
        .filter(|&(j, _)| {
            let mut deux = *item;
            deux.push(Mod {
                idx: j,
                ..Mod::default()
            });
            goal.is_met(&deux)
        })
        .map(|(_, p)| p)
        .sum()
}

fn proche(a: f64, b: f64, rel: f64) {
    assert!(
        (a - b).abs() <= rel * b.max(1e-9),
        "calcul {a:.6} trop loin de la simulation {b:.6}"
    );
}

#[test]
fn donnees_chargees() {
    let d = donnees().unwrap();
    assert!(d.bases.len() > 500, "{} bases", d.bases.len());
    assert!(d.essences.len() > 90);
    let anneau = bases()
        .unwrap()
        .into_iter()
        .find(|b| b.id == ANNEAU)
        .unwrap();
    assert_eq!(anneau.nom, "Two-Stone Ring");
    assert_eq!(anneau.influences.len(), 6);
    assert!(anneau.implicites[0].contains("Fire and Lightning"));
}

#[test]
fn influence_ajoute_ses_mods() {
    let neutre = construire_pool(ANNEAU, 86, None).unwrap();
    let shaper = construire_pool(ANNEAU, 86, Some("Shaper")).unwrap();
    let tirables = |p: &donnees::PoolPoe1| p.pool.affixes.iter().filter(|a| a.weight > 0).count();
    assert!(tirables(&shaper) > tirables(&neutre));
    assert!(construire_pool(ANNEAU, 86, Some("Inconnue")).is_err());
}

#[test]
fn tiers_et_affixes_proposes() {
    let a = affixes(ANNEAU, 86, None).unwrap();
    let vie = a
        .affixes
        .iter()
        .find(|g| g.libelle.contains("maximum Life") && g.slot == Slot::Prefix)
        .expect("vie sur un anneau");
    assert_eq!(vie.paliers[0].tier, 1);
    assert!(vie.part > 0.0);
    // Un niveau d'objet bas retire les meilleurs tiers.
    let bas = affixes(ANNEAU, 30, None).unwrap();
    let vie_bas = bas
        .affixes
        .iter()
        .find(|g| g.group == vie.group && g.family == vie.family)
        .unwrap();
    assert!(vie_bas.paliers.len() < vie.paliers.len());
}

#[test]
fn alteration_exacte() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    for voulus in [
        vec![voulu(&pb.pool, "IncreasedLife6")],
        vec![voulu(&pb.pool, "ColdResist5")],
        vec![
            voulu(&pb.pool, "IncreasedLife5"),
            voulu(&pb.pool, "ColdResist4"),
        ],
    ] {
        let goal = Goal::new(&pb.pool, &voulus).unwrap();
        let (c, aug) = chance_alteration(&pb.pool, &goal, 86).unwrap();
        let (ce, auge) = enumerer_alteration(&pb.pool, &goal, 86);
        proche(c, ce, 1e-9);
        proche(aug, auge, 1e-9);
    }
}

#[test]
fn chaos_proche_de_la_simulation() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    for voulus in [
        vec![
            voulu(&pb.pool, "IncreasedLife5"),
            voulu(&pb.pool, "ColdResist4"),
        ],
        vec![
            voulu(&pb.pool, "IncreasedLife3"),
            voulu(&pb.pool, "ColdResist2"),
            voulu(&pb.pool, "FireResist2"),
        ],
    ] {
        let goal = Goal::new(&pb.pool, &voulus).unwrap();
        let c = chance_rare(&pb.pool, &goal, 86, None);
        let s = simuler_rare(&pb.pool, &goal, 86, None, 200_000);
        proche(c, s, 0.05);
    }
}

#[test]
fn essence_garantit_son_mod() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    let ess = pb
        .essences
        .iter()
        .find(|e| e.essence.nom == "Deafening Essence of Hatred")
        .expect("Essence of Hatred utilisable sur un anneau");
    let cible = &pb.pool.affixes[ess.affixe as usize];
    let voulus = vec![
        WantedAffix {
            group: cible.group,
            family: cible.family_id,
            max_tier: cible.tier,
        },
        voulu(&pb.pool, "IncreasedLife5"),
    ];
    let goal = Goal::new(&pb.pool, &voulus).unwrap();
    let c = chance_rare(&pb.pool, &goal, 86, Some(ess.affixe));
    let s = simuler_rare(&pb.pool, &goal, 86, Some(ess.affixe), 200_000);
    proche(c, s, 0.05);
    assert!(c > chance_rare(&pb.pool, &goal, 86, None));

    let r = planifier(&DemandePoe1 {
        base_id: ANNEAU.into(),
        ilvl: 86,
        influence: None,
        voulus,
    })
    .unwrap();
    assert_eq!(r.methodes[0].genre, "essence");
    assert!(r.methodes.iter().any(|m| m.genre == "chaos"));
}

#[test]
fn essence_basse_reservee_aux_petits_niveaux() {
    let haut = construire_pool(ANNEAU, 86, None).unwrap();
    assert!(
        !haut
            .essences
            .iter()
            .any(|e| e.essence.nom.starts_with("Whispering"))
    );
    let bas = construire_pool(ANNEAU, 30, None).unwrap();
    assert!(
        bas.essences
            .iter()
            .any(|e| e.essence.nom.starts_with("Whispering"))
    );
}

#[test]
fn deux_prefixes_ecartent_l_alteration() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    let mana = pb
        .pool
        .affixes
        .iter()
        .find(|a| a.slot == Slot::Prefix && a.weight > 0 && a.text.contains("maximum Mana"))
        .unwrap();
    let voulus = vec![
        voulu(&pb.pool, "IncreasedLife5"),
        WantedAffix {
            group: mana.group,
            family: mana.family_id,
            max_tier: mana.tier,
        },
    ];
    let goal = Goal::new(&pb.pool, &voulus).unwrap();
    assert!(chance_alteration(&pb.pool, &goal, 86).is_none());
}
