//! Overlay de Forge : un vrai objet copié en jeu (Ctrl+Alt+C) suivi sur un plan calculé.

use poe2_core::forge::WantedAffix;
use poe2_core::forge::atelier::{DemandePlan, affixes, bases, planifier};
use poe2_core::forge::prix::TablePrix;
use poe2_core::forge::suivi::{ErreurSuivi, PlanActif, conseiller};
use poe2_core::parser::parser;
use std::sync::atomic::AtomicBool;

fn corpus(nom: &str) -> String {
    let chemin = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/corpus/objets/");
    std::fs::read_to_string(format!("{chemin}{nom}")).unwrap()
}

/// Plan Vie + Feu + Foudre sur Ornate Cuffs niveau 78 (base des gants du corpus).
fn plan(textes: &[&str]) -> PlanActif {
    let id = bases()
        .unwrap()
        .into_iter()
        .find(|b| b.nom == "Ornate Cuffs")
        .unwrap()
        .id;
    let a = affixes(&id, 78).unwrap();
    let voulus = textes
        .iter()
        .map(|t| {
            let g = a.affixes.iter().find(|g| g.libelle.contains(t)).unwrap();
            WantedAffix {
                group: g.group,
                family: g.family,
                max_tier: u8::MAX,
            }
        })
        .collect();
    let demande = DemandePlan {
        base_id: id,
        ilvl: 78,
        voulus,
        exclues: vec![],
    };
    planifier(&demande, &TablePrix::indicative(), &AtomicBool::new(false))
        .unwrap()
        .1
}

#[test]
fn gants_du_corpus_au_but() {
    let actif = plan(&["maximum Life", "Fire Resistance", "Lightning Resistance"]);
    let objet = parser(&corpus("reel-rare-gants-avance.txt")).unwrap();
    let c = conseiller(&actif, &objet).unwrap();
    assert_eq!(c.mods.len(), 6);
    assert!(c.mods.iter().all(|m| m.reconnu), "{:#?}", c.mods);
    let voulus: Vec<u8> = c.mods.iter().filter_map(|m| m.voulu).collect();
    assert_eq!(voulus.len(), 3, "{:#?}", c.mods);
    assert!(c.conseil.goal_reached);
}

#[test]
fn objet_magique_en_cours() {
    let actif = plan(&["maximum Life", "Fire Resistance"]);
    let texte = "Item Class: Gloves\nRarity: Magic\nAthlete's Ornate Cuffs\n--------\nItem Level: 78\n--------\n\
{ Prefix Modifier \"Athlete's\" (Tier: 1) — Life }\n+136(120-149) to maximum Life\n";
    let objet = parser(texte).unwrap();
    let c = conseiller(&actif, &objet).unwrap();
    assert_eq!(c.mods[0].voulu, Some(0));
    assert!(!c.conseil.goal_reached);
    assert!(c.conseil.action.is_some(), "une action est conseillée");
    assert!(c.conseil.cost_to_go.unwrap() < actif.expected_cost);

    // copie simple : on ne sait pas quels mods sont des préfixes
    let simple = "Item Class: Gloves\nRarity: Magic\nAthlete's Ornate Cuffs\n--------\nItem Level: 78\n--------\n+136 to maximum Life\n";
    assert_eq!(
        conseiller(&actif, &parser(simple).unwrap()).unwrap_err(),
        ErreurSuivi::CopieSimple
    );
    // autre base
    let bague = parser(&corpus("rare-bague-avance.txt")).unwrap();
    assert!(matches!(
        conseiller(&actif, &bague),
        Err(ErreurSuivi::AutreBase(_))
    ));
}
