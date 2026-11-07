//! Tests croisés Forge : le pool construit en Rust (`forge::donnees`) et la distribution
//! exacte du moteur doivent redonner les probabilités de `tools/theorycraft/craft_proba.py`.
//!
//! Les cas sont générés par `tools/theorycraft/fixture_forge.py` sur de vraies bases
//! (RePoE 0.5.5.7, poids poe2db du 05/11/2026, CC BY-NC-SA 3.0).

use poe2_core::forge::donnees::{BaseTable, RelevePoids, TableMods, construire_pool};
use poe2_core::forge::{DrawFilter, ItemState, Mod, Rarity};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Fixture {
    ilvl: u32,
    base: BaseTable,
    mods: TableMods,
    releve: RelevePoids,
    cas: Vec<Cas>,
}

#[derive(Deserialize)]
struct Cas {
    presents: Vec<String>,
    attendu: BTreeMap<String, f64>,
}

fn verifier(nom: &str, sans_poids_attendus: usize) {
    let chemin = format!("{}/tests/donnees/forge/{nom}", env!("CARGO_MANIFEST_DIR"));
    let f: Fixture = serde_json::from_str(&std::fs::read_to_string(&chemin).unwrap()).unwrap();
    let pb = construire_pool(&f.mods, &f.base, f.ilvl, &f.releve).unwrap();
    assert_eq!(
        pb.sans_poids.len(),
        sans_poids_attendus,
        "{nom} : mods sans poids"
    );
    assert_eq!(pb.complet(), sans_poids_attendus == 0);

    let index = |id: &str| {
        pb.pool
            .affixes
            .iter()
            .position(|a| a.id == id)
            .unwrap_or_else(|| panic!("{nom} : {id} absent du pool")) as u16
    };
    for cas in &f.cas {
        let mut item = ItemState::new(Rarity::Rare, u8::try_from(f.ilvl).unwrap());
        for id in &cas.presents {
            item.push(Mod {
                idx: index(id),
                fractured: false,
                crafted: false,
            });
        }
        let obtenu: BTreeMap<String, f64> = pb
            .pool
            .distribution(&item, &DrawFilter::default())
            .into_iter()
            .map(|(i, p)| (pb.pool.affixes[i as usize].id.clone(), p))
            .collect();
        assert_eq!(
            obtenu.keys().collect::<Vec<_>>(),
            cas.attendu.keys().collect::<Vec<_>>(),
            "{nom}, présents {:?} : mêmes mods tirables",
            cas.presents
        );
        for (id, p) in &cas.attendu {
            assert!(
                (obtenu[id] - p).abs() < 1e-12,
                "{nom}, présents {:?} : {id} {} au lieu de {p}",
                cas.presents,
                obtenu[id]
            );
        }
    }
}

#[test]
fn robe_int_niveau_82() {
    verifier("vile-robe-82.json", 3);
}

#[test]
fn anneau_niveau_75() {
    verifier("gold-ring-75.json", 5);
}

#[test]
fn gants_dex_niveau_45() {
    verifier("suede-bracers-45.json", 0);
}
