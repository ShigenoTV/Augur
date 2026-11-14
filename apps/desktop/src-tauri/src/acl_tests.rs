//! Chaque commande enregistrée doit être déclarée au build et autorisée par une capability :
//! sinon l'appli publiée répond « Command … not allowed by ACL ».

use std::collections::BTreeSet;

const LIB: &str = include_str!("lib.rs");
const BUILD: &str = include_str!("../build.rs");
const CAPABILITIES: [&str; 2] = [
    include_str!("../capabilities/main.json"),
    include_str!("../capabilities/overlay.json"),
];

fn commandes_enregistrees() -> BTreeSet<String> {
    let debut = LIB
        .find("generate_handler![")
        .expect("generate_handler! absent de lib.rs");
    let fin = debut + LIB[debut..].find(']').expect("generate_handler! non fermé");
    LIB[debut..fin]
        .split(',')
        .filter_map(|c| c.trim().strip_prefix("commands::"))
        .map(str::to_string)
        .collect()
}

fn commandes_declarees() -> BTreeSet<String> {
    let debut = BUILD
        .find(".commands(&[")
        .expect("liste de commandes absente de build.rs");
    let fin = debut
        + BUILD[debut..]
            .find("])")
            .expect("liste de commandes non fermée");
    BUILD[debut..fin]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn commandes_autorisees() -> BTreeSet<String> {
    CAPABILITIES
        .iter()
        .flat_map(|c| {
            let v: serde_json::Value = serde_json::from_str(c).expect("capability illisible");
            v["permissions"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|p| p.as_str().map(str::to_string))
        })
        .filter_map(|p| p.strip_prefix("allow-").map(|c| c.replace('-', "_")))
        .collect()
}

#[test]
fn toutes_les_commandes_sont_declarees_au_build() {
    let manquantes: Vec<_> = commandes_enregistrees()
        .difference(&commandes_declarees())
        .cloned()
        .collect();
    assert!(
        manquantes.is_empty(),
        "absentes de build.rs : {manquantes:?}"
    );
}

#[test]
fn toutes_les_commandes_sont_autorisees() {
    let manquantes: Vec<_> = commandes_enregistrees()
        .difference(&commandes_autorisees())
        .cloned()
        .collect();
    assert!(
        manquantes.is_empty(),
        "absentes des capabilities : {manquantes:?}"
    );
}
