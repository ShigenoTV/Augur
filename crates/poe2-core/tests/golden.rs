//! Exécute tous les cas golden de `tests/golden/` (racine du dépôt).
//!
//! Variable d'environnement facultative : `AUGUR_RAPPORT_GOLDEN=<chemin>`
//! écrit le rapport Markdown (pour le commentaire de PR en CI).

use std::path::{Path, PathBuf};

use poe2_core::calc::golden::{self, Verdict};

fn fichiers_yaml(dossier: &Path, acc: &mut Vec<PathBuf>) {
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return;
    };
    for e in entrees.flatten() {
        let p = e.path();
        if p.is_dir() {
            fichiers_yaml(&p, acc);
        } else if matches!(p.extension().and_then(|x| x.to_str()), Some("yaml" | "yml")) {
            acc.push(p);
        }
    }
}

#[test]
fn cas_golden() {
    let racine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden");
    let mut fichiers = Vec::new();
    fichiers_yaml(&racine, &mut fichiers);
    fichiers.sort();
    assert!(
        !fichiers.is_empty(),
        "aucun cas golden trouvé dans {}",
        racine.display()
    );

    let mut resultats = Vec::new();
    let mut erreurs_lecture = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for f in &fichiers {
        let texte = std::fs::read_to_string(f).expect("lecture du fichier");
        match golden::lire(&texte) {
            Err(e) => erreurs_lecture.push(format!("{} : {e}", f.display())),
            Ok(cas) => {
                for c in cas {
                    if !ids.insert(c.id.clone()) {
                        erreurs_lecture.push(format!(
                            "{} : id en double « {} »",
                            f.display(),
                            c.id
                        ));
                    }
                    resultats.push(golden::executer(&c));
                }
            }
        }
    }

    let rapport = golden::rapport_markdown(&resultats);
    println!("{rapport}");
    if let Ok(chemin) = std::env::var("AUGUR_RAPPORT_GOLDEN") {
        std::fs::write(chemin, &rapport).expect("écriture du rapport");
    }

    let echecs: Vec<_> = resultats
        .iter()
        .filter(|r| matches!(r.verdict, Verdict::Echec | Verdict::Erreur))
        .map(|r| r.id.as_str())
        .collect();
    assert!(
        erreurs_lecture.is_empty(),
        "fichiers illisibles :\n{}",
        erreurs_lecture.join("\n")
    );
    assert!(
        echecs.is_empty(),
        "cas golden en échec : {echecs:?}\n\n{rapport}"
    );
}
