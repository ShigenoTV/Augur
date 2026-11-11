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
        objet_depart: None,
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

/// Objet de départ (demande de Max du 10/11/2026) : le plan repart d'un objet déjà en possession.
mod objet_de_depart {
    use super::*;
    use poe2_core::forge::atelier::{ErreurAtelier, analyser_objet};
    use poe2_core::forge::moteur::Rarity;

    const MAGIQUE: &str = "Item Class: Gloves\nRarity: Magic\nAthlete's Ornate Cuffs\n--------\nItem Level: 78\n--------\n\
{ Prefix Modifier \"Athlete's\" (Tier: 1) — Life }\n+136(120-149) to maximum Life\n";

    fn demande(textes: &[&str], depart: Option<String>) -> DemandePlan {
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
        DemandePlan {
            base_id: id,
            ilvl: 78,
            voulus,
            exclues: vec![],
            objet_depart: depart,
        }
    }

    #[test]
    fn analyser_un_objet_magique() {
        let o = analyser_objet(MAGIQUE).unwrap();
        assert_eq!(o.base_nom, "Ornate Cuffs");
        assert_eq!(o.classe, "Gloves");
        assert_eq!(o.ilvl, Some(78));
        assert_eq!(o.rarete, Rarity::Magic);
        assert_eq!(o.mods.len(), 1);
        assert!(o.mods[0].reconnu);

        let rare = analyser_objet(&corpus("reel-rare-gants-avance.txt")).unwrap();
        assert_eq!(rare.rarete, Rarity::Rare);
        assert_eq!(rare.mods.len(), 6);
        assert!(rare.mods.iter().all(|m| m.reconnu), "{:#?}", rare.mods);

        assert!(matches!(
            analyser_objet("n'importe quoi"),
            Err(ErreurAtelier::ObjetDepart(_))
        ));
    }

    #[test]
    fn plan_depuis_un_objet_magique() {
        let voulus = ["maximum Life", "Fire Resistance"];
        let annuler = AtomicBool::new(false);
        let table = TablePrix::indicative();
        let (neuf, _) = planifier(&demande(&voulus, None), &table, &annuler).unwrap();
        assert!(neuf.depart.is_none());
        let (r, actif) =
            planifier(&demande(&voulus, Some(MAGIQUE.into())), &table, &annuler).unwrap();
        let mods = r.depart.as_ref().unwrap();
        assert_eq!(mods[0].voulu, Some(0));
        assert!(
            r.plan.expected_cost < neuf.plan.expected_cost,
            "objet {} / base neuve {}",
            r.plan.expected_cost,
            neuf.plan.expected_cost
        );
        let mc = r.plan.mc.as_ref().unwrap();
        assert!(
            (mc.mean_cost - r.plan.expected_cost).abs() < 0.05 * r.plan.expected_cost,
            "solveur {} / moteur {}",
            r.plan.expected_cost,
            mc.mean_cost
        );
        // l'objet est déjà payé : seules les bases après un abandon s'achètent
        let base = |p: &poe2_core::forge::atelier::ResultatPlan| {
            p.plan
                .shopping
                .iter()
                .find(|l| l.id == "__base")
                .map_or(0.0, |l| l.expected_count)
        };
        assert!(base(&neuf) >= 1.0);
        assert!(base(&r) < base(&neuf));
        // l'overlay suit ensuite le plan sur le même objet
        let c = conseiller(&actif, &parser(MAGIQUE).unwrap()).unwrap();
        assert!(c.conseil.action.is_some());
    }

    /// Un mod que Forge ne retrouve pas compte comme un mauvais mod de son côté ; le moteur exact le
    /// remplace par un mod hors objectif pour la vérification.
    #[test]
    fn mod_inconnu() {
        let texte = format!(
            "{MAGIQUE}{{ Suffix Modifier \"of Nowhere\" (Tier: 1) }}\n+3 to Nothing At All\n"
        );
        let (r, _) = planifier(
            &demande(&["maximum Life", "Fire Resistance"], Some(texte)),
            &TablePrix::indicative(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let mods = r.depart.as_ref().unwrap();
        assert_eq!(mods.len(), 2);
        assert!(!mods[1].reconnu);
        let mc = r
            .plan
            .mc
            .as_ref()
            .expect("objet remplacé pour le moteur exact");
        assert!(
            (mc.mean_cost - r.plan.expected_cost).abs() < 0.05 * r.plan.expected_cost,
            "solveur {} / moteur {}",
            r.plan.expected_cost,
            mc.mean_cost
        );
    }

    #[test]
    fn objet_deja_au_but() {
        let voulus = ["maximum Life", "Fire Resistance", "Lightning Resistance"];
        let (r, _) = planifier(
            &demande(&voulus, Some(corpus("reel-rare-gants-avance.txt"))),
            &TablePrix::indicative(),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(r.plan.expected_cost, 0.0);
    }

    #[test]
    fn texte_vide_base_neuve() {
        let voulus = ["maximum Life"];
        let table = TablePrix::indicative();
        let annuler = AtomicBool::new(false);
        let (r, _) = planifier(&demande(&voulus, Some("  \n".into())), &table, &annuler).unwrap();
        assert!(r.depart.is_none());
    }

    #[test]
    fn objets_refuses() {
        let voulus = ["maximum Life"];
        let table = TablePrix::indicative();
        let annuler = AtomicBool::new(false);
        let erreur =
            |texte: String| match planifier(&demande(&voulus, Some(texte)), &table, &annuler) {
                Err(ErreurAtelier::ObjetDepart(m)) => m,
                Err(e) => panic!("{e}"),
                Ok(_) => panic!("accepté"),
            };
        // autre base
        assert!(erreur(corpus("rare-bague-avance.txt")).contains("pas la base"));
        // autre niveau d'objet
        assert!(erreur(MAGIQUE.replace("Item Level: 78", "Item Level: 70")).contains("niveau"));
        // copie simple
        let simple = "Item Class: Gloves\nRarity: Magic\nAthlete's Ornate Cuffs\n--------\nItem Level: 78\n--------\n+136 to maximum Life\n";
        assert!(erreur(simple.into()).contains("Ctrl+Alt+C"));
    }
}
