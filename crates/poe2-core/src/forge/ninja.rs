//! Prix du marché depuis poe.ninja (API économique publique de PoE 2), repris de
//! poe2-craft-assistant. Décision de Max du 07/11/2026 : poe.ninja **en plus** des prix saisis.
//!
//! Ce module ne fait aucune entrée-sortie : l'appli télécharge les réponses
//! (`{base}/leagues`, puis `{base}/exchange/current/overview?league=…&type=…` par catégorie)
//! et les lui passe. Règles d'usage : identifiant d'appli dans le User-Agent, une requête par
//! catégorie ([`CATEGORIES`]) plus la liste des ligues, et jamais plus d'une actualisation toutes
//! les [`DELAI_MIN_SECS`] secondes ([`actualisation_permise`]).
//!
//! Format (vérifié le 07/11/2026 sur une réponse réelle) : `{ core: { primary, rates }, lines:
//! [{ id, primaryValue }] }`. `primaryValue` est dans la monnaie `core.primary` (aujourd'hui
//! « divine ») ; tout est converti en Exalted Orbs.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Adresse de l'API économique PoE 2 de poe.ninja.
pub const API: &str = "https://poe.ninja/poe2/api/economy";

/// Délai minimal entre deux actualisations (poe.ninja ne met à jour PoE 2 qu'environ toutes les
/// heures).
pub const DELAI_MIN_SECS: u64 = 300;
/// Après un échec réseau, nouvel essai au plus tôt 10 minutes plus tard.
pub const DELAI_APRES_ERREUR_SECS: u64 = 600;

/// Catégories poe.ninja interrogées : monnaies et Omens (`Ritual`).
pub const CATEGORIES: [&str; 2] = ["Currency", "Ritual"];

/// Identifiant Forge -> (catégorie poe.ninja, identifiant poe.ninja). La base neuve et sa revente
/// n'ont pas de cote : elles restent saisies par le joueur.
pub const SOURCES: [(&str, &str, &str); 24] = [
    ("transmute", "Currency", "transmute"),
    (
        "transmute_greater",
        "Currency",
        "greater-orb-of-transmutation",
    ),
    (
        "transmute_perfect",
        "Currency",
        "perfect-orb-of-transmutation",
    ),
    ("augment", "Currency", "aug"),
    ("augment_greater", "Currency", "greater-orb-of-augmentation"),
    ("augment_perfect", "Currency", "perfect-orb-of-augmentation"),
    ("regal", "Currency", "regal"),
    ("regal_greater", "Currency", "greater-regal-orb"),
    ("regal_perfect", "Currency", "perfect-regal-orb"),
    ("alchemy", "Currency", "alch"),
    ("exalt", "Currency", "exalted"),
    ("exalt_greater", "Currency", "greater-exalted-orb"),
    ("exalt_perfect", "Currency", "perfect-exalted-orb"),
    ("chaos", "Currency", "chaos"),
    ("chaos_greater", "Currency", "greater-chaos-orb"),
    ("chaos_perfect", "Currency", "perfect-chaos-orb"),
    ("annul", "Currency", "annul"),
    ("fracture", "Currency", "fracturing-orb"),
    (
        "omen_sinistral_exaltation",
        "Ritual",
        "omen-of-sinistral-exaltation",
    ),
    (
        "omen_dextral_exaltation",
        "Ritual",
        "omen-of-dextral-exaltation",
    ),
    (
        "omen_sinistral_annulment",
        "Ritual",
        "omen-of-sinistral-annulment",
    ),
    (
        "omen_dextral_annulment",
        "Ritual",
        "omen-of-dextral-annulment",
    ),
    (
        "omen_sinistral_erasure",
        "Ritual",
        "omen-of-sinistral-erasure",
    ),
    ("omen_dextral_erasure", "Ritual", "omen-of-dextral-erasure"),
];

/// Relevé de prix poe.ninja, en Exalted Orbs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleveMarche {
    pub ligue: String,
    /// Date (unix, secondes) du dernier relevé réussi.
    pub releve_a: u64,
    /// identifiant Forge -> prix en Exalted
    pub prix: BTreeMap<String, f64>,
    /// identifiant Forge -> date (unix) de la cote utilisée : un prix absent d'un relevé garde sa
    /// valeur et sa date précédentes (même ligue).
    pub dates: BTreeMap<String, u64>,
    /// Identifiants sans cote exploitable dans ce relevé (et jamais cotés avant).
    pub manquants: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErreurNinja {
    #[error("réponse poe.ninja inattendue : {0}")]
    Format(String),
    #[error("réponse vide : la ligue « {0} » n'a pas encore de prix")]
    Vide(String),
    #[error("aucun taux de change vers l'Exalted dans la réponse (référence « {0} »)")]
    SansTaux(String),
}

/// Facteur « monnaie de référence de la réponse -> Exalted » : la ligne `exalted` sert d'ancre si
/// elle est là (l'Exalted vaut alors exactement 1), sinon `core.rates.exalted`.
fn facteur_exalted(doc: &Value) -> Result<f64, ErreurNinja> {
    let primary = doc["core"]["primary"]
        .as_str()
        .ok_or_else(|| ErreurNinja::Format("core.primary absent".into()))?;
    if primary == "exalted" {
        return Ok(1.0);
    }
    let ancre = doc["lines"]
        .as_array()
        .and_then(|l| l.iter().find(|l| l["id"].as_str() == Some("exalted")))
        .and_then(|l| l["primaryValue"].as_f64())
        .filter(|v| v.is_finite() && *v > 0.0);
    if let Some(v) = ancre {
        return Ok(1.0 / v);
    }
    doc["core"]["rates"]["exalted"]
        .as_f64()
        .filter(|r| r.is_finite() && *r > 0.0)
        .ok_or_else(|| ErreurNinja::SansTaux(primary.to_string()))
}

/// Prix en Exalted des identifiants Forge de `categorie` trouvés dans une réponse, et ceux qui y
/// manquent (absents, ou cote nulle) : jamais de prix inventé.
pub fn extraire(
    doc: &Value,
    categorie: &str,
    ligue: &str,
) -> Result<(BTreeMap<String, f64>, Vec<String>), ErreurNinja> {
    let lignes = doc["lines"]
        .as_array()
        .ok_or_else(|| ErreurNinja::Format("lines absent".into()))?;
    if lignes.is_empty() {
        return Err(ErreurNinja::Vide(ligue.to_string()));
    }
    let facteur = facteur_exalted(doc)?;
    let (mut prix, mut manquants) = (BTreeMap::new(), Vec::new());
    for (id, _, ninja_id) in SOURCES.iter().filter(|(_, c, _)| *c == categorie) {
        let v = lignes
            .iter()
            .find(|l| l["id"].as_str() == Some(ninja_id))
            .and_then(|l| l["primaryValue"].as_f64())
            .filter(|v| v.is_finite() && *v > 0.0);
        match v {
            Some(v) => {
                prix.insert((*id).to_string(), v * facteur);
            }
            None => manquants.push((*id).to_string()),
        }
    }
    Ok((prix, manquants))
}

/// Première ligue de la liste poe.ninja (la ligue temporaire courante).
pub fn premiere_ligue(doc: &Value) -> Result<String, ErreurNinja> {
    doc.as_array()
        .and_then(|a| a.first())
        .and_then(|l| l["id"].as_str())
        .map(str::to_string)
        .ok_or_else(|| ErreurNinja::Format("liste de ligues vide".into()))
}

/// Construit un relevé à partir des réponses de chaque catégorie, puis le fusionne avec le
/// précédent : même ligue, un prix absent cette fois garde sa dernière valeur et sa date ; une
/// autre ligue repart de zéro.
pub fn releve(
    ligue: &str,
    maintenant: u64,
    reponses: &[(&str, Value)],
    precedent: Option<&ReleveMarche>,
) -> Result<ReleveMarche, ErreurNinja> {
    let mut r = ReleveMarche {
        ligue: ligue.to_string(),
        releve_a: maintenant,
        ..Default::default()
    };
    for (categorie, doc) in reponses {
        let (prix, manquants) = extraire(doc, categorie, ligue)?;
        r.dates.extend(prix.keys().map(|k| (k.clone(), maintenant)));
        r.prix.extend(prix);
        r.manquants.extend(manquants);
    }
    if let Some(p) = precedent.filter(|p| p.ligue == ligue) {
        for (k, v) in &p.prix {
            if !r.prix.contains_key(k) {
                r.prix.insert(k.clone(), *v);
                r.dates
                    .insert(k.clone(), p.dates.get(k).copied().unwrap_or(p.releve_a));
            }
        }
    }
    r.manquants.retain(|k| !r.prix.contains_key(k));
    r.manquants.sort();
    Ok(r)
}

/// Une actualisation est-elle permise maintenant ? `Err(secondes)` : temps à attendre.
pub fn actualisation_permise(
    maintenant: u64,
    dernier_releve: Option<u64>,
    derniere_erreur: Option<u64>,
) -> Result<(), u64> {
    let attente = |depuis: u64, delai: u64| delai.saturating_sub(maintenant.saturating_sub(depuis));
    let a = dernier_releve.map_or(0, |t| attente(t, DELAI_MIN_SECS));
    let b = derniere_erreur.map_or(0, |t| attente(t, DELAI_APRES_ERREUR_SECS));
    match a.max(b) {
        0 => Ok(()),
        s => Err(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(nom: &str) -> Value {
        let chemin = format!("{}/tests/donnees/forge/{nom}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap()
    }

    /// Réponses réelles du 07/11/2026, ligue Forbidden Rites (1 Divine = 635 Exalted).
    #[test]
    fn reponses_reelles_converties_en_exalted() {
        let monnaies = fixture("ninja-currency.json");
        assert_eq!(monnaies["core"]["primary"], "divine");
        let (prix, manquants) = extraire(&monnaies, "Currency", "Forbidden Rites").unwrap();
        assert!(manquants.is_empty(), "{manquants:?}");
        assert_eq!(prix.len(), 18);
        assert!(
            (prix["exalt"] - 1.0).abs() < 1e-9,
            "l'Exalted vaut 1 : {}",
            prix["exalt"]
        );
        assert!((prix["chaos"] - 0.1046 / 0.001575).abs() < 1e-6);

        // Pas de ligne `exalted` chez les Omens : taux de `core.rates`
        let omens = fixture("ninja-ritual.json");
        let (prix, manquants) = extraire(&omens, "Ritual", "Forbidden Rites").unwrap();
        assert!(manquants.is_empty());
        assert_eq!(prix.len(), 6);
        let brut = omens["lines"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["id"] == "omen-of-dextral-annulment")
            .unwrap()["primaryValue"]
            .as_f64()
            .unwrap();
        assert!((prix["omen_dextral_annulment"] - brut * 635.0).abs() < 1e-6);
    }

    #[test]
    fn releve_complet_et_fusion_avec_le_precedent() {
        let reponses = [
            ("Currency", fixture("ninja-currency.json")),
            ("Ritual", fixture("ninja-ritual.json")),
        ];
        let r = releve("Forbidden Rites", 1000, &reponses, None).unwrap();
        assert_eq!(r.prix.len(), SOURCES.len());
        assert!(r.manquants.is_empty());

        // une heure plus tard, le Chaos n'est plus coté : il garde sa valeur et sa date
        let sans_chaos = json!({"core": {"primary": "exalted"}, "lines": [{"id": "exalted", "primaryValue": 1.0}]});
        let r2 = releve(
            "Forbidden Rites",
            4600,
            &[("Currency", sans_chaos.clone())],
            Some(&r),
        )
        .unwrap();
        assert_eq!(
            (r2.prix["chaos"], r2.dates["chaos"]),
            (r.prix["chaos"], 1000)
        );
        assert_eq!(r2.dates["exalt"], 4600);
        assert!(r2.manquants.is_empty());

        // autre ligue : on repart de zéro
        let r3 = releve("Standard", 4600, &[("Currency", sans_chaos)], Some(&r)).unwrap();
        assert!(!r3.prix.contains_key("chaos"));
        assert!(r3.manquants.contains(&"chaos".to_string()));
    }

    #[test]
    fn reponses_vides_ou_abimees() {
        let vide = json!({"core": {"primary": "divine"}, "lines": []});
        assert_eq!(
            extraire(&vide, "Currency", "L").unwrap_err(),
            ErreurNinja::Vide("L".into())
        );
        assert!(matches!(
            extraire(&json!({"lines": [{"id": "a"}]}), "Currency", "L"),
            Err(ErreurNinja::Format(_))
        ));
        let sans_taux = json!({"core": {"primary": "divine", "rates": {"chaos": 8.0}}, "lines": [{"id": "chaos", "primaryValue": 0.1}]});
        assert_eq!(
            extraire(&sans_taux, "Currency", "L").unwrap_err(),
            ErreurNinja::SansTaux("divine".into())
        );
        // cote nulle : manquant, pas zéro
        let nul = json!({"core": {"primary": "exalted"}, "lines": [{"id": "chaos", "primaryValue": 0.0}]});
        let (prix, manquants) = extraire(&nul, "Currency", "L").unwrap();
        assert!(!prix.contains_key("chaos") && manquants.contains(&"chaos".to_string()));
    }

    #[test]
    fn ligues_et_cadence() {
        assert_eq!(
            premiere_ligue(&json!([{"id": "Forbidden Rites"}, {"id": "Standard"}])).unwrap(),
            "Forbidden Rites"
        );
        assert!(premiere_ligue(&json!([])).is_err());

        assert_eq!(actualisation_permise(1000, None, None), Ok(()));
        assert_eq!(actualisation_permise(1000, Some(900), None), Err(200));
        assert_eq!(actualisation_permise(1300, Some(1000), None), Ok(()));
        assert_eq!(actualisation_permise(1300, Some(100), Some(1000)), Err(300));
    }

    #[test]
    fn chaque_monnaie_et_omen_du_catalogue_a_une_source() {
        use crate::forge::catalogue::{MONNAIES, OMENS};
        for id in MONNAIES
            .iter()
            .map(|m| m.id)
            .chain(OMENS.iter().map(|o| o.id))
        {
            assert!(
                SOURCES.iter().any(|(s, _, _)| *s == id),
                "pas de source pour {id}"
            );
        }
        assert!(SOURCES.iter().all(|(_, c, _)| CATEGORIES.contains(c)));
    }
}
