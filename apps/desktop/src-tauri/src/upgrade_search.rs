//! Recherche d'upgrades (Alt+U dans l'overlay) : recherche de trade pondérée
//! ouverte dans le navigateur du joueur.
//!
//! Les poids viennent de `poe2_core::comparaison::recherche` (gain de DPS et
//! d'EHP par point de chaque stat, pour l'emplacement de l'objet copié).
//! Augur n'envoie aucune requête au site de trade : il ouvre l'adresse de la
//! recherche (`/trade2/search/poe2/<ligue>?q=<requête>`) et c'est le
//! navigateur du joueur qui la lance, une fois, sur son action.

use poe2_core::comparaison::recherche::{self, Recherche};
use poe2_core::pob::BuildPob;
use serde_json::{Value, json};

use crate::contracts::{AppError, ErrorCode, app_error};

const SITE: &str = "https://www.pathofexile.com/trade2/search/poe2/";

/// Les défenses locales demandées valent cette part de celles de l'objet
/// équipé : une pièce un peu moins défensive mais meilleure ailleurs passe.
const PART_DEFENSES: f64 = 0.8;

/// Ce que la recherche a ouvert, pour l'overlay.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeSearch {
    pub url: String,
    /// Stats pondérées, de la plus forte à la plus faible (libellés du jeu).
    pub stats: Vec<String>,
    /// Le DPS a compté (sinon, la défense seule).
    pub with_dps: bool,
}

fn arrondi(v: f64, decimales: i32) -> f64 {
    let p = 10f64.powi(decimales);
    (v * p).round() / p
}

/// Requête du site de trade pour cette recherche.
pub fn requete(r: &Recherche, budget_exalts: Option<u32>) -> Value {
    let filtres: Vec<Value> = r
        .poids
        .iter()
        .map(|p| json!({ "id": p.id, "value": { "weight": arrondi(p.poids, 3) } }))
        .collect();
    let mut filters = serde_json::Map::new();
    if let Some(c) = &r.categorie {
        filters.insert(
            "type_filters".into(),
            json!({ "filters": { "category": { "option": c } } }),
        );
    }
    let d = r.defenses_min;
    let mut equipement = serde_json::Map::new();
    for (cle, v) in [
        ("ar", d.armure),
        ("ev", d.evasion),
        ("es", d.bouclier_energie),
    ] {
        if v > 0.0 {
            equipement.insert(cle.into(), json!({ "min": (v * PART_DEFENSES).floor() }));
        }
    }
    if !equipement.is_empty() {
        filters.insert("equipment_filters".into(), json!({ "filters": equipement }));
    }
    if let Some(b) = budget_exalts.filter(|b| *b > 0) {
        filters.insert(
            "trade_filters".into(),
            json!({ "filters": { "price": { "max": b, "option": "exalted" } } }),
        );
    }
    let mut groupe = json!({ "type": "weight", "filters": filtres });
    if r.minimum > 0.0 {
        groupe["value"] = json!({ "min": arrondi(r.minimum, 1) });
    }
    json!({
        "query": {
            "status": { "option": "online" },
            "stats": [groupe],
            "filters": filters,
        },
        "sort": { "statgroup.0": "desc" },
    })
}

/// Adresse de la recherche sur le site de trade.
pub fn adresse(r: &Recherche, ligue: &str, budget_exalts: Option<u32>) -> String {
    let mut url = reqwest::Url::parse(SITE).expect("adresse du site de trade valide");
    if let Ok(mut segments) = url.path_segments_mut() {
        segments.pop_if_empty().push(ligue);
    }
    url.query_pairs_mut()
        .append_pair("q", &requete(r, budget_exalts).to_string());
    url.to_string()
}

/// Prépare la recherche pour l'objet de classe `classe`.
pub fn preparer(
    build: Option<&BuildPob>,
    classe: &str,
    ligue: &str,
    budget_exalts: Option<u32>,
) -> Result<UpgradeSearch, AppError> {
    let Some(build) = build else {
        return Err(app_error(
            ErrorCode::NoCharacter,
            "Importe ton personnage (code Path of Building) dans la fenêtre Augur.",
        ));
    };
    let r = recherche::recherche(build, classe).map_err(|e| {
        app_error(
            ErrorCode::FeatureDisabled,
            format!("Recherche impossible pour cet objet ({e})."),
        )
    })?;
    if r.poids.is_empty() {
        return Err(app_error(
            ErrorCode::FeatureDisabled,
            "Aucune stat de cet emplacement n'améliore ton personnage selon Augur.",
        ));
    }
    Ok(UpgradeSearch {
        url: adresse(&r, ligue, budget_exalts),
        stats: r.poids.iter().map(|p| p.libelle.clone()).collect(),
        with_dps: r.avec_dps,
    })
}

/// Ouvre l'adresse dans le navigateur par défaut.
pub fn ouvrir(url: &str) -> Result<(), AppError> {
    if !url.starts_with(SITE) {
        return Err(app_error(
            ErrorCode::Internal,
            "Adresse de recherche refusée.",
        ));
    }
    #[cfg(windows)]
    let lancement = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let lancement = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let lancement = std::process::Command::new("xdg-open").arg(url).spawn();
    lancement.map(|_| ()).map_err(|e| {
        app_error(
            ErrorCode::Internal,
            format!("Navigateur introuvable ({e})."),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use poe2_core::comparaison::defense::DefensesLocales;
    use poe2_core::comparaison::recherche::Poids;

    fn recherche_test() -> Recherche {
        Recherche {
            emplacement: "Body Armour".into(),
            categorie: Some("armour.chest".into()),
            poids: vec![Poids {
                id: "explicit.stat_3299347043".into(),
                libelle: "+# to maximum Life".into(),
                poids: 0.091_234,
                gain_dps_pct: 0.0,
                gain_ehp_pct: 4.56,
                valeur_testee: 50.0,
            }],
            minimum: 12.345,
            avec_dps: false,
            defenses_min: DefensesLocales {
                bouclier_energie: 300.0,
                ..Default::default()
            },
        }
    }

    #[test]
    fn requete_ponderee() {
        let q = requete(&recherche_test(), Some(50));
        let g = &q["query"]["stats"][0];
        assert_eq!(g["type"], "weight");
        assert_eq!(g["value"]["min"], 12.3);
        assert_eq!(g["filters"][0]["value"]["weight"], 0.091);
        let f = &q["query"]["filters"];
        assert_eq!(
            f["type_filters"]["filters"]["category"]["option"],
            "armour.chest"
        );
        assert_eq!(f["equipment_filters"]["filters"]["es"]["min"], 240.0);
        assert_eq!(f["trade_filters"]["filters"]["price"]["max"], 50);
        assert_eq!(q["sort"]["statgroup.0"], "desc");
        let sans_budget = requete(&recherche_test(), None);
        assert!(
            sans_budget["query"]["filters"]
                .get("trade_filters")
                .is_none()
        );
    }

    #[test]
    fn adresse_encodee() {
        let u = adresse(&recherche_test(), "Forbidden Rites", None);
        assert!(
            u.starts_with("https://www.pathofexile.com/trade2/search/poe2/Forbidden%20Rites?q=")
        );
        let url = reqwest::Url::parse(&u).unwrap();
        let q = url.query_pairs().find(|(k, _)| k == "q").unwrap().1;
        let v: Value = serde_json::from_str(&q).unwrap();
        assert_eq!(v["query"]["status"]["option"], "online");
    }

    #[test]
    fn refuse_une_autre_adresse() {
        assert!(ouvrir("https://exemple.org").is_err());
    }
}
