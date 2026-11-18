//! Prix estimé d'un objet de PoE 2 copié en jeu, d'après les pages « échange »
//! de poe.ninja (`/poe2/api/economy/exchange/current/overview?type=…`).
//!
//! poe.ninja PoE 2 ne cote que ce qui s'échange en pile : monnaies, essences,
//! Omens, runes, Soul Cores, catalyseurs, fragments, gemmes brutes… (vérifié le
//! 18/11/2026 : les pages d'uniques n'existent pas pour PoE 2, et les rares ne
//! sont cotés nulle part). Chaque page donne le nom de chaque identifiant
//! (`items`) et sa valeur dans la monnaie de référence (`lines[].primaryValue`,
//! aujourd'hui en Divine) ; tout est converti en Exalted Orbs.
//!
//! Ce module ne fait aucune entrée-sortie : l'appli télécharge les pages et les
//! lui passe. Les prix sont approximatifs : la cote dit « environ », jamais plus.

use serde::Serialize;
use serde_json::Value;

use crate::cible::ObjetCible;
use crate::forge::achat::{self, RechercheAchat};
use crate::forge::ninja::facteur_exalted;
use crate::parser::GenreMod;
use crate::parser::{Objet, Rarete};
use std::collections::BTreeMap;

/// Catégories « échange » non vides de poe.ninja PoE 2 (18/11/2026).
pub const ECHANGE: [&str; 14] = [
    "Currency",
    "Fragments",
    "Abyss",
    "UncutGems",
    "LineageSupportGems",
    "Essences",
    "SoulCores",
    "Idols",
    "Runes",
    "Ritual",
    "Expedition",
    "Delirium",
    "Breach",
    "Verisium",
];

/// Catégorie probable d'après le nom de l'objet (le type de base).
fn categorie_probable(objet: &Objet) -> Option<&'static str> {
    let n = objet.base.to_ascii_lowercase();
    let a = |mots: &[&str]| mots.iter().any(|m| n.contains(m));
    Some(if n.starts_with("omen of") {
        "Ritual"
    } else if a(&["essence"]) {
        "Essences"
    } else if a(&["soul core"]) {
        "SoulCores"
    } else if a(&["uncut "]) {
        "UncutGems"
    } else if a(&["catalyst", "breach splinter", "breachstone"]) {
        "Breach"
    } else if a(&["alloy", "verisium", "starlit ore"]) {
        "Verisium"
    } else if a(&[" liquid ", "liquid "]) && !a(&["flask"]) {
        "Delirium"
    } else if a(&["flux"]) {
        "Expedition"
    } else if a(&["idol"]) || n.starts_with("carved ") {
        "Idols"
    } else if a(&["collarbone", "jawbone", " rib", "cranium", "'s gaze"]) {
        "Abyss"
    } else if a(&["rune"]) {
        "Runes"
    } else if a(&[
        "fragment",
        "splinter",
        "reliquary key",
        "invitation",
        " fate",
    ]) {
        "Fragments"
    } else if objet.rarete == Rarete::Gemme {
        "LineageSupportGems"
    } else {
        return None;
    })
}

/// Objet qui s'échange en pile (ou une gemme) : toutes les pages sont essayées.
fn empilable(objet: &Objet) -> bool {
    objet.taille_pile.is_some() || matches!(objet.rarete, Rarete::Devise | Rarete::Gemme)
}

/// Classe d'équipement (armes, armures, bijoux, flasques) : jamais cotée par poe.ninja PoE 2.
fn equipement(objet: &Objet) -> bool {
    let c = objet.classe.as_deref().unwrap_or("").to_ascii_lowercase();
    [
        "shield",
        "buckler",
        "focus",
        "foci",
        "quiver",
        "body armour",
        "helmet",
        "glove",
        "boot",
        "ring",
        "amulet",
        "belt",
        "flask",
        "charm",
        "jewel",
        "bow",
        "sword",
        "axe",
        "mace",
        "sceptre",
        "staff",
        "staves",
        "wand",
        "spear",
        "flail",
        "dagger",
        "claw",
        "talisman",
    ]
    .iter()
    .any(|m| c.contains(m))
}

/// Pages à consulter, dans l'ordre, pour coter cet objet. Vide : poe.ninja ne
/// cote pas ce genre d'objet (équipement rare, magique, unique…).
pub fn pages(objet: &Objet) -> Vec<&'static str> {
    if matches!(objet.rarete, Rarete::Rare | Rarete::Magique)
        || (!empilable(objet) && equipement(objet))
    {
        return Vec::new();
    }
    let probable = categorie_probable(objet);
    let mut v: Vec<&'static str> = probable.into_iter().collect();
    if empilable(objet) {
        for t in ["Currency"].into_iter().chain(ECHANGE) {
            if !v.contains(&t) {
                v.push(t);
            }
        }
    }
    v
}

/// Niveau inscrit sur l'objet (« Level: 19 (Max) » → 19), pour les gemmes brutes.
fn niveau(objet: &Objet) -> Option<u32> {
    objet
        .proprietes
        .iter()
        .find(|p| p.nom == "Level")
        .and_then(|p| {
            let chiffres: String = p
                .valeur
                .trim()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            chiffres.parse().ok()
        })
}

/// Noms sous lesquels poe.ninja peut connaître l'objet.
fn noms(objet: &Objet) -> Vec<String> {
    let mut v = Vec::new();
    if objet.rarete == Rarete::Unique
        && let Some(nom) = &objet.nom
    {
        v.push(nom.clone());
    }
    if let Some(n) = niveau(objet) {
        v.push(format!("{} (Level {n})", objet.base));
    }
    v.push(objet.base.clone());
    v
}

/// Valeur unitaire en Exalted d'un nom dans une page d'échange.
fn valeur(page: &Value, nom: &str, facteur: f64) -> Option<f64> {
    let id = page["items"].as_array()?.iter().find(|i| {
        i["name"]
            .as_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(nom))
    })?["id"]
        .as_str()?;
    let v = page["lines"]
        .as_array()?
        .iter()
        .find(|l| l["id"].as_str() == Some(id))?["primaryValue"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0)?;
    Some(v * facteur)
}

/// Cote d'un objet empilable, en Exalted Orbs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cote {
    /// Valeur d'une unité.
    pub unite: f64,
    /// Taille de la pile copiée (1 hors pile).
    pub quantite: u32,
    /// Valeur de la pile entière.
    pub total: f64,
}

/// Cote de l'objet dans une page d'échange (`None` : la page ne le connaît pas).
pub fn coter(page: &Value, objet: &Objet) -> Option<Cote> {
    let facteur = facteur_exalted(page).ok()?;
    let unite = noms(objet).iter().find_map(|n| valeur(page, n, facteur))?;
    let quantite = objet.taille_pile.map_or(1, |(n, _)| n.max(1));
    Some(Cote {
        unite,
        quantite,
        total: unite * f64::from(quantite),
    })
}

/// Cours de la Divine Orb en Exalted, tiré de la page `Currency`.
pub fn exalts_par_divine(page_devises: &Value) -> Option<f64> {
    let facteur = facteur_exalted(page_devises).ok()?;
    let v = page_devises["lines"]
        .as_array()?
        .iter()
        .find(|l| l["id"].as_str() == Some("divine"))?["primaryValue"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.0)?;
    Some(v * facteur)
}

/// Montant arrondi pour l'affichage : une décimale sous 10, entier au-delà.
pub fn arrondi(v: f64) -> f64 {
    if v >= 10.0 {
        v.round()
    } else if v >= 0.1 {
        (v * 10.0).round() / 10.0
    } else {
        (v * 1000.0).round() / 1000.0
    }
}

// ── Cote du site de trade (uniques et rares) ────────────────────────────────
//
// Décision de Max du 18/11/2026 : comme Awakened PoE Trade et Exiled Exchange 2, l'appli lance la
// recherche depuis le PC du joueur, sur son raccourci, en suivant les limites que GGG renvoie.
// Ce module prépare la recherche et lit les annonces ; l'appli fait les requêtes.

/// Annonces lues au plus par cote (une seule requête `fetch`).
pub const ANNONCES_MAX: usize = 10;

/// Recherche de trade d'objets comparables : l'unique par son nom, le rare par sa base et ses mods
/// explicites à au moins 80 % de leur valeur. `None` : objet que le trade ne sait pas comparer
/// (magique, normal, unique non identifié, rare sans mod reconnu).
pub fn recherche_trade(objet: &Objet) -> Option<RechercheAchat> {
    let o = match objet.rarete {
        Rarete::Unique => ObjetCible {
            emplacement: String::new(),
            unique: Some(objet.nom.clone()?),
            base: Some(objet.base.clone()),
            lignes: Vec::new(),
        },
        Rarete::Rare if !objet.non_identifie => ObjetCible {
            emplacement: String::new(),
            unique: None,
            base: Some(objet.base.clone()),
            lignes: objet
                .mods
                .iter()
                .filter(|m| {
                    matches!(
                        m.genre,
                        GenreMod::Explicite | GenreMod::Fracture | GenreMod::Desacralise
                    )
                })
                .map(|m| m.texte.clone())
                .collect(),
        },
        _ => return None,
    };
    let r = achat::recherche_objet_cible(&o, false);
    (o.unique.is_some() || !r.filtres.is_empty()).then_some(r)
}

/// Valeur en Exalted de chaque devise de la page `Currency` de poe.ninja (identifiants communs au
/// site de trade : `exalted`, `divine`, `chaos`, `alch`…).
pub fn taux_devises(page_devises: &Value) -> BTreeMap<String, f64> {
    let mut t = BTreeMap::new();
    let Ok(facteur) = facteur_exalted(page_devises) else {
        return t;
    };
    for l in page_devises["lines"].as_array().into_iter().flatten() {
        if let (Some(id), Some(v)) = (l["id"].as_str(), l["primaryValue"].as_f64())
            && v.is_finite()
            && v > 0.0
        {
            t.insert(id.to_string(), v * facteur);
        }
    }
    t.insert("exalted".into(), 1.0);
    t
}

/// Prix en Exalted des annonces d'une réponse `fetch` du site de trade (devise inconnue : ignorée).
pub fn prix_annonces(fetch: &Value, taux: &BTreeMap<String, f64>) -> Vec<f64> {
    fetch["result"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let p = &r["listing"]["price"];
            let montant = p["amount"].as_f64().filter(|v| v.is_finite() && *v > 0.0)?;
            Some(montant * taux.get(p["currency"].as_str()?)?)
        })
        .collect()
}

/// Fourchette des annonces les moins chères : (bas, médiane, haut). Dès 5 annonces, la moins chère
/// est écartée (prix d'appel ou erreur de saisie).
pub fn fourchette(prix: &[f64]) -> Option<(f64, f64, f64)> {
    let mut p: Vec<f64> = prix.iter().copied().filter(|v| v.is_finite()).collect();
    p.sort_by(f64::total_cmp);
    if p.len() >= 5 {
        p.remove(0);
    }
    let (bas, haut) = (*p.first()?, *p.last()?);
    let n = p.len();
    let mediane = if n % 2 == 1 {
        p[n / 2]
    } else {
        (p[n / 2 - 1] + p[n / 2]) / 2.0
    };
    Some((bas, mediane, haut))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parser;

    fn donnees(nom: &str) -> Value {
        let chemin = format!("{}/tests/donnees/cote/{nom}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap()
    }

    fn corpus(nom: &str) -> Objet {
        let chemin = format!(
            "{}/../../tests/corpus/objets/{nom}",
            env!("CARGO_MANIFEST_DIR")
        );
        parser(&std::fs::read_to_string(chemin).unwrap()).unwrap()
    }

    fn devise(nom: &str, pile: Option<&str>) -> Objet {
        let pile = pile.map_or(String::new(), |p| format!("--------\nStack Size: {p}\n"));
        parser(&format!(
            "Item Class: Stackable Currency\nRarity: Currency\n{nom}\n{pile}"
        ))
        .unwrap()
    }

    #[test]
    fn choisit_les_pages_selon_l_objet() {
        assert_eq!(pages(&corpus("devise.txt"))[0], "Currency");
        assert_eq!(pages(&corpus("devise.txt")).len(), ECHANGE.len());
        let essence = devise("Greater Essence of Ruin", Some("3/10"));
        assert_eq!(&pages(&essence)[..2], ["Essences", "Currency"]);
        assert_eq!(pages(&devise("Omen of Light", None))[0], "Ritual");
        assert_eq!(pages(&devise("Uncut Skill Gem", None))[0], "UncutGems");
        assert_eq!(pages(&corpus("gemme.txt"))[0], "LineageSupportGems");
        // L'équipement n'est pas coté par poe.ninja PoE 2.
        assert!(pages(&corpus("rare-bague-avance.txt")).is_empty());
        assert!(pages(&corpus("magique-amulette.txt")).is_empty());
        assert!(pages(&corpus("unique-gants.txt")).is_empty());
        assert!(pages(&corpus("normal-bouclier.txt")).is_empty());
    }

    #[test]
    fn cote_une_pile_en_exalts() {
        let page = donnees("ninja-currency.json");
        let div = exalts_par_divine(&page).unwrap();
        assert!((div - 500.25).abs() < 0.1, "{div}");
        // L'Exalted vaut exactement 1 (ancre de la conversion).
        let ex = coter(&page, &corpus("devise.txt")).unwrap();
        assert_eq!(ex.quantite, 7);
        assert!((ex.unite - 1.0).abs() < 1e-9);
        assert!((ex.total - 7.0).abs() < 1e-9);
        let chaos = coter(&page, &devise("Chaos Orb", Some("20/20"))).unwrap();
        assert!((chaos.unite - 47.5).abs() < 0.1, "{}", chaos.unite);
        assert_eq!(chaos.quantite, 20);
        assert_eq!(coter(&page, &devise("Objet inconnu", None)), None);
    }

    #[test]
    fn cote_une_essence_et_une_gemme_brute_par_niveau() {
        let essences = donnees("ninja-essences.json");
        let e = coter(&essences, &devise("Essence of Delirium", None)).unwrap();
        assert!((e.unite - 2.54 * 500.25).abs() < 1.0, "{}", e.unite);
        assert_eq!(e.quantite, 1);
        let gemme = parser(
            "Item Class: Uncut Skill Gems\nRarity: Currency\nUncut Skill Gem\n--------\nLevel: 19\n--------\nCreates a Skill Gem.",
        )
        .unwrap();
        let g = coter(&donnees("ninja-uncutgems.json"), &gemme).unwrap();
        assert!((g.unite - 0.08158 * 500.25).abs() < 0.1, "{}", g.unite);
    }

    #[test]
    fn prepare_la_recherche_trade() {
        let u = recherche_trade(&corpus("unique-gants.txt")).unwrap();
        assert_eq!(
            u.requete["query"]["filters"]["type_filters"]["filters"]["rarity"]["option"],
            "unique"
        );
        assert!(u.requete["query"]["name"].is_string());
        let r = recherche_trade(&corpus("rare-bague-avance.txt")).unwrap();
        assert!(!r.filtres.is_empty());
        assert_eq!(r.requete["sort"]["price"], "asc");
        assert!(recherche_trade(&corpus("magique-amulette.txt")).is_none());
        assert!(recherche_trade(&corpus("rare-non-identifie.txt")).is_none());
        assert!(recherche_trade(&corpus("devise.txt")).is_none());
    }

    #[test]
    fn lit_les_annonces_et_leur_fourchette() {
        let taux = taux_devises(&donnees("ninja-currency.json"));
        assert_eq!(taux["exalted"], 1.0);
        let fetch: Value = serde_json::json!({ "result": [
            { "listing": { "price": { "amount": 1, "currency": "divine" } } },
            { "listing": { "price": { "amount": 20, "currency": "exalted" } } },
            { "listing": { "price": { "amount": 3, "currency": "inconnue" } } },
            { "listing": null },
        ]});
        let p = prix_annonces(&fetch, &taux);
        assert_eq!(p.len(), 2);
        assert!((p[0] - 500.25).abs() < 0.1);
        assert_eq!(fourchette(&[]), None);
        assert_eq!(fourchette(&[3.0, 1.0]), Some((1.0, 2.0, 3.0)));
        // Dès 5 annonces, la moins chère est écartée.
        assert_eq!(
            fourchette(&[1.0, 10.0, 11.0, 12.0, 40.0]),
            Some((10.0, 11.5, 40.0))
        );
    }

    #[test]
    fn arrondit_pour_l_affichage() {
        assert_eq!(arrondi(1234.4), 1234.0);
        assert_eq!(arrondi(4.26), 4.3);
        assert_eq!(arrondi(0.01234), 0.012);
    }
}
