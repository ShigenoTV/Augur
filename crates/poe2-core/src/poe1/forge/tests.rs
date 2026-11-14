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

/// Objet réussi, éventuellement fini par le craft `etabli` (place libre de son côté, groupe libre).
fn fini(pool: &AffixPool, goal: &Goal, item: &ItemState, etabli: Option<u16>, cap: u8) -> bool {
    if goal.is_met(item) {
        return true;
    }
    let Some(e) = etabli else { return false };
    let a = &pool.affixes[e as usize];
    let mut x = *item;
    pool.count(item, a.slot) < cap
        && !item
            .mods()
            .iter()
            .any(|m| pool.affixes[m.idx as usize].conflicts(a))
        && {
            x.push(Mod {
                idx: e,
                ..Mod::default()
            });
            goal.is_met(&x)
        }
}

fn simuler_rare(
    pool: &AffixPool,
    goal: &Goal,
    ilvl: u8,
    garanti: Option<u16>,
    etabli: Option<u16>,
    n: usize,
) -> f64 {
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
        if fini(pool, goal, &item, etabli, 3) {
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
        let c = chance_rare(&pb.pool, &goal, 86, None, None);
        let s = simuler_rare(&pb.pool, &goal, 86, None, None, 200_000);
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
    let c = chance_rare(&pb.pool, &goal, 86, Some(ess.affixe), None);
    let s = simuler_rare(&pb.pool, &goal, 86, Some(ess.affixe), None, 200_000);
    proche(c, s, 0.05);
    assert!(c > chance_rare(&pb.pool, &goal, 86, None, None));

    let r = planifier(&DemandePoe1 {
        prix: Default::default(),
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

fn craft_froid(pb: &donnees::PoolPoe1) -> u16 {
    pb.etabli
        .iter()
        .map(|e| e.affixe)
        .filter(|&i| {
            pb.pool.affixes[i as usize]
                .text
                .contains("to Cold Resistance")
        })
        .max_by_key(|&i| pb.pool.affixes[i as usize].req_ilvl)
        .expect("craft de résistance au froid sur un anneau")
}

#[test]
fn etabli_finit_un_rare() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    let craft = craft_froid(&pb);
    let a = &pb.pool.affixes[craft as usize];
    assert!(a.weight == 0, "un mod d'établi n'est jamais tiré");
    let voulus = vec![
        voulu(&pb.pool, "IncreasedLife5"),
        WantedAffix {
            group: a.group,
            family: a.family_id,
            max_tier: a.tier,
        },
    ];
    let goal = Goal::new(&pb.pool, &voulus).unwrap();
    let sans = chance_rare(&pb.pool, &goal, 86, None, None);
    let avec = chance_rare(&pb.pool, &goal, 86, None, Some(craft));
    let s = simuler_rare(&pb.pool, &goal, 86, None, Some(craft), 200_000);
    proche(avec, s, 0.05);
    assert!(avec > 2.0 * sans, "{avec} contre {sans}");
}

#[test]
fn etabli_finit_un_magique() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    let craft = craft_froid(&pb);
    let a = &pb.pool.affixes[craft as usize];
    let voulus = vec![
        voulu(&pb.pool, "IncreasedLife5"),
        WantedAffix {
            group: a.group,
            family: a.family_id,
            max_tier: a.tier,
        },
    ];
    let goal = Goal::new(&pb.pool, &voulus).unwrap();
    let (avec, aug) = chance_alteration_etabli(&pb.pool, &goal, 86, craft).unwrap();
    let (sans, _) = chance_alteration(&pb.pool, &goal, 86).unwrap();
    assert!(avec > 2.0 * sans, "{avec} contre {sans}");
    // Simulation : Alteration, Augmentation si un second mod peut finir l'objet, puis l'établi.
    let mut rng = SmallRng::seed_from_u64(3);
    let f = DrawFilter::default();
    let (mut ok, mut augs) = (0, 0);
    let n = 200_000;
    for _ in 0..n {
        let mut item = ItemState::new(Rarity::Magic, 86);
        let nb = if rng.random_bool(0.5) { 2 } else { 1 };
        while item.len() < nb {
            let i = pool_tirer(&pb.pool, &item, &f, &mut rng);
            item.push(Mod {
                idx: i,
                ..Mod::default()
            });
        }
        if item.len() == 1 && !fini(&pb.pool, &goal, &item, Some(craft), 1) {
            let peut = pb.pool.distribution(&item, &f).iter().any(|&(j, _)| {
                let mut x = item;
                x.push(Mod {
                    idx: j,
                    ..Mod::default()
                });
                fini(&pb.pool, &goal, &x, Some(craft), 1)
            });
            if peut {
                augs += 1;
                let i = pool_tirer(&pb.pool, &item, &f, &mut rng);
                item.push(Mod {
                    idx: i,
                    ..Mod::default()
                });
            }
        }
        if fini(&pb.pool, &goal, &item, Some(craft), 1) {
            ok += 1;
        }
    }
    proche(avec, ok as f64 / n as f64, 0.05);
    proche(aug, augs as f64 / n as f64, 0.05);

    let r = planifier(&DemandePoe1 {
        prix: Default::default(),
        base_id: ANNEAU.into(),
        ilvl: 86,
        influence: None,
        voulus,
    })
    .unwrap();
    let premiere = &r.methodes[0];
    assert!(premiere.finition.is_some(), "{:?}", premiere.nom);
    assert!(premiere.nom.ends_with("+ établi"));
}

fn pool_tirer(pool: &AffixPool, item: &ItemState, f: &DrawFilter, rng: &mut SmallRng) -> u16 {
    pool.draw(item, f, rng).unwrap()
}

fn fossile(nom: &str) -> &'static donnees::FossilePoe1 {
    donnees::donnees()
        .unwrap()
        .fossiles
        .iter()
        .find(|f| f.nom == nom)
        .unwrap_or_else(|| panic!("{nom} absent des données"))
}

fn poids_de(pool: &AffixPool, id: &str) -> u32 {
    pool.affixes.iter().find(|a| a.id == id).unwrap().weight
}

#[test]
fn fossiles_changent_les_poids() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    let vie = poids_de(&pb.pool, "IncreasedLife5");
    let froid = poids_de(&pb.pool, "ColdResist4");
    let mana = poids_de(&pb.pool, "IncreasedMana5");
    // Pristine : vie ×10 ; Frigid : froid ×10, feu bloqué ; le reste ne bouge pas (poids ×100).
    let p = pool_fossiles(
        &pb.pool,
        &[fossile("Pristine Fossil"), fossile("Frigid Fossil")],
    );
    assert_eq!(poids_de(&p, "IncreasedLife5"), vie * 1000);
    assert_eq!(poids_de(&p, "ColdResist4"), froid * 1000);
    assert_eq!(poids_de(&p, "FireResist4"), 0);
    assert_eq!(poids_de(&p, "IncreasedMana5"), mana * 100);
    // Bonus additifs : froid (×10) + élémentaire (×6) = ×16 ; Aetheric : attaque ×0,15.
    let p = pool_fossiles(
        &pb.pool,
        &[fossile("Frigid Fossil"), fossile("Prismatic Fossil")],
    );
    assert_eq!(poids_de(&p, "ColdResist4"), froid * 1600);
    let attaque = pb
        .pool
        .affixes
        .iter()
        .find(|a| {
            a.weight > 0 && a.tags & donnees::TAG_ATTAQUE != 0 && a.tags & donnees::TAG_SORT == 0
        })
        .unwrap();
    let p = pool_fossiles(&pb.pool, &[fossile("Aetheric Fossil")]);
    assert_eq!(poids_de(&p, &attaque.id), attaque.weight * 15);
}

#[test]
fn fossiles_proches_de_la_simulation() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    let voulus = vec![
        voulu(&pb.pool, "IncreasedLife5"),
        voulu(&pb.pool, "ColdResist4"),
    ];
    let goal = Goal::new(&pb.pool, &voulus).unwrap();
    let p = pool_fossiles(
        &pb.pool,
        &[fossile("Pristine Fossil"), fossile("Frigid Fossil")],
    );
    let c = chance_rare(&p, &goal, 86, None, None);
    let s = simuler_rare(&p, &goal, 86, None, None, 100_000);
    proche(c, s, 0.05);
    assert!(c > 5.0 * chance_rare(&pb.pool, &goal, 86, None, None));
}

#[test]
fn methodes_fossiles_par_resonateur() {
    let pb = construire_pool(ANNEAU, 86, None).unwrap();
    let voulus = vec![
        voulu(&pb.pool, "IncreasedLife5"),
        voulu(&pb.pool, "ColdResist4"),
    ];
    let prix: std::collections::BTreeMap<String, f64> = [
        ("Pristine Fossil", 2.0),
        ("Frigid Fossil", 1.0),
        ("Scorched Fossil", 1.0),
        ("Prismatic Fossil", 1.0),
        ("Dense Fossil", 1.0),
        ("Primitive Chaotic Resonator", 1.0),
        ("Potent Chaotic Resonator", 3.0),
        ("Powerful Chaotic Resonator", 10.0),
        ("Prime Chaotic Resonator", 30.0),
    ]
    .iter()
    .map(|(n, p)| (n.to_string(), *p))
    .collect();
    let t = std::time::Instant::now();
    let r = methodes(&pb, &voulus, &prix).unwrap();
    eprintln!("méthodes en {:?}", t.elapsed());
    let fos: Vec<_> = r.methodes.iter().filter(|m| m.genre == "fossile").collect();
    assert!(!fos.is_empty());
    for m in &fos {
        eprintln!("{} {:.5} {:?}", m.nom, m.chance, m.monnaies);
    }
    // Les fossiles qui bloquent un voulu (Scorched : froid, Dense : vie) ne sont jamais proposés.
    assert!(
        fos.iter()
            .all(|m| !m.id.contains("Scorched") && !m.id.contains("Dense"))
    );
    // Pristine + Frigid dans un Potent : la paire attendue, résonateur compté.
    let paire = fos
        .iter()
        .find(|m| m.monnaies.len() == 3 && m.finition.is_none())
        .expect("une combinaison de deux fossiles");
    assert!(paire.id.contains("Pristine Fossil") && paire.id.contains("Frigid Fossil"));
    assert!(
        paire
            .monnaies
            .iter()
            .any(|c| c.nom == "Potent Chaotic Resonator")
    );
    let chaos = r.methodes.iter().find(|m| m.id == "chaos").unwrap();
    assert!(paire.chance > 5.0 * chaos.chance);
}
