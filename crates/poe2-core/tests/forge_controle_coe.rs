//! Export des pools de Forge pour le contrôle avec Craft of Exile (`tools/theorycraft/coe_controle.py`).
//!
//!     POOLS=pools.jsonl cargo test -p poe2-core --release --test forge_controle_coe -- --ignored

use poe2_core::forge::atelier::{bases, donnees};
use poe2_core::forge::donnees::construire_pool;
use std::io::Write;

#[test]
#[ignore = "export manuel, voir l'en-tête"]
fn exporter_pools() {
    let chemin = std::env::var("POOLS").expect("variable POOLS");
    let d = donnees().unwrap();
    let mut f = std::io::BufWriter::new(std::fs::File::create(chemin).unwrap());
    for b in bases().unwrap() {
        let table = d.bases.bases.iter().find(|x| x.id == b.id).unwrap();
        for ilvl in [45u32, 65, 75, 82] {
            let pb = construire_pool(&d.mods, table, ilvl, &d.releve).unwrap();
            let mods: Vec<_> = pb
                .pool
                .affixes
                .iter()
                .filter(|a| !a.desecrated)
                .map(|a| (a.id.clone(), format!("{:?}", a.slot), a.weight))
                .collect();
            let ligne = serde_json::json!({"nom": b.nom, "ilvl": ilvl, "mods": mods});
            writeln!(f, "{ligne}").unwrap();
        }
    }
}
