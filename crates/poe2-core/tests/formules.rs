//! Exécute les cas unitaires de formule de `tests/formules/` (racine du dépôt).
//!
//! `AUGUR_RAPPORT_FORMULES=<chemin>` écrit le rapport Markdown pour la CI.

use std::collections::BTreeSet;
use std::path::Path;

use poe2_core::calc::formules::{self, Verdict};

#[test]
fn cas_de_formule() {
    let dossier = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/formules");
    let mut fichiers: Vec<_> = std::fs::read_dir(&dossier)
        .unwrap_or_else(|e| panic!("{} : {e}", dossier.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| matches!(p.extension().and_then(|x| x.to_str()), Some("yaml" | "yml")))
        .collect();
    fichiers.sort();
    assert!(!fichiers.is_empty());

    let mut resultats = Vec::new();
    let mut problemes = Vec::new();
    let mut ids = BTreeSet::new();
    for f in &fichiers {
        let texte = std::fs::read_to_string(f).unwrap();
        match formules::lire(&texte) {
            Err(e) => problemes.push(format!("{} : {e}", f.display())),
            Ok(fichier) => {
                for c in &fichier.cas {
                    if !ids.insert(c.id.clone()) {
                        problemes.push(format!("{} : id en double « {} »", f.display(), c.id));
                    }
                    resultats.push(formules::executer(c));
                }
            }
        }
    }

    let rapport = formules::rapport_markdown(&resultats);
    println!("{rapport}");
    if let Ok(chemin) = std::env::var("AUGUR_RAPPORT_FORMULES") {
        std::fs::write(chemin, &rapport).unwrap();
    }
    let echecs: Vec<_> = resultats
        .iter()
        .filter(|r| matches!(r.verdict, Verdict::Echec | Verdict::Erreur))
        .map(|r| r.id.as_str())
        .collect();
    assert!(problemes.is_empty(), "{}", problemes.join("\n"));
    assert!(
        echecs.is_empty(),
        "cas de formule en échec : {echecs:?}\n\n{rapport}"
    );
}
