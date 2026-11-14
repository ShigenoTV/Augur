//! Recherche d'un objet de PoE 1 sur le site de trade officiel.
//!
//! Augur ne fait aucune requête au site : il construit la requête (même format
//! que le site de PoE 2) et l'appli ouvre `/trade/search/<ligue>?q=<requête>`
//! dans le navigateur du joueur.
//!
//! - unique : même nom et même base, avec au moins ses liens s'il en a 5 ou 6 ;
//! - devise, carte, fragment : même type ;
//! - rare, magique, normal : même base, mêmes influences, niveau d'objet au moins
//!   égal. Les mods ne sont pas encore cherchés (il faudra les identifiants de
//!   stats du site de trade de PoE 1).

use serde_json::{Map, Value, json};

use super::liens;
use crate::parser::{Objet, Rarete};

/// Adresse de la recherche du site de trade de PoE 1 (la ligue suit).
pub const SITE: &str = "https://www.pathofexile.com/trade/search/";

/// Filtre du site de trade pour chaque influence ou marque de l'objet.
fn filtre_influence(influence: &str) -> Option<&'static str> {
    Some(match influence {
        "Shaper Item" => "shaper_item",
        "Elder Item" => "elder_item",
        "Crusader Item" => "crusader_item",
        "Hunter Item" => "hunter_item",
        "Redeemer Item" => "redeemer_item",
        "Warlord Item" => "warlord_item",
        "Synthesised Item" => "synthesised_item",
        "Fractured Item" => "fractured_item",
        _ => return None,
    })
}

/// Requête du site de trade pour cet objet, et ce qu'elle cherche (pour l'overlay).
pub fn requete(objet: &Objet) -> (Value, Vec<String>) {
    let mut query = Map::new();
    query.insert("status".into(), json!({ "option": "online" }));
    let mut filters = Map::new();
    let mut resume = Vec::new();
    match objet.rarete {
        Rarete::Unique => {
            if let Some(nom) = &objet.nom {
                query.insert("name".into(), json!(nom));
                resume.push(nom.clone());
            }
            query.insert("type".into(), json!(objet.base));
            let n = liens(objet);
            if n >= 5 {
                filters.insert(
                    "socket_filters".into(),
                    json!({ "filters": { "links": { "min": n } } }),
                );
                resume.push(format!("{n} liens"));
            }
        }
        Rarete::Rare | Rarete::Magique | Rarete::Normal => {
            query.insert("type".into(), json!(objet.base));
            resume.push(objet.base.clone());
            let mut misc = Map::new();
            if let Some(ilvl) = objet.niveau_objet {
                misc.insert("ilvl".into(), json!({ "min": ilvl }));
                resume.push(format!("niveau d'objet {ilvl}+"));
            }
            for i in &objet.influences {
                if let Some(f) = filtre_influence(i) {
                    misc.insert(f.into(), json!({ "option": "true" }));
                    resume.push(i.trim_end_matches(" Item").to_string());
                }
            }
            if !misc.is_empty() {
                filters.insert("misc_filters".into(), json!({ "filters": misc }));
            }
            filters.insert(
                "type_filters".into(),
                json!({ "filters": { "rarity": { "option": "nonunique" } } }),
            );
        }
        _ => {
            query.insert("type".into(), json!(objet.base));
            resume.push(objet.base.clone());
        }
    }
    if !filters.is_empty() {
        query.insert("filters".into(), Value::Object(filters));
    }
    (
        json!({ "query": query, "sort": { "price": "asc" } }),
        resume,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parser;

    fn corpus(nom: &str) -> Objet {
        let chemin = format!(
            "{}/../../tests/corpus/objets/poe1/{nom}",
            env!("CARGO_MANIFEST_DIR")
        );
        parser(&std::fs::read_to_string(chemin).unwrap()).unwrap()
    }

    #[test]
    fn unique_avec_ses_liens() {
        let (q, resume) = requete(&corpus("unique-armure-6l.txt"));
        assert_eq!(q["query"]["name"], "The Covenant");
        assert_eq!(q["query"]["type"], "Spidersilk Robe");
        assert_eq!(
            q["query"]["filters"]["socket_filters"]["filters"]["links"]["min"],
            6
        );
        assert_eq!(resume, vec!["The Covenant", "6 liens"]);
        assert_eq!(q["sort"]["price"], "asc");
    }

    #[test]
    fn rare_par_base_influence_et_niveau() {
        let (q, resume) = requete(&corpus("rare-casque-influence-avance.txt"));
        assert_eq!(q["query"]["type"], "Hubris Circlet");
        assert!(q["query"].get("name").is_none());
        let misc = &q["query"]["filters"]["misc_filters"]["filters"];
        assert_eq!(misc["ilvl"]["min"], 86);
        assert_eq!(misc["shaper_item"]["option"], "true");
        assert_eq!(
            resume,
            vec!["Hubris Circlet", "niveau d'objet 86+", "Shaper"]
        );
    }

    #[test]
    fn devise_par_type() {
        let (q, _) = requete(&corpus("devise-divine.txt"));
        assert_eq!(q["query"]["type"], "Divine Orb");
        assert!(q["query"].get("filters").is_none());
    }
}
