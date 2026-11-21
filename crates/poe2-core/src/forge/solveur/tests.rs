use super::*;
use crate::forge::*;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn aff(id: String, group: u16, slot: Slot, tier: u8, w: u32) -> Affix {
    Affix {
        id: id.clone(),
        name: id.clone(),
        family: id,
        text: String::new(),
        group,
        family_id: 0,
        slot,
        tier,
        req_ilvl: 1,
        weight: w,
        tags: 0,
        desecrated: false,
        cap_shift: (0, 0),
        extra_groups: Vec::new(),
    }
}

/// Pool synthétique : 2 groupes voulus (1 préfixe, 1 suffixe) avec T1/T2, et beaucoup de groupes « inutiles »
/// (l'approximation « exclusion de groupe des mauvais affixes ignorée » devient négligeable).
fn pool(n_bad: u16) -> AffixPool {
    let mut v = vec![
        aff("P_T1".into(), 1, Slot::Prefix, 1, 60),
        aff("P_T2".into(), 1, Slot::Prefix, 2, 140),
        aff("S_T1".into(), 2, Slot::Suffix, 1, 60),
        aff("S_T2".into(), 2, Slot::Suffix, 2, 140),
    ];
    for i in 0..n_bad {
        v.push(aff(format!("bp{i}"), 100 + i, Slot::Prefix, 1, 100));
        v.push(aff(format!("bs{i}"), 500 + i, Slot::Suffix, 1, 100));
    }
    AffixPool::new(v)
}

fn cur(id: &str, kind: CurrencyKind, cost: f64) -> Action {
    Action {
        id: id.into(),
        label: id.into(),
        cost,
        kind: ActionKind::Currency(Currency {
            id: id.into(),
            label: id.into(),
            kind,
            min_mod_level: 0,
            add_slot: None,
            remove_slot: None,
            target: None,
            alt_target: None,
            more_targets: Vec::new(),
            require_tag: None,
            remove_desecrated_only: false,
            remove_lowest_level: false,
            reveal_twice: false,
            requires_rare: false,
            double: false,
            unit_cost: cost,
        }),
    }
}

fn build(n_bad: u16, max_tier: u8, extra: Vec<Action>) -> (Model, Solution) {
    let pool = Arc::new(pool(n_bad));
    let goal = Arc::new(
        Goal::new(
            &pool,
            &[
                WantedAffix {
                    group: 1,
                    family: 0,
                    max_tier,
                },
                WantedAffix {
                    group: 2,
                    family: 0,
                    max_tier,
                },
            ],
        )
        .unwrap(),
    );
    let mut actions = vec![
        cur("transmute", CurrencyKind::Transmute, 0.1),
        cur("augment", CurrencyKind::Augment, 0.1),
        cur("regal", CurrencyKind::Regal, 0.3),
        cur("alchemy", CurrencyKind::Alchemy, 0.4),
        cur("exalt", CurrencyKind::Exalt, 1.0),
        cur("chaos", CurrencyKind::Chaos, 0.8),
        cur("annul", CurrencyKind::Annul, 2.0),
    ];
    actions.extend(extra);
    actions.push(Action {
        id: "abandon".into(),
        label: "Abandon".into(),
        cost: 0.0,
        kind: ActionKind::Abandon,
    });
    let model = Model::new(pool, goal, 80, actions, 0.5, 0.0);
    let sol = solve(
        &model,
        &[MacroState::empty(Rarity::Normal)],
        &SolveConfig::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    (model, sol)
}

fn start_id(model: &Model, sol: &Solution) -> usize {
    {
        let _ = model;
        sol.id(&MacroState::empty(Rarity::Normal)).unwrap()
    }
}

#[test]
fn solver_converges_and_costs_are_finite() {
    let (m, s) = build(20, 1, vec![]);
    assert!(s.converged, "sweeps={}", s.sweeps);
    let v0 = s.value[start_id(&m, &s)];
    assert!(v0.is_finite() && v0 > 0.0);
}

#[test]
fn visits_cost_equals_value_at_root() {
    // contrôle d'intégrité : Σ visites × coût par visite == V(s0)
    let (m, s) = build(20, 1, vec![]);
    let root = start_id(&m, &s);
    let inputs = PlanInputs {
        model: &m,
        sol: &s,
        start: MacroState::empty(Rarity::Normal),
        base_id: "t",
        base_cost: 0.5,
        salvage: 0.0,
        goal_items: vec![
            GoalItem {
                label: "P".into(),
                slot: Slot::Prefix,
                group: 1,
                family_id: 0,
                max_tier: 1,
            },
            GoalItem {
                label: "S".into(),
                slot: Slot::Suffix,
                group: 2,
                family_id: 0,
                max_tier: 1,
            },
        ],
        prices_source: "test".into(),
    };
    let plan = build_plan(&inputs, &PlanConfig::default()).unwrap();
    let rel = (plan.solver.cost_from_visits - s.value[root]).abs() / s.value[root];
    assert!(
        rel < 1e-6,
        "V={} visites={}",
        s.value[root],
        plan.solver.cost_from_visits
    );
    // les probabilités de branches somment à 1 (avec les branches mineures)
    for n in plan.nodes.values() {
        if let CraftNode::Action(a) = n {
            let sum: f64 =
                a.branches.iter().map(|b| b.probability).sum::<f64>() + a.merged_minor_probability;
            assert!((sum - 1.0).abs() < 1e-9, "{} somme={}", a.id, sum);
        }
    }
    assert!(plan.nodes.contains_key("goal"));
}

#[test]
fn analytic_cost_matches_exact_engine_monte_carlo() {
    // Le coût espéré du solveur (abstraction) doit coller au coût mesuré sur le moteur exact.
    let (m, s) = build(40, 2, vec![]);
    let root = start_id(&m, &s);
    let mc = verify_policy(
        &m,
        &s,
        ItemState::new(Rarity::Normal, 80),
        60_000,
        5_000,
        1234,
        |_, _| true,
    )
    .unwrap();
    let v = s.value[root];
    let se = (mc.ci95_mean.1 - mc.ci95_mean.0) / 3.92;
    let rel = (mc.mean_cost - v).abs() / v;
    assert!(mc.censored == 0);
    assert!(
        rel < 0.06,
        "V analytique={v:.3} MC={:.3} ± {:.3} (écart {:.1} %)",
        mc.mean_cost,
        se,
        rel * 100.0
    );
}

#[test]
fn fracture_and_omens_are_handled() {
    let mut extra = vec![cur("fracture", CurrencyKind::Fracture, 3.0)];
    let mut dext = cur("exalt+dextral", CurrencyKind::Exalt, 1.6);
    if let ActionKind::Currency(c) = &mut dext.kind {
        c.add_slot = Some(Slot::Suffix);
    }
    let mut sinis = cur("annul+sinistral", CurrencyKind::Annul, 3.0);
    if let ActionKind::Currency(c) = &mut sinis.kind {
        c.remove_slot = Some(Slot::Prefix);
    }
    extra.push(dext);
    extra.push(sinis);
    let (m, s) = build(40, 2, extra);
    let root = start_id(&m, &s);
    let (v_plain, _) = {
        let (m0, s0) = build(40, 2, vec![]);
        (s0.value[start_id(&m0, &s0)], 0)
    };
    // plus d'options ne peut que réduire le coût optimal
    assert!(
        s.value[root] <= v_plain + 1e-9,
        "{} > {}",
        s.value[root],
        v_plain
    );
    let mc = verify_policy(
        &m,
        &s,
        ItemState::new(Rarity::Normal, 80),
        40_000,
        5_000,
        99,
        |_, _| true,
    )
    .unwrap();
    let rel = (mc.mean_cost - s.value[root]).abs() / s.value[root];
    assert!(
        rel < 0.07,
        "V={:.3} MC={:.3} (écart {:.1} %)",
        s.value[root],
        mc.mean_cost,
        rel * 100.0
    );
}

#[test]
fn unreachable_goal_is_reported() {
    // ilvl trop bas : les affixes T1 exigent un niveau supérieur
    let mut p = pool(5);
    for a in p.affixes.iter_mut().filter(|a| a.id.ends_with("T1")) {
        a.req_ilvl = 90;
    }
    let pool = Arc::new(p);
    let goal = Arc::new(
        Goal::new(
            &pool,
            &[WantedAffix {
                group: 1,
                family: 0,
                max_tier: 1,
            }],
        )
        .unwrap(),
    );
    let actions = vec![
        cur("transmute", CurrencyKind::Transmute, 0.1),
        cur("exalt", CurrencyKind::Exalt, 1.0),
    ];
    let m = Model::new(pool, goal, 80, actions, 0.5, 0.0);
    let r = solve(
        &m,
        &[MacroState::empty(Rarity::Normal)],
        &SolveConfig::default(),
        &AtomicBool::new(false),
    );
    assert!(r.is_err());
}

#[test]
fn advise_from_a_mid_craft_item() {
    let (m, s) = build(20, 1, vec![]);
    let mut st = MacroState::empty(Rarity::Rare);
    st.held = 0b01;
    st.bad_s = 2;
    // état possiblement non énuméré depuis s0 : on résout depuis lui
    let s2 = solve(&m, &[st], &SolveConfig::default(), &AtomicBool::new(false)).unwrap();
    let adv = advise(&m, &s2, &st, &["P".into(), "S".into()]).unwrap();
    assert!(adv.action.is_some() && adv.cost_to_go.unwrap() > 0.0);
    let _ = s;
}

/// Bout en bout, base à plafond décalé (Penumbra : 5 préfixes / 1 suffixe en Rare) : un objectif à 4
/// préfixes, impossible sur une base ordinaire, devient atteignable ; le coût du solveur colle à celui
/// mesuré sur le moteur exact, qui applique le même plafond.
#[test]
fn shifted_base_cap_allows_four_prefixes_and_matches_monte_carlo() {
    let mut v: Vec<Affix> = (1..=4)
        .map(|g| aff(format!("P{g}"), g, Slot::Prefix, 1, 600))
        .collect();
    for i in 0..12 {
        v.push(aff(format!("bp{i}"), 100 + i, Slot::Prefix, 1, 100));
        v.push(aff(format!("bs{i}"), 500 + i, Slot::Suffix, 1, 100));
    }
    let wanted: Vec<WantedAffix> = (1..=4)
        .map(|g| WantedAffix {
            group: g,
            family: 0,
            max_tier: 1,
        })
        .collect();
    assert!(
        Goal::new(&AffixPool::new(v.clone()), &wanted).is_err(),
        "4 préfixes impossibles sur une base 3/3"
    );

    let pool = Arc::new(AffixPool {
        affixes: v,
        cap_delta: (2, -2),
        ..AffixPool::default()
    });
    let goal = Arc::new(Goal::new(&pool, &wanted).unwrap());
    let mut actions = vec![
        cur("transmute", CurrencyKind::Transmute, 0.1),
        cur("augment", CurrencyKind::Augment, 0.1),
        cur("regal", CurrencyKind::Regal, 0.3),
        cur("exalt", CurrencyKind::Exalt, 1.0),
        cur("chaos", CurrencyKind::Chaos, 0.8),
        cur("annul", CurrencyKind::Annul, 2.0),
    ];
    actions.push(Action {
        id: "abandon".into(),
        label: "Abandon".into(),
        cost: 0.0,
        kind: ActionKind::Abandon,
    });
    let m = Model::new(pool, goal, 80, actions, 0.5, 0.0);
    let s = solve(
        &m,
        &[MacroState::empty(Rarity::Normal)],
        &SolveConfig::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(s.converged);
    let v0 = s.value[start_id(&m, &s)];
    assert!(v0.is_finite() && v0 > 0.0);
    let mc = verify_policy(
        &m,
        &s,
        ItemState::new(Rarity::Normal, 80),
        40_000,
        5_000,
        4321,
        |_, _| true,
    )
    .unwrap();
    assert!(mc.censored == 0);
    let rel = (mc.mean_cost - v0).abs() / v0;
    assert!(
        rel < 0.07,
        "V={v0:.3} MC={:.3} (écart {:.1} %)",
        mc.mean_cost,
        rel * 100.0
    );
}

/// Budget, bout en bout : Transmutation (0,1) puis, si raté, abandon (base 0,5) et on recommence. Une
/// Transmutation pose le préfixe voulu avec p = 200/1000 = 0,2, donc avec k tentatives réussies au plus
/// P(coût ≤ 0,6·k − 0,5) = 1 − 0,8^k. La politique du solveur, rejouée sur le moteur exact, doit donner
/// cette loi du coût (quantiles, P90, probabilité de réussir avec un budget donné).
#[test]
fn budget_success_probability_matches_closed_form() {
    let pool = Arc::new(pool(3));
    let goal = Arc::new(
        Goal::new(
            &pool,
            &[WantedAffix {
                group: 1,
                family: 0,
                max_tier: 2,
            }],
        )
        .unwrap(),
    );
    let actions = vec![
        cur("transmute", CurrencyKind::Transmute, 0.1),
        Action {
            id: "abandon".into(),
            label: "Abandon".into(),
            cost: 0.0,
            kind: ActionKind::Abandon,
        },
    ];
    let m = Model::new(pool, goal, 80, actions, 0.5, 0.0);
    let s = solve(
        &m,
        &[MacroState::empty(Rarity::Normal)],
        &SolveConfig::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    // espérance : 5 tentatives, 4 abandons → 5 × 0,1 + 4 × 0,5 = 2,5
    assert!(
        (s.value[start_id(&m, &s)] - 2.5).abs() < 1e-6,
        "V={}",
        s.value[start_id(&m, &s)]
    );
    let mc = verify_policy(
        &m,
        &s,
        ItemState::new(Rarity::Normal, 80),
        100_000,
        5_000,
        777,
        |_, _| true,
    )
    .unwrap();
    assert_eq!(mc.censored, 0);
    assert_eq!(mc.cost_quantiles.len(), QUANTILE_STEPS + 1);
    assert!(
        mc.cost_quantiles.windows(2).all(|w| w[0] <= w[1]),
        "quantiles croissants"
    );
    let exact = |budget: f64| {
        let k = ((budget + 0.5) / 0.6 + 1e-9).floor().max(0.0);
        1.0 - 0.8f64.powf(k)
    };
    for budget in [0.05, 0.1, 0.7, 1.3, 2.5, 4.3, 6.1, 10.0, 20.0] {
        let p = mc.success_probability(budget);
        assert!(
            (p - exact(budget)).abs() < 0.012,
            "budget {budget} : {p:.4} contre {:.4} attendu",
            exact(budget)
        );
    }
    // 1 − 0,8^k ≥ 0,9 dès k = 11 tentatives : 11 × 0,1 + 10 × 0,5 = 6,1
    assert!((mc.p90_cost - 6.1).abs() < 1e-4, "P90 = {}", mc.p90_cost);
    assert_eq!(mc.success_probability(-1.0), 0.0);
    assert!(mc.success_probability(1e9) > 0.999);
}

/// Une monnaie que le moteur exact refuse n'est pas consommée (rien à payer), et un plan qui la redemande
/// sans fin interrompt l'essai vite, au lieu de facturer `max_steps` fois la monnaie.
#[test]
fn refused_currency_is_not_charged_and_stops_the_trial() {
    use rand::SeedableRng;
    let (m, mut s) = build(20, 1, vec![]);
    let root = start_id(&m, &s);
    // plan faussé : Exalted sur la base Normale, que le moteur exact refuse toujours
    s.policy[root] = m.actions.iter().position(|a| a.id == "exalt").unwrap() as u32;
    let mut rng = rand::rngs::SmallRng::seed_from_u64(1);
    let (cost, steps, abandons, censored) =
        super::verify::one_trial(&m, &s, ItemState::new(Rarity::Normal, 80), 20_000, &mut rng);
    assert!(censored);
    assert_eq!((cost, abandons), (0.0, 0));
    assert!(steps <= 64, "essai interrompu après {steps} étapes");
}

/// Mod voulu à plusieurs groupes (Transcendent Alloy : Force, Dextérité, Intelligence) : tout mod de l'un de
/// ses groupes bloque la zone ; un second bloqueur compte comme un mauvais affixe, pour garder le bon
/// nombre de mods.
#[test]
fn zone_d_un_mod_a_plusieurs_groupes() {
    let mut voulu = aff("Alloy".into(), 1, Slot::Suffix, 1, 0);
    voulu.extra_groups = vec![2, 3];
    let pool = AffixPool::new(vec![
        voulu,
        aff("Dex".into(), 2, Slot::Suffix, 1, 100),
        aff("Int".into(), 3, Slot::Suffix, 1, 100),
        aff("Vie".into(), 4, Slot::Prefix, 1, 100),
    ]);
    let goal = Goal::new(
        &pool,
        &[WantedAffix {
            group: 1,
            family: 0,
            max_tier: 1,
        }],
    )
    .unwrap();
    assert_eq!(goal.classify(&pool, 0), Class::Wanted(0));
    assert_eq!(goal.classify(&pool, 1), Class::Blocked(0));
    assert_eq!(goal.classify(&pool, 2), Class::Blocked(0));
    assert_eq!(goal.classify(&pool, 3), Class::Other);
    let mut it = ItemState::new(Rarity::Rare, 80);
    for idx in [1, 2, 3] {
        it.push(Mod {
            idx,
            fractured: false,
            crafted: false,
            desecrated: false,
        });
    }
    let s = state::project(&goal, &pool, &it).unwrap();
    assert_eq!((s.blocked, s.bad_s, s.bad_p), (1, 1, 1));
    assert_eq!(s.total(), 3);
    // deux voulus dont les groupes se recouvrent : objectif refusé
    assert!(
        Goal::new(
            &pool,
            &[
                WantedAffix {
                    group: 1,
                    family: 0,
                    max_tier: 1,
                },
                WantedAffix {
                    group: 2,
                    family: 0,
                    max_tier: 1,
                },
            ],
        )
        .is_err()
    );
}

/// Omens du 21/11/2026 (Greater Exaltation / Annulment, Coronation, Alchemy) : la loi d'un essai dans le
/// modèle abstrait colle à celle du moteur exact, depuis plusieurs objets. (Whittling, déjà dans le
/// solveur, n'est pas comparé ici : le modèle prend la loi moyenne des niveaux des mods anonymes.)
#[test]
fn nouveaux_omens_meme_loi_que_le_moteur() {
    use rand::SeedableRng;
    let avec = |id: &str, kind: CurrencyKind, f: &dyn Fn(&mut Currency)| {
        let mut a = cur(id, kind, 1.0);
        if let ActionKind::Currency(c) = &mut a.kind {
            f(c);
        }
        a
    };
    let actions = vec![
        avec("exalt_x2", CurrencyKind::Exalt, &|c| c.double = true),
        avec("exalt_x2_suffixes", CurrencyKind::Exalt, &|c| {
            c.double = true;
            c.add_slot = Some(Slot::Suffix);
        }),
        avec("annul_x2", CurrencyKind::Annul, &|c| c.double = true),
        avec("annul_x2_prefixes", CurrencyKind::Annul, &|c| {
            c.double = true;
            c.remove_slot = Some(Slot::Prefix);
        }),
        avec("regal_prefixe", CurrencyKind::Regal, &|c| {
            c.add_slot = Some(Slot::Prefix)
        }),
        avec("alchemy_prefixes", CurrencyKind::Alchemy, &|c| {
            c.add_slot = Some(Slot::Prefix)
        }),
        avec("alchemy_suffixes", CurrencyKind::Alchemy, &|c| {
            c.add_slot = Some(Slot::Suffix)
        }),
    ];
    let pool = Arc::new(pool(30));
    let goal = Arc::new(
        Goal::new(
            &pool,
            &[
                WantedAffix {
                    group: 1,
                    family: 0,
                    max_tier: 1,
                },
                WantedAffix {
                    group: 2,
                    family: 0,
                    max_tier: 1,
                },
            ],
        )
        .unwrap(),
    );
    let model = Model::new(pool.clone(), goal, 80, actions, 0.5, 0.0);
    let idx = |id: &str| pool.affixes.iter().position(|a| a.id == id).unwrap() as AffixIdx;
    let objet = |rarete: Rarity, ids: &[&str]| {
        let mut it = ItemState::new(rarete, 80);
        for id in ids {
            it.push(Mod {
                idx: idx(id),
                fractured: false,
                crafted: false,
                desecrated: false,
            });
        }
        it
    };
    let departs = [
        objet(Rarity::Normal, &[]),
        objet(Rarity::Magic, &["P_T1"]),
        objet(Rarity::Rare, &["P_T1", "bs3"]),
        objet(Rarity::Rare, &["P_T1", "bp1", "bp2", "bs4"]),
        objet(Rarity::Rare, &["S_T1", "bp5", "bs6", "bs7", "bp8"]),
    ];
    let mut rng = rand::rngs::SmallRng::seed_from_u64(21);
    let mut compares = 0;
    for depart in departs {
        let s = model.project(&depart).unwrap();
        for (ai, act) in model.actions.iter().enumerate() {
            let ActionKind::Currency(c) = &act.kind else {
                continue;
            };
            let mut tr = Vec::new();
            model.outcomes(s, ai, &mut tr);
            const N: usize = 60_000;
            let mut emp: Vec<(MacroState, f64)> = Vec::new();
            let mut appliquee = 0;
            for _ in 0..N {
                let mut it = depart;
                // un Exalted sous Omen sans mod tirable de ce côté est « appliqué » sans rien changer par
                // le moteur, refusé par le modèle (comportement antérieur à ces Omens) : non compté
                if pool.apply(&mut it, c, &mut rng) == Outcome::Applied
                    && ((it.len(), it.rarity) != (depart.len(), depart.rarity))
                {
                    appliquee += 1;
                    let t = model.project(&it).unwrap();
                    match emp.iter_mut().find(|e| e.0 == t) {
                        Some(e) => e.1 += 1.0 / N as f64,
                        None => emp.push((t, 1.0 / N as f64)),
                    }
                }
            }
            assert_eq!(
                tr.is_empty(),
                appliquee == 0,
                "{} applicable dans un seul des deux, depuis {s:?}",
                act.id
            );
            for t in &tr {
                let e = emp.iter().find(|e| e.0 == t.to).map_or(0.0, |e| e.1);
                assert!(
                    (t.p - e).abs() < 0.02,
                    "{} depuis {s:?} vers {:?} : modèle {:.4}, moteur {:.4}",
                    act.id,
                    t.to,
                    t.p,
                    e
                );
                compares += 1;
            }
            let couvert: f64 = tr.iter().map(|t| t.p).sum();
            assert!(tr.is_empty() || (couvert - 1.0).abs() < 1e-9);
        }
    }
    assert!(compares > 50, "{compares} comparaisons");
}

/// Plan complet avec les Omens du 21/11/2026 : jamais plus cher que sans eux, et la politique simulée
/// sur le moteur exact retrouve le coût calculé.
#[test]
fn plan_avec_les_nouveaux_omens() {
    let avec = |id: &str, kind: CurrencyKind, cout: f64, f: &dyn Fn(&mut Currency)| {
        let mut a = cur(id, kind, cout);
        if let ActionKind::Currency(c) = &mut a.kind {
            f(c);
        }
        a
    };
    let extra = vec![
        avec("exalt_x2", CurrencyKind::Exalt, 1.3, &|c| c.double = true),
        avec("exalt_x2_suffixes", CurrencyKind::Exalt, 1.6, &|c| {
            c.double = true;
            c.add_slot = Some(Slot::Suffix);
        }),
        avec("annul_x2", CurrencyKind::Annul, 2.5, &|c| c.double = true),
        avec("regal_prefixe", CurrencyKind::Regal, 0.5, &|c| {
            c.add_slot = Some(Slot::Prefix)
        }),
        avec("alchemy_suffixes", CurrencyKind::Alchemy, 0.6, &|c| {
            c.add_slot = Some(Slot::Suffix)
        }),
    ];
    let (m, s) = build(40, 2, extra);
    let root = start_id(&m, &s);
    let (m0, s0) = build(40, 2, vec![]);
    let v_plain = s0.value[start_id(&m0, &s0)];
    assert!(s.value[root] <= v_plain + 1e-9);
    let mc = verify_policy(
        &m,
        &s,
        ItemState::new(Rarity::Normal, 80),
        40_000,
        5_000,
        7,
        |_, _| true,
    )
    .unwrap();
    let rel = (mc.mean_cost - s.value[root]).abs() / s.value[root];
    assert!(
        rel < 0.07,
        "V={:.3} MC={:.3} (écart {:.1} %)",
        s.value[root],
        mc.mean_cost,
        rel * 100.0
    );
}

/// Omen of Sinistral Alchemy : 3 préfixes et 1 suffixe ; Omen of Sinistral Crystallisation : l'Essence
/// Perfect retire un préfixe, même quand son mod est un suffixe et que le côté des suffixes a de la place.
#[test]
fn alchemy_et_crystallisation_sur_le_moteur() {
    use rand::SeedableRng;
    let p = pool(10);
    let mut rng = rand::rngs::SmallRng::seed_from_u64(3);
    let ActionKind::Currency(mut alch) = cur("a", CurrencyKind::Alchemy, 1.0).kind else {
        panic!()
    };
    alch.add_slot = Some(Slot::Prefix);
    for _ in 0..200 {
        let mut it = ItemState::new(Rarity::Normal, 80);
        assert_eq!(p.apply(&mut it, &alch, &mut rng), Outcome::Applied);
        assert_eq!(
            (p.count(&it, Slot::Prefix), p.count(&it, Slot::Suffix)),
            (3, 1)
        );
    }
    let ActionKind::Currency(mut ess) = cur("e", CurrencyKind::Essence, 1.0).kind else {
        panic!()
    };
    let cible = p.affixes.iter().position(|a| a.id == "S_T1").unwrap() as AffixIdx;
    ess.target = Some(cible);
    ess.requires_rare = true;
    ess.remove_slot = Some(Slot::Prefix);
    let idx = |id: &str| p.affixes.iter().position(|a| a.id == id).unwrap() as AffixIdx;
    for _ in 0..200 {
        let mut it = ItemState::new(Rarity::Rare, 80);
        for id in ["bp1", "bp2", "bp3", "bs1"] {
            it.push(Mod {
                idx: idx(id),
                fractured: false,
                crafted: false,
                desecrated: false,
            });
        }
        assert_eq!(p.apply(&mut it, &ess, &mut rng), Outcome::Applied);
        assert_eq!(
            (p.count(&it, Slot::Prefix), p.count(&it, Slot::Suffix)),
            (2, 2)
        );
    }
}

/// Pool de `pool(30)` plus des mods de liche (Désécration) : 12 préfixes et 12 suffixes inutiles, et un
/// suffixe de liche voulu (groupe 900).
fn pool_desecration() -> AffixPool {
    let mut p = pool(30);
    for i in 0..12 {
        for (slot, g) in [(Slot::Prefix, 700), (Slot::Suffix, 800)] {
            let mut a = aff(format!("d{g}_{i}"), g + i, slot, 1, 1);
            a.desecrated = true;
            p.affixes.push(a);
        }
    }
    let mut a = aff("D_VOULU".into(), 900, Slot::Suffix, 1, 1);
    a.desecrated = true;
    a.tags = crate::forge::donnees::TAG_KURGAL;
    p.affixes.push(a);
    p
}

fn os(id: &str, f: &dyn Fn(&mut Currency)) -> Action {
    let mut a = cur(id, CurrencyKind::Desecrate, 3.0);
    if let ActionKind::Currency(c) = &mut a.kind {
        f(c);
    }
    a
}

/// La loi de la révélation du modèle (options indépendantes, poids inutile décalé option après option)
/// colle au moteur exact (tirage sans remise,
/// choix du joueur guidé par l'objectif), Omens compris.
#[test]
fn desecration_meme_loi_que_le_moteur() {
    use rand::SeedableRng;
    let pool = Arc::new(pool_desecration());
    let goal = Arc::new(
        Goal::new(
            &pool,
            &[
                WantedAffix {
                    group: 1,
                    family: 0,
                    max_tier: 1,
                },
                WantedAffix {
                    group: 900,
                    family: 0,
                    max_tier: 1,
                },
            ],
        )
        .unwrap(),
    );
    let actions = vec![
        os("os", &|_| {}),
        os("os_dextral", &|c| c.add_slot = Some(Slot::Suffix)),
        os("os_kurgal", &|c| {
            c.require_tag = Some(crate::forge::donnees::TAG_KURGAL)
        }),
        os("os_echos", &|c| c.reveal_twice = true),
        os("os_sinistral", &|c| c.add_slot = Some(Slot::Prefix)),
    ];
    let model = Model::new(pool.clone(), goal.clone(), 80, actions, 0.5, 0.0);
    let idx = |id: &str| pool.affixes.iter().position(|a| a.id == id).unwrap() as AffixIdx;
    let objet = |ids: &[&str]| {
        let mut it = ItemState::new(Rarity::Rare, 80);
        for id in ids {
            it.push(Mod {
                idx: idx(id),
                fractured: false,
                crafted: false,
                desecrated: false,
            });
        }
        it
    };
    let departs = [
        objet(&["bs3"]),
        objet(&["bp1", "bp2", "bs4"]),
        objet(&["S_T1", "bs6", "bs7"]),
        objet(&["bp1", "bp2", "bp3", "bs4", "bs5", "bs6"]),
    ];
    let mut rng = rand::rngs::SmallRng::seed_from_u64(77);
    let mut compares = 0;
    for depart in departs {
        let s = model.project(&depart).unwrap();
        for (ai, act) in model.actions.iter().enumerate() {
            let ActionKind::Currency(c) = &act.kind else {
                continue;
            };
            let mut tr = Vec::new();
            model.outcomes(s, ai, &mut tr);
            const N: usize = 40_000;
            let mut emp: Vec<(MacroState, f64)> = Vec::new();
            for _ in 0..N {
                let mut it = depart;
                if pool.apply_pour(&mut it, c, &mut rng, Some(&goal)) == Outcome::Applied {
                    let t = model.project(&it).unwrap();
                    match emp.iter_mut().find(|e| e.0 == t) {
                        Some(e) => e.1 += 1.0 / N as f64,
                        None => emp.push((t, 1.0 / N as f64)),
                    }
                }
            }
            for t in &tr {
                let e = emp.iter().find(|e| e.0 == t.to).map_or(0.0, |e| e.1);
                assert!(
                    (t.p - e).abs() < 0.02,
                    "{} depuis {s:?} vers {:?} : modèle {:.4}, moteur {:.4}",
                    act.id,
                    t.to,
                    t.p,
                    e
                );
                compares += 1;
            }
            let couvert: f64 = tr.iter().map(|t| t.p).sum();
            assert!(!tr.is_empty() && (couvert - 1.0).abs() < 1e-9, "{}", act.id);
            // le mod révélé est marqué Désécré : pas de seconde Désécration
            assert!(tr.iter().all(|t| t.to.desecrated));
        }
    }
    assert!(compares > 40, "{compares} comparaisons");
}

/// Plan avec les os : le mod de liche voulu n'est accessible que par eux ; la politique simulée sur le
/// moteur exact retrouve le coût calculé.
#[test]
fn plan_avec_desecration() {
    let pool = Arc::new(pool_desecration());
    let goal = Arc::new(
        Goal::new(
            &pool,
            &[
                WantedAffix {
                    group: 1,
                    family: 0,
                    max_tier: 2,
                },
                WantedAffix {
                    group: 900,
                    family: 0,
                    max_tier: 1,
                },
            ],
        )
        .unwrap(),
    );
    let mut actions = vec![
        cur("transmute", CurrencyKind::Transmute, 0.1),
        cur("augment", CurrencyKind::Augment, 0.1),
        cur("regal", CurrencyKind::Regal, 0.3),
        cur("exalt", CurrencyKind::Exalt, 1.0),
        cur("chaos", CurrencyKind::Chaos, 0.8),
        os("os", &|_| {}),
        os("os_kurgal", &|c| {
            c.require_tag = Some(crate::forge::donnees::TAG_KURGAL);
            c.add_slot = Some(Slot::Suffix);
        }),
    ];
    let mut light = cur("annul_light", CurrencyKind::Annul, 2.0);
    if let ActionKind::Currency(c) = &mut light.kind {
        c.remove_desecrated_only = true;
    }
    actions.push(light);
    actions.push(Action {
        id: "abandon".into(),
        label: "Abandon".into(),
        cost: 0.0,
        kind: ActionKind::Abandon,
    });
    let model = Model::new(pool, goal, 80, actions, 0.5, 0.0);
    let sol = solve(
        &model,
        &[MacroState::empty(Rarity::Normal)],
        &SolveConfig::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let v = sol.value[start_id(&model, &sol)];
    assert!(v.is_finite(), "objectif atteignable par les os");
    let mc = verify_policy(
        &model,
        &sol,
        ItemState::new(Rarity::Normal, 80),
        30_000,
        5_000,
        99,
        |_, _| true,
    )
    .unwrap();
    let rel = (mc.mean_cost - v).abs() / v;
    assert!(mc.censored == 0);
    assert!(
        rel < 0.06,
        "V analytique={v:.3} MC={:.3} (écart {:.1} %)",
        mc.mean_cost,
        rel * 100.0
    );
}

/// Révélation : un mod de liche du même groupe qu'un mod normal (Strength and Dexterity et Dexterity)
/// n'écarte pas ce mod du pool normal, tiré à part (émulateur de Craft of Exile, `getRevealPicks`).
#[test]
fn revelation_liche_n_ecarte_pas_le_pool_normal() {
    use rand::SeedableRng;
    let mut p = pool(4);
    let mut liche = aff("D_S2".into(), 2, Slot::Suffix, 1, 1);
    liche.desecrated = true;
    liche.family_id = 1;
    p.affixes.push(liche);
    let pool = AffixPool::new(p.affixes);
    let c = match os("os", &|_| {}).kind {
        ActionKind::Currency(c) => c,
        _ => unreachable!(),
    };
    let it = ItemState::new(Rarity::Rare, 80);
    let mut rng = rand::rngs::SmallRng::seed_from_u64(5);
    let (mut avec, n) = (0, 4000);
    for _ in 0..n {
        let o = pool.reveal_options(&it, &c, Slot::Suffix, &mut rng);
        assert_eq!(pool.affixes[o[0] as usize].id, "D_S2");
        avec += o
            .iter()
            .any(|&i| pool.affixes[i as usize].group == 2 && !pool.affixes[i as usize].desecrated)
            as usize;
    }
    // S_T1 + S_T2 pèsent 200 sur 600 : présents dans environ 1 − (4/6 × 3/5) = 60 % des révélations
    let part = avec as f64 / n as f64;
    assert!((0.5..0.7).contains(&part), "part {part}");
}
