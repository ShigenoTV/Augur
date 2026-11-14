//! Cote poe.ninja d'un objet de PoE 1.
//!
//! Deux familles de pages poe.ninja (`/poe1/api/economy/…`, vérifiées le
//! 14/11/2026) :
//! - **échange** (`exchange/current/overview?type=Currency`) : devises, cartes,
//!   essences, fossiles… `items` donne le nom de chaque identifiant, `lines` sa
//!   valeur en Chaos (`primaryValue`) ;
//! - **objets** (`stash/current/item/overview?type=UniqueArmour`) : uniques,
//!   avec la base, les liens (5 ou 6, absent sinon) et `chaosValue`.
//!
//! Les prix sont approximatifs : la cote dit « environ », jamais plus.

use serde::Serialize;
use serde_json::Value;

use super::liens;
use crate::parser::{Objet, Rarete};

/// Adresse de l'API économique PoE 1 de poe.ninja.
pub const API: &str = "https://poe.ninja/poe1/api/economy";

/// Catégories « échange » non vides de poe.ninja PoE 1, essayées dans cet ordre
/// pour une devise (la première qui connaît l'objet donne sa cote).
pub const ECHANGE_DEVISES: [&str; 13] = [
    "Currency",
    "Fragment",
    "Scarab",
    "Essence",
    "Fossil",
    "Resonator",
    "Oil",
    "DeliriumOrb",
    "Omen",
    "Tattoo",
    "Artifact",
    "Runegraft",
    "AllflameEmber",
];

/// Catégories d'uniques, essayées dans cet ordre quand la classe ne dit rien.
pub const UNIQUES: [&str; 8] = [
    "UniqueWeapon",
    "UniqueArmour",
    "UniqueAccessory",
    "UniqueFlask",
    "UniqueJewel",
    "UniqueMap",
    "UniqueTincture",
    "UniqueRelic",
];

/// Page poe.ninja à consulter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Echange(&'static str),
    Objets(&'static str),
}

impl Page {
    /// Chemin relatif à [`API`] (sans la ligue).
    pub fn chemin(self) -> String {
        match self {
            Page::Echange(t) => format!("exchange/current/overview?type={t}"),
            Page::Objets(t) => format!("stash/current/item/overview?type={t}"),
        }
    }
}

/// Catégorie d'uniques probable d'après la classe de l'objet (`Item Class`).
fn categorie_unique(classe: &str) -> Option<&'static str> {
    let c = classe.to_ascii_lowercase();
    let armure = ["body armour", "helmet", "glove", "boot", "shield", "quiver"];
    let accessoire = ["ring", "amulet", "belt"];
    Some(if armure.iter().any(|a| c.contains(a)) {
        "UniqueArmour"
    } else if accessoire.iter().any(|a| c.contains(a)) {
        "UniqueAccessory"
    } else if c.contains("flask") {
        "UniqueFlask"
    } else if c.contains("jewel") {
        "UniqueJewel"
    } else if c.contains("tincture") {
        "UniqueTincture"
    } else if c.contains("relic") {
        "UniqueRelic"
    } else if c.contains("map") {
        "UniqueMap"
    } else if [
        "sword",
        "axe",
        "mace",
        "sceptre",
        "bow",
        "claw",
        "dagger",
        "wand",
        "staff",
        "staves",
        "warstaff",
        "rune dagger",
        "fishing",
    ]
    .iter()
    .any(|a| c.contains(a))
    {
        "UniqueWeapon"
    } else {
        return None;
    })
}

/// Pages à consulter, dans l'ordre, pour coter cet objet. Vide : poe.ninja ne
/// cote pas ce genre d'objet (rares, magiques, normaux, gemmes).
pub fn pages(objet: &Objet) -> Vec<Page> {
    match objet.rarete {
        Rarete::CarteDivination => vec![Page::Echange("DivinationCard")],
        Rarete::Devise => ECHANGE_DEVISES.iter().map(|t| Page::Echange(t)).collect(),
        Rarete::Unique => {
            let probable = objet.classe.as_deref().and_then(categorie_unique);
            let mut v: Vec<Page> = probable.into_iter().map(Page::Objets).collect();
            v.extend(
                UNIQUES
                    .iter()
                    .filter(|t| Some(**t) != probable)
                    .map(|t| Page::Objets(t)),
            );
            v
        }
        _ => Vec::new(),
    }
}

/// Cote d'un objet.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cote {
    /// Valeur en Chaos Orbs (la plus basse des variantes s'il y en a plusieurs).
    pub chaos: f64,
    /// Valeur la plus haute des variantes, si elle diffère.
    pub chaos_max: Option<f64>,
    /// Valeur en Divine Orbs, si le cours de la Divine est connu.
    pub divine: Option<f64>,
    /// Nombre d'annonces relevées par poe.ninja (uniques seulement).
    pub annonces: Option<u32>,
    /// Liens retenus pour la cote (5 ou 6), si l'objet en a autant.
    pub liens: Option<u32>,
}

/// Cours de la Divine Orb en Chaos, tiré d'une page d'échange (`core.rates.divine`
/// donne des Divines par Chaos).
pub fn cours_divine(page_echange: &Value) -> Option<f64> {
    let taux = page_echange["core"]["rates"]["divine"].as_f64()?;
    (taux > 0.0).then(|| 1.0 / taux)
}

fn arrondi(v: f64) -> f64 {
    if v >= 10.0 {
        v.round()
    } else {
        (v * 10.0).round() / 10.0
    }
}

fn cote(chaos: f64, chaos_max: Option<f64>, divine: Option<f64>) -> Cote {
    Cote {
        chaos: arrondi(chaos),
        chaos_max: chaos_max
            .filter(|m| arrondi(*m) > arrondi(chaos))
            .map(arrondi),
        divine: divine.map(|d| (chaos / d * 100.0).round() / 100.0),
        annonces: None,
        liens: None,
    }
}

/// Cote d'une devise ou d'une carte dans une page d'échange, par son nom.
pub fn cote_echange(page: &Value, nom: &str, cours_divine: Option<f64>) -> Option<Cote> {
    let id = page["items"].as_array()?.iter().find(|i| {
        i["name"]
            .as_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(nom))
    })?["id"]
        .as_str()?;
    let chaos = page["lines"]
        .as_array()?
        .iter()
        .find(|l| l["id"].as_str() == Some(id))?["primaryValue"]
        .as_f64()?;
    Some(cote(chaos, None, cours_divine))
}

/// Cote d'un unique dans une page d'objets : même nom (et même base), avec les
/// mêmes liens si l'objet en a 5 ou 6, sans liens sinon.
pub fn cote_unique(page: &Value, objet: &Objet, cours_divine: Option<f64>) -> Option<Cote> {
    let nom = objet.nom.as_deref().unwrap_or(&objet.base);
    let lignes: Vec<&Value> = page["lines"]
        .as_array()?
        .iter()
        .filter(|l| l["name"].as_str() == Some(nom))
        .filter(|l| objet.nom.is_none() || l["baseType"].as_str().is_none_or(|b| b == objet.base))
        .collect();
    if lignes.is_empty() {
        return None;
    }
    let n = liens(objet);
    let voulus = if n >= 5 { Some(n) } else { None };
    let liens_de = |l: &Value| l["links"].as_u64().map(|v| v as u32);
    let mut retenues: Vec<&Value> = lignes
        .iter()
        .copied()
        .filter(|l| liens_de(l) == voulus)
        .collect();
    if retenues.is_empty() {
        // Pas de cote pour ces liens : celle sans liens, faute de mieux.
        retenues = lignes
            .iter()
            .copied()
            .filter(|l| liens_de(l).is_none())
            .collect();
    }
    let valeurs: Vec<f64> = retenues
        .iter()
        .filter_map(|l| l["chaosValue"].as_f64())
        .collect();
    let min = valeurs.iter().copied().fold(f64::INFINITY, f64::min);
    if !min.is_finite() {
        return None;
    }
    let max = valeurs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut c = cote(min, Some(max), cours_divine);
    c.annonces = Some(
        retenues
            .iter()
            .filter_map(|l| l["listingCount"].as_u64())
            .sum::<u64>() as u32,
    );
    c.liens = retenues.first().and_then(|l| liens_de(l));
    Some(c)
}

/// Cote de l'objet dans une page, selon la famille de la page.
pub fn coter(page: Page, doc: &Value, objet: &Objet, cours_divine: Option<f64>) -> Option<Cote> {
    match page {
        Page::Echange(_) => cote_echange(doc, &objet.base, cours_divine),
        Page::Objets(_) => cote_unique(doc, objet, cours_divine),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parser;

    fn donnees(nom: &str) -> Value {
        let chemin = format!("{}/tests/donnees/poe1/{nom}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap()
    }

    fn corpus(nom: &str) -> Objet {
        let chemin = format!(
            "{}/../../tests/corpus/objets/poe1/{nom}",
            env!("CARGO_MANIFEST_DIR")
        );
        parser(&std::fs::read_to_string(chemin).unwrap()).unwrap()
    }

    #[test]
    fn choisit_les_pages_selon_l_objet() {
        assert_eq!(
            pages(&corpus("carte-divination.txt")),
            vec![Page::Echange("DivinationCard")]
        );
        assert_eq!(
            pages(&corpus("devise-divine.txt"))[0],
            Page::Echange("Currency")
        );
        let unique = pages(&corpus("unique-armure-6l.txt"));
        assert_eq!(unique[0], Page::Objets("UniqueArmour"));
        assert_eq!(unique.len(), UNIQUES.len());
        assert!(pages(&corpus("rare-casque-influence-avance.txt")).is_empty());
        assert_eq!(
            Page::Echange("Currency").chemin(),
            "exchange/current/overview?type=Currency"
        );
    }

    #[test]
    fn cote_une_devise_et_une_carte() {
        let devises = donnees("ninja-currency.json");
        let divine = cours_divine(&devises).unwrap();
        assert!((divine - 78.3).abs() < 0.1, "{divine}");
        let c = cote_echange(&devises, "Divine Orb", Some(divine)).unwrap();
        assert_eq!(c.chaos, 78.0);
        assert_eq!(c.divine, Some(1.0));
        let carte = coter(
            Page::Echange("DivinationCard"),
            &donnees("ninja-cards.json"),
            &corpus("carte-divination.txt"),
            Some(divine),
        )
        .unwrap();
        assert_eq!(carte.chaos, 7.2);
        assert_eq!(cote_echange(&devises, "Objet inconnu", None), None);
    }

    #[test]
    fn cote_un_unique_selon_ses_liens() {
        let page = donnees("ninja-unique-armour.json");
        let six = cote_unique(&page, &corpus("unique-armure-6l.txt"), Some(78.29)).unwrap();
        assert_eq!((six.chaos, six.liens), (7829.0, Some(6)));
        assert_eq!(six.divine, Some(100.0));
        let mut sans = corpus("unique-armure-6l.txt");
        sans.emplacements = Some("B-B B".into());
        let c = cote_unique(&page, &sans, None).unwrap();
        assert_eq!((c.chaos, c.liens, c.annonces), (157.0, None, Some(46)));
        // Cinq liens sans cote propre : on retombe sur la cote sans liens.
        sans.emplacements = Some("B-B-B-B-B B".into());
        assert_eq!(cote_unique(&page, &sans, None).unwrap().chaos, 157.0);
    }
}
