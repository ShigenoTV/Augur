//! Affiche la trace de calcul complète d'un fichier de cas golden.
//!
//! `cargo run -p poe2-core --example trace -- <fichier.yaml> [sortie] [--json]`

use poe2_core::calc::golden;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let Some(chemin) = pos.first() else {
        eprintln!("usage : trace <fichier.yaml> [sortie] [--json]");
        std::process::exit(2);
    };
    let filtre = pos.get(1).map(|s| s.as_str());
    let texte = std::fs::read_to_string(chemin).unwrap_or_else(|e| {
        eprintln!("{chemin} : {e}");
        std::process::exit(1);
    });
    let cas = golden::lire(&texte).unwrap_or_else(|e| {
        eprintln!("{chemin} : {e}");
        std::process::exit(1);
    });
    for c in &cas {
        let r = golden::executer(c);
        println!("══ {} ({:?}) ══", c.id, r.verdict);
        for e in &r.erreurs {
            println!("  erreur : {e}");
        }
        let Some(res) = &r.resultats else { continue };
        let noms: Vec<&str> = match filtre {
            Some(f) => vec![f],
            None => c.attendu.valeurs.keys().map(String::as_str).collect(),
        };
        for nom in noms {
            let nom = if nom == "tooltip_dps" { "dps" } else { nom };
            match res.trace(nom) {
                Some(t) if json => println!("{}", serde_json::to_string_pretty(t).unwrap()),
                Some(t) => println!("{t}"),
                None => println!("  sortie inconnue : {nom}"),
            }
        }
    }
}
