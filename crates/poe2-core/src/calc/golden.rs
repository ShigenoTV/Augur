//! Cas de test « golden » du Theorycrafter (section 3.1 du plan fondateur).
//!
//! Un fichier YAML contient un cas ou une liste de cas. Ce module ne lit pas
//! le disque (frontière de `poe2-core`) : l'appelant fournit le texte, et les
//! tests d'intégration, la CI ou l'écran interne de l'application se
//! chargent de trouver les fichiers.
//!
//! ```yaml
//! id: sort-feu-nominal-001
//! patch: "0.3.0"
//! confiance: supposé          # confirmé | mesuré | supposé
//! personnage:
//!   competence: { nom: "Sort de test", temps_incantation: 0.8,
//!                 critique_base: 8, degats: { feu: [10, 20] } }
//!   modificateurs:
//!     - { stat: fire_damage, type: increased, valeur: 50, source: "Passif : X" }
//! conditions:
//!   cible: { resistances: { feu: 30 } }
//! attendu:
//!   tolerance_pct: 0.5        # tolérance par défaut du cas
//!   dps: 123.4
//!   chance_critique: { valeur: 8, tolerance_abs: 0 }
//! preuve: "lien vers la capture"
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use super::trace::nombre;
use super::{Conditions, Personnage, Resultats, calculer};

/// Taille maximale d'un fichier de cas : protège l'éditeur interne d'un
/// collage accidentel énorme.
pub const TAILLE_MAX_YAML: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confiance {
    #[serde(rename = "confirmé", alias = "confirme")]
    Confirme,
    #[serde(rename = "mesuré", alias = "mesure")]
    Mesure,
    #[serde(rename = "supposé", alias = "suppose")]
    Suppose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StatutCas {
    #[default]
    Actif,
    /// Mécanique pas encore implémentée : le cas est listé mais pas exécuté.
    NonSupporte,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ValeurAttendue {
    Nombre(f64),
    Detail {
        valeur: f64,
        #[serde(default)]
        tolerance_pct: Option<f64>,
        #[serde(default)]
        tolerance_abs: Option<f64>,
    },
}

fn tolerance_defaut() -> f64 {
    0.5
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Attendu {
    #[serde(default = "tolerance_defaut")]
    pub tolerance_pct: f64,
    #[serde(flatten)]
    pub valeurs: BTreeMap<String, ValeurAttendue>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CasGolden {
    pub id: String,
    pub patch: String,
    pub confiance: Confiance,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub statut: StatutCas,
    pub personnage: Personnage,
    #[serde(default)]
    pub conditions: Conditions,
    pub attendu: Attendu,
    #[serde(default)]
    pub preuve: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Fichier {
    Liste(Vec<CasGolden>),
    Un(Box<CasGolden>),
}

/// Lit un fichier YAML de cas golden (un cas ou une liste de cas).
pub fn lire(yaml: &str) -> Result<Vec<CasGolden>, String> {
    if yaml.len() > TAILLE_MAX_YAML {
        return Err(format!("fichier trop gros ({} octets)", yaml.len()));
    }
    // On tente d'abord un cas seul pour obtenir un message d'erreur précis.
    match serde_yaml_ng::from_str::<CasGolden>(yaml) {
        Ok(c) => Ok(vec![c]),
        Err(e_un) => match serde_yaml_ng::from_str::<Fichier>(yaml) {
            Ok(Fichier::Liste(v)) => Ok(v),
            Ok(Fichier::Un(c)) => Ok(vec![*c]),
            Err(_) => Err(e_un.to_string()),
        },
    }
}

/// Noms acceptés pour compatibilité avec le format du plan fondateur.
fn nom_sortie(nom: &str) -> &str {
    match nom {
        "tooltip_dps" => "dps",
        autre => autre,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparaison {
    pub sortie: String,
    pub attendu: f64,
    pub obtenu: Option<f64>,
    pub ecart_pct: Option<f64>,
    pub tolerance: String,
    pub ok: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Reussi,
    Echec,
    Erreur,
    NonSupporte,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultatCas {
    pub id: String,
    pub confiance: Confiance,
    pub verdict: Verdict,
    pub comparaisons: Vec<Comparaison>,
    pub erreurs: Vec<String>,
    #[serde(skip)]
    pub resultats: Option<Resultats>,
}

fn comparer(nom: &str, attendu: &ValeurAttendue, defaut_pct: f64, r: &Resultats) -> Comparaison {
    let (valeur, tol_pct, tol_abs) = match attendu {
        ValeurAttendue::Nombre(v) => (*v, defaut_pct, None),
        ValeurAttendue::Detail {
            valeur,
            tolerance_pct,
            tolerance_abs,
        } => (*valeur, tolerance_pct.unwrap_or(defaut_pct), *tolerance_abs),
    };
    let obtenu = r.valeur(nom_sortie(nom));
    let ecart_pct = obtenu.map(|o| {
        if valeur == 0.0 {
            if o == 0.0 { 0.0 } else { f64::INFINITY }
        } else {
            (o - valeur) / valeur * 100.0
        }
    });
    let (ok, tolerance) = match (obtenu, tol_abs) {
        (None, _) => (false, "-".to_string()),
        (Some(o), Some(abs)) => (
            (o - valeur).abs() <= abs + 1e-9,
            format!("±{}", nombre(abs)),
        ),
        (Some(o), None) => {
            let marge = (valeur.abs() * tol_pct / 100.0).max(1e-9);
            (
                (o - valeur).abs() <= marge,
                format!("±{} %", nombre(tol_pct)),
            )
        }
    };
    Comparaison {
        sortie: nom.to_string(),
        attendu: valeur,
        obtenu,
        ecart_pct,
        tolerance,
        ok,
    }
}

/// Exécute un cas golden.
pub fn executer(cas: &CasGolden) -> ResultatCas {
    let mut res = ResultatCas {
        id: cas.id.clone(),
        confiance: cas.confiance,
        verdict: Verdict::NonSupporte,
        comparaisons: Vec::new(),
        erreurs: Vec::new(),
        resultats: None,
    };
    if cas.statut == StatutCas::NonSupporte {
        return res;
    }
    if cas.attendu.valeurs.is_empty() {
        res.verdict = Verdict::Erreur;
        res.erreurs.push("aucune valeur attendue".to_string());
        return res;
    }
    match calculer(&cas.personnage, &cas.conditions) {
        Err(erreurs) => {
            res.verdict = Verdict::Erreur;
            res.erreurs = erreurs.iter().map(ToString::to_string).collect();
        }
        Ok(r) => {
            for (nom, attendu) in &cas.attendu.valeurs {
                let c = comparer(nom, attendu, cas.attendu.tolerance_pct, &r);
                if c.obtenu.is_none() {
                    let dispo: Vec<_> = r.noms().collect();
                    res.erreurs.push(format!(
                        "sortie « {nom} » inconnue ; sorties disponibles : {}",
                        dispo.join(", ")
                    ));
                }
                res.comparaisons.push(c);
            }
            res.verdict = if res.comparaisons.iter().all(|c| c.ok) && res.erreurs.is_empty() {
                Verdict::Reussi
            } else {
                Verdict::Echec
            };
            res.resultats = Some(r);
        }
    }
    res
}

/// Rapport Markdown « cas golden : attendu / obtenu / écart », prêt à être
/// commenté sur une PR par la CI.
pub fn rapport_markdown(resultats: &[ResultatCas]) -> String {
    let total = resultats.len();
    let reussis = resultats
        .iter()
        .filter(|r| r.verdict == Verdict::Reussi)
        .count();
    let ignores = resultats
        .iter()
        .filter(|r| r.verdict == Verdict::NonSupporte)
        .count();
    let mut s = String::new();
    let _ = writeln!(
        s,
        "### Cas golden : {reussis}/{} au vert ({ignores} non supporté(s))\n",
        total - ignores
    );
    let _ = writeln!(
        s,
        "| Cas | Confiance | Sortie | Attendu | Obtenu | Écart | Tolérance | |"
    );
    let _ = writeln!(s, "|---|---|---|---:|---:|---:|---|---|");
    for r in resultats {
        let conf = match r.confiance {
            Confiance::Confirme => "confirmé",
            Confiance::Mesure => "mesuré",
            Confiance::Suppose => "supposé",
        };
        if r.verdict == Verdict::NonSupporte {
            let _ = writeln!(s, "| `{}` | {conf} | | | | | | non supporté |", r.id);
            continue;
        }
        for e in &r.erreurs {
            let _ = writeln!(s, "| `{}` | {conf} | erreur | | | | | ❌ {e} |", r.id);
        }
        for c in &r.comparaisons {
            let _ = writeln!(
                s,
                "| `{}` | {conf} | {} | {} | {} | {} | {} | {} |",
                r.id,
                c.sortie,
                nombre(c.attendu),
                c.obtenu.map(nombre).unwrap_or_else(|| "-".into()),
                c.ecart_pct
                    .map(|e| format!("{} %", nombre(e)))
                    .unwrap_or_else(|| "-".into()),
                c.tolerance,
                if c.ok { "✅" } else { "❌" }
            );
        }
    }
    s
}
