//! Bout en bout sur une vraie base : pool (RePoE + poids poe2db) → catalogue → solveur →
//! vérification du plan sur le moteur exact.
//!
//! Base : Suede Bracers, niveau d'objet 45 (pool complet : tous les mods ont un poids).
//! Objectif : Vie maximale (préfixe) et Résistance au feu (suffixe), n'importe quel tier.

use poe2_core::forge::catalogue::actions;
use poe2_core::forge::donnees::{BaseTable, RelevePoids, TableMods, construire_pool};
use poe2_core::forge::solveur::{MacroState, Model, SolveConfig, solve, verify_policy};
use poe2_core::forge::{Goal, ItemState, Rarity, WantedAffix};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Deserialize)]
struct Fixture {
    ilvl: u32,
    base: BaseTable,
    mods: TableMods,
    releve: RelevePoids,
}

#[test]
fn vie_et_feu_sur_gants_dex() {
    let chemin = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/donnees/forge/suede-bracers-45.json"
    );
    let f: Fixture = serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap();
    let pb = construire_pool(&f.mods, &f.base, f.ilvl, &f.releve).unwrap();
    assert!(pb.complet());

    let groupe = |famille: &str| {
        pb.pool
            .affixes
            .iter()
            .find(|a| a.id.starts_with(famille))
            .unwrap_or_else(|| panic!("{famille} absent"))
            .group
    };
    let voulu = |famille| WantedAffix {
        group: groupe(famille),
        family: 0,
        max_tier: u8::MAX,
    };
    let pool = Arc::new(pb.pool.clone());
    let goal = Arc::new(Goal::new(&pool, &[voulu("IncreasedLife"), voulu("FireResist")]).unwrap());

    // Prix fictifs (en Exalted) : seules les monnaies de base, sans Omen.
    let prix: BTreeMap<String, f64> = [
        ("transmute", 0.1),
        ("augment", 0.2),
        ("regal", 0.5),
        ("alchemy", 0.5),
        ("exalt", 1.0),
        ("chaos", 3.0),
        ("annul", 30.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let actives = prix.keys().cloned().collect();
    let acts = actions(&prix, Some(&actives)).unwrap();

    let model = Model::new(pool, goal, 45, acts, 0.5, 0.0);
    let depart = MacroState::empty(Rarity::Normal);
    let sol = solve(
        &model,
        &[depart],
        &SolveConfig::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(sol.converged);
    let v0 = sol.value[sol.id(&depart).unwrap()];
    assert!(v0.is_finite() && v0 > 0.0, "coût espéré {v0}");

    // Le plan suivi sur le moteur exact coûte en moyenne ce que le solveur annonce (à partir de
    // la base en main ; le prix de la base ne compte qu'en cas d'abandon).
    // Mesuré le 07/11/2026 : 12,33 ex annoncés, 12,23 ex sur 20 000 essais, P50 8,5 ex, P90 27,6 ex.
    let r = verify_policy(
        &model,
        &sol,
        ItemState::new(Rarity::Normal, 45),
        20_000,
        10_000,
        7,
        |_, _| true,
    )
    .unwrap();
    assert_eq!(r.censored, 0);
    let ecart = (r.mean_cost - v0).abs() / v0;
    assert!(
        ecart < 0.03,
        "solveur {v0:.3} ex, moteur exact {:.3} ex, écart {:.1} %",
        r.mean_cost,
        ecart * 100.0
    );
    assert!(r.p90_cost >= r.median_cost);
}
