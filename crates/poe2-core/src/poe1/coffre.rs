//! Coffres de PoE 1 : lecture des réponses de `character-window/get-stash-items`
//! (site pathofexile.com) et valeur d'un relevé au prix poe.ninja.
//!
//! Aucune entrée-sortie ici : l'appli télécharge les onglets et les pages
//! poe.ninja, ce module les lit et fait les comptes. Un relevé ne garde que des
//! noms, des quantités et des valeurs, jamais rien du compte.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ninja::{self, Page};
use crate::parser::{Exigences, Objet, Rarete};

/// Onglet de coffre, tel que la liste des onglets (`tabs=1`) le décrit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Onglet {
    pub index: u32,
    pub nom: String,
    /// Type d'onglet du jeu (`CurrencyStash`, `QuadStash`…).
    pub genre: String,
}

/// Onglets décrits par une réponse `tabs=1`.
pub fn onglets(doc: &Value) -> Vec<Onglet> {
    doc["tabs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| {
            Some(Onglet {
                index: t["i"].as_u64()? as u32,
                nom: t["n"].as_str().unwrap_or("").to_string(),
                genre: t["type"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Objet d'un coffre et sa quantité (taille de la pile, 1 sinon).
#[derive(Debug, Clone, PartialEq)]
pub struct ObjetCoffre {
    pub objet: Objet,
    pub quantite: u32,
}

impl ObjetCoffre {
    /// Nom affiché : nom propre de l'unique, sinon la ligne de type.
    pub fn libelle(&self) -> String {
        match (&self.objet.nom, self.objet.rarete) {
            (Some(n), Rarete::Unique) => n.clone(),
            _ => self.objet.base.clone(),
        }
    }
}

/// Sockets du site (`[{group, sColour}]`) au format du texte copié (« B-B-R B »).
fn emplacements(sockets: &Value) -> Option<String> {
    let liste = sockets.as_array()?;
    if liste.is_empty() {
        return None;
    }
    let mut groupes: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for s in liste {
        groupes
            .entry(s["group"].as_u64().unwrap_or(0))
            .or_default()
            .push(s["sColour"].as_str().unwrap_or("?").to_string());
    }
    Some(
        groupes
            .values()
            .map(|g| g.join("-"))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Lit un objet d'une réponse du site. `None` pour les genres qu'Augur ne cote pas.
pub fn objet(item: &Value) -> Option<ObjetCoffre> {
    // frameType du site : 0 normal, 1 magique, 2 rare, 3 unique, 4 gemme,
    // 5 devise, 6 carte de divination, 9 relique (unique).
    let rarete = match item["frameType"].as_u64()? {
        0 => Rarete::Normal,
        1 => Rarete::Magique,
        2 => Rarete::Rare,
        3 | 9 => Rarete::Unique,
        4 => Rarete::Gemme,
        5 => Rarete::Devise,
        6 => Rarete::CarteDivination,
        _ => return None,
    };
    let nom = item["name"]
        .as_str()
        .filter(|n| !n.is_empty())
        .map(str::to_string);
    let type_ligne = item["typeLine"].as_str().unwrap_or("");
    let base = match rarete {
        Rarete::Unique | Rarete::Rare => item["baseType"].as_str().unwrap_or(type_ligne),
        _ => type_ligne,
    }
    .to_string();
    if base.is_empty() {
        return None;
    }
    Some(ObjetCoffre {
        objet: Objet {
            classe: None,
            rarete,
            nom,
            base,
            qualite: None,
            niveau_objet: item["ilvl"].as_u64().map(|v| v as u32),
            exigences: Exigences::default(),
            proprietes: Vec::new(),
            emplacements: emplacements(&item["sockets"]),
            taille_pile: None,
            mods: Vec::new(),
            corrompu: item["corrupted"].as_bool().unwrap_or(false),
            non_identifie: !item["identified"].as_bool().unwrap_or(true),
            miroir: false,
            sanctifie: false,
            influences: Vec::new(),
            texte_saveur: None,
            note: None,
            avance: false,
            lignes_inconnues: Vec::new(),
        },
        quantite: item["stackSize"].as_u64().unwrap_or(1).max(1) as u32,
    })
}

/// Objets d'une réponse d'onglet.
pub fn objets(doc: &Value) -> Vec<ObjetCoffre> {
    doc["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(objet)
        .collect()
}

/// Pages poe.ninja nécessaires pour coter ces objets, sans doublon, dans l'ordre.
pub fn pages_necessaires<'a>(objets: impl IntoIterator<Item = &'a ObjetCoffre>) -> Vec<Page> {
    let mut vues = BTreeSet::new();
    let mut out = Vec::new();
    for o in objets {
        for p in ninja::pages(&o.objet) {
            if vues.insert(p.chemin()) {
                out.push(p);
            }
        }
    }
    out
}

/// Ligne d'un relevé : un objet (regroupé par nom), sa quantité et sa valeur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LigneReleve {
    pub nom: String,
    pub quantite: u32,
    /// Valeur totale en Chaos (quantité × cote).
    pub chaos: f64,
}

/// Valeur d'un onglet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValeurOnglet {
    pub index: u32,
    pub nom: String,
    pub chaos: f64,
    pub objets: u32,
}

/// Relevé des coffres : valeur totale, par onglet et par objet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Releve {
    /// Date du relevé (RFC 3339), posée par l'appli.
    pub at: String,
    pub ligue: String,
    pub chaos: f64,
    /// Cours de la Divine en Chaos au moment du relevé.
    pub cours_divine: Option<f64>,
    pub onglets: Vec<ValeurOnglet>,
    /// Objets cotés, regroupés par nom, du plus précieux au moins précieux.
    pub lignes: Vec<LigneReleve>,
    /// Objets sans cote (rares, objets que poe.ninja ne suit pas).
    pub non_cotes: u32,
}

/// Cote unitaire d'un objet en Chaos, d'après les pages poe.ninja fournies.
pub fn cote_unitaire(
    o: &ObjetCoffre,
    pages: &dyn Fn(Page) -> Option<Value>,
    cours_divine: Option<f64>,
) -> Option<f64> {
    if o.objet.base == "Chaos Orb" {
        return Some(1.0);
    }
    ninja::pages(&o.objet).into_iter().find_map(|p| {
        let doc = pages(p)?;
        ninja::coter(p, &doc, &o.objet, cours_divine).map(|c| c.chaos)
    })
}

/// Valeur d'un relevé. `onglets` : chaque onglet relevé et ses objets.
pub fn evaluer(
    at: String,
    ligue: String,
    onglets: &[(Onglet, Vec<ObjetCoffre>)],
    pages: &dyn Fn(Page) -> Option<Value>,
    cours_divine: Option<f64>,
) -> Releve {
    let mut par_nom: BTreeMap<String, (u32, f64)> = BTreeMap::new();
    let mut valeurs = Vec::new();
    let mut non_cotes = 0;
    let mut total = 0.0;
    for (onglet, objets) in onglets {
        let mut chaos = 0.0;
        for o in objets {
            match cote_unitaire(o, pages, cours_divine) {
                Some(c) => {
                    let v = c * f64::from(o.quantite);
                    chaos += v;
                    let e = par_nom.entry(o.libelle()).or_default();
                    e.0 += o.quantite;
                    e.1 += v;
                }
                None => non_cotes += 1,
            }
        }
        total += chaos;
        valeurs.push(ValeurOnglet {
            index: onglet.index,
            nom: onglet.nom.clone(),
            chaos: chaos.round(),
            objets: objets.len() as u32,
        });
    }
    let mut lignes: Vec<LigneReleve> = par_nom
        .into_iter()
        .map(|(nom, (quantite, chaos))| LigneReleve {
            nom,
            quantite,
            chaos: (chaos * 10.0).round() / 10.0,
        })
        .collect();
    lignes.sort_by(|a, b| b.chaos.total_cmp(&a.chaos).then_with(|| a.nom.cmp(&b.nom)));
    Releve {
        at,
        ligue,
        chaos: total.round(),
        cours_divine,
        onglets: valeurs,
        lignes,
        non_cotes,
    }
}

/// Écart d'un objet entre deux relevés.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ecart {
    pub nom: String,
    pub quantite: i64,
    pub chaos: f64,
}

/// Différence entre deux relevés : gain total et objets qui ont bougé, du plus
/// gros écart (en valeur absolue) au plus petit.
pub fn difference(avant: &Releve, apres: &Releve) -> (f64, Vec<Ecart>) {
    let mut m: BTreeMap<&str, (i64, f64)> = BTreeMap::new();
    for l in &avant.lignes {
        let e = m.entry(&l.nom).or_default();
        e.0 -= i64::from(l.quantite);
        e.1 -= l.chaos;
    }
    for l in &apres.lignes {
        let e = m.entry(&l.nom).or_default();
        e.0 += i64::from(l.quantite);
        e.1 += l.chaos;
    }
    let mut ecarts: Vec<Ecart> = m
        .into_iter()
        .filter(|(_, (q, c))| *q != 0 || c.abs() >= 0.5)
        .map(|(nom, (quantite, chaos))| Ecart {
            nom: nom.to_string(),
            quantite,
            chaos: (chaos * 10.0).round() / 10.0,
        })
        .collect();
    ecarts.sort_by(|a, b| b.chaos.abs().total_cmp(&a.chaos.abs()));
    (apres.chaos - avant.chaos, ecarts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn donnees(nom: &str) -> Value {
        let chemin = format!("{}/tests/donnees/poe1/{nom}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap()
    }

    fn pages(p: Page) -> Option<Value> {
        match p {
            Page::Echange("Currency") => Some(donnees("ninja-currency.json")),
            Page::Echange("DivinationCard") => Some(donnees("ninja-cards.json")),
            Page::Objets("UniqueArmour") => Some(donnees("ninja-unique-armour.json")),
            _ => None,
        }
    }

    #[test]
    fn lit_les_onglets_et_les_objets() {
        let doc = donnees("stash-onglet.json");
        let o = onglets(&doc);
        assert_eq!(o.len(), 3);
        assert_eq!(
            (o[1].index, o[1].nom.as_str(), o[1].genre.as_str()),
            (1, "Butin", "QuadStash")
        );
        let objets = objets(&doc);
        assert_eq!(objets.len(), 6);
        assert_eq!(
            (objets[0].objet.base.as_str(), objets[0].quantite),
            ("Divine Orb", 3)
        );
        let covenant = &objets[2];
        assert_eq!(covenant.libelle(), "The Covenant");
        assert_eq!(covenant.objet.base, "Spidersilk Robe");
        assert_eq!(super::super::liens(&covenant.objet), 6);
        assert_eq!(objets[3].objet.rarete, Rarete::CarteDivination);
        assert_eq!(objets[4].objet.base, "Contract: Underbelly");
    }

    #[test]
    fn evalue_un_releve() {
        let doc = donnees("stash-onglet.json");
        let onglet = onglets(&doc).remove(0);
        let objets = objets(&doc);
        let necessaires = pages_necessaires(&objets);
        assert!(necessaires.contains(&Page::Echange("Currency")));
        assert!(necessaires.contains(&Page::Objets("UniqueArmour")));
        let cours = ninja::cours_divine(&donnees("ninja-currency.json"));
        let r = evaluer(
            "t".into(),
            "Standard".into(),
            &[(onglet, objets)],
            &pages,
            cours,
        );
        // 3 Divine (78) + 40 Chaos + Covenant 6L (7829) + 2 cartes (7,2).
        assert_eq!(
            r.chaos,
            (3.0 * 78.0 + 40.0 + 7829.0 + 2.0 * 7.2_f64).round()
        );
        assert_eq!(r.non_cotes, 2); // le contrat rare et la Rogue's Marker
        assert_eq!(r.lignes[0].nom, "The Covenant");
        assert_eq!(r.onglets[0].objets, 6);
    }

    #[test]
    fn difference_entre_deux_releves() {
        let ligne = |nom: &str, quantite, chaos| LigneReleve {
            nom: nom.into(),
            quantite,
            chaos,
        };
        let releve = |chaos, lignes| Releve {
            at: String::new(),
            ligue: String::new(),
            chaos,
            cours_divine: None,
            onglets: Vec::new(),
            lignes,
            non_cotes: 0,
        };
        let avant = releve(
            118.0,
            vec![ligne("Divine Orb", 1, 78.0), ligne("Chaos Orb", 40, 40.0)],
        );
        let apres = releve(
            296.0,
            vec![ligne("Divine Orb", 3, 234.0), ligne("Chaos Orb", 62, 62.0)],
        );
        let (gain, ecarts) = difference(&avant, &apres);
        assert_eq!(gain, 178.0);
        assert_eq!(
            ecarts[0],
            Ecart {
                nom: "Divine Orb".into(),
                quantite: 2,
                chaos: 156.0
            }
        );
        assert_eq!(ecarts[1].quantite, 22);
    }
}
