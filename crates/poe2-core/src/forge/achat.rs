//! « Crafter ou acheter » (demande de Max du 13/11/2026) : la recherche de l'objet visé sur le site de
//! trade, pour comparer son prix au coût espéré du craft.
//!
//! Aucune source publique ne cote les objets Rares (poe.ninja ne cote que les objets échangeables en
//! vrac, et l'API officielle demande un client OAuth que GGG n'accorde plus). Augur prépare donc la
//! recherche de l'objet (même base, chaque mod voulu au tier visé ou mieux) ; l'appli l'ouvre dans le
//! navigateur du joueur, qui lance la recherche et reporte le prix qu'il y voit. Augur n'envoie
//! aucune requête au site de trade.
//!
//! Identifiants des stats (`explicit.stat_…`) : `data/trade-stats.json`, généré par
//! `tools/gamedata/trade_stats.py` (voir `crates/poe2-core/data/README.md` pour l'attribution).

use super::atelier::{self, DemandePlan, ErreurAtelier, PalierAffixe};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const TRADE_STATS: &str = include_str!("../../data/trade-stats.json");

#[derive(Debug, Deserialize)]
struct EntreeStat {
    ids: Vec<String>,
    #[serde(default)]
    negate: bool,
}

#[derive(Debug, Deserialize)]
struct TableStats {
    stats: BTreeMap<String, EntreeStat>,
}

fn table() -> &'static TableStats {
    static T: OnceLock<TableStats> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(TRADE_STATS).expect("trade-stats.json embarqué valide"))
}

fn plage() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\((-?\d+(?:\.\d+)?)-(-?\d+(?:\.\d+)?)\)").expect("regex"))
}

fn nombre() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Pas de look-behind en `regex` : on capture le caractère qui précède.
    R.get_or_init(|| Regex::new(r"(^|[^\w#])([+-]?\d+(?:\.\d+)?)").expect("regex"))
}

/// Texte d'une ligne de mod, valeurs remplacées par `#`, « + » retiré, en minuscules (même règle
/// que `tools/gamedata/trade_stats.py`).
pub fn normaliser(ligne: &str) -> String {
    let t = plage().replace_all(ligne, "#");
    let t = nombre().replace_all(&t, "${1}#");
    t.replace("+#", "#").trim().to_lowercase()
}

fn valeur() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"\((-?\d+(?:\.\d+)?)-(-?\d+(?:\.\d+)?)\)|(?:^|[^\w#(])([+-]?\d+(?:\.\d+)?)")
            .expect("regex")
    })
}

/// Plus petite valeur de chaque nombre de la ligne, dans l'ordre (borne basse d'une plage, ou valeur
/// fixe).
fn bornes_basses(ligne: &str) -> Vec<f64> {
    let n = |m: Option<regex::Match>| m.and_then(|m| m.as_str().parse::<f64>().ok());
    valeur()
        .captures_iter(ligne)
        .filter_map(|c| match (n(c.get(1)), n(c.get(2)), n(c.get(3))) {
            (Some(a), Some(b), _) => Some(a.min(b)),
            (_, _, x) => x,
        })
        .collect()
}

/// Valeur que le site de trade donne à une ligne : la seule valeur, ou la moyenne de deux
/// (« Adds # to # »).
fn valeur_trade(valeurs: &[f64]) -> Option<f64> {
    match valeurs {
        [] => None,
        [v] => Some(*v),
        vs => Some(vs.iter().sum::<f64>() / vs.len() as f64),
    }
}

/// Un filtre de stat de la recherche.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltreAchat {
    /// Ligne du mod voulu (meilleur palier accessible).
    pub libelle: String,
    /// Identifiants de trade acceptés (stat globale ou locale).
    pub ids: Vec<String>,
    /// Valeur minimale (ou maximale si `negate`) du site de trade : la borne basse du tier visé.
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// Recherche de l'objet visé, prête à ouvrir.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RechercheAchat {
    pub base_nom: String,
    /// Corps de la recherche (`query` + `sort`) du site de trade.
    pub requete: Value,
    pub filtres: Vec<FiltreAchat>,
    /// Lignes de mods voulus que le site de trade ne sait pas chercher : la recherche est plus large.
    pub ignorees: Vec<String>,
}

fn arrondi(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Filtre d'une ligne de mod pour les paliers acceptés : borne basse la plus faible d'entre eux.
fn filtre_ligne(ligne_ref: &str, rang: usize, paliers: &[&PalierAffixe]) -> Option<FiltreAchat> {
    let e = table().stats.get(&normaliser(ligne_ref))?;
    let seuil = paliers
        .iter()
        .filter_map(|p| p.texte.split('\n').nth(rang))
        .filter_map(|l| valeur_trade(&bornes_basses(l)))
        .reduce(f64::min)
        .map(arrondi);
    // Stat rangée sous sa forme inverse (« reduced » en « increased » négatif) : au moins autant de
    // réduction, c'est au plus -seuil.
    let (min, max) = match seuil {
        Some(v) if e.negate => (None, Some(-v)),
        s => (s, None),
    };
    Some(FiltreAchat {
        libelle: ligne_ref.to_string(),
        ids: e.ids.clone(),
        min,
        max,
    })
}

/// Corps de la recherche : même base, objet non unique et non corrompu, en ligne, du moins cher au
/// plus cher. Un filtre à un identifiant va dans le groupe « and » ; un filtre à deux (globale ou
/// locale) a son propre groupe « count » (au moins un).
pub fn requete(base_nom: &str, filtres: &[FiltreAchat]) -> Value {
    let valeur = |f: &FiltreAchat| {
        let mut v = serde_json::Map::new();
        if let Some(m) = f.min {
            v.insert("min".into(), json!(m));
        }
        if let Some(m) = f.max {
            v.insert("max".into(), json!(m));
        }
        Value::Object(v)
    };
    let filtre = |id: &str, f: &FiltreAchat| {
        let v = valeur(f);
        if v.as_object().is_some_and(|o| o.is_empty()) {
            json!({ "id": id })
        } else {
            json!({ "id": id, "value": v })
        }
    };
    let et: Vec<Value> = filtres
        .iter()
        .filter(|f| f.ids.len() == 1)
        .map(|f| filtre(&f.ids[0], f))
        .collect();
    let mut groupes = vec![json!({ "type": "and", "filters": et })];
    for f in filtres.iter().filter(|f| f.ids.len() > 1) {
        groupes.push(json!({
            "type": "count",
            "value": { "min": 1 },
            "filters": f.ids.iter().map(|id| filtre(id, f)).collect::<Vec<_>>(),
        }));
    }
    json!({
        "query": {
            "status": { "option": "online" },
            "type": base_nom,
            "stats": groupes,
            "filters": {
                "type_filters": { "filters": { "rarity": { "option": "nonunique" } } },
                "misc_filters": { "filters": { "corrupted": { "option": "false" } } },
            },
        },
        "sort": { "price": "asc" },
    })
}

/// Recherche de l'objet que le plan vise.
pub fn recherche(demande: &DemandePlan) -> Result<RechercheAchat, ErreurAtelier> {
    let base_nom = atelier::bases()?
        .into_iter()
        .find(|b| b.id == demande.base_id)
        .map(|b| b.nom)
        .ok_or_else(|| ErreurAtelier::BaseInconnue(demande.base_id.clone()))?;
    let affixes = atelier::affixes(&demande.base_id, demande.ilvl)?;
    let mut filtres = Vec::new();
    let mut ignorees = Vec::new();
    for w in &demande.voulus {
        let g = affixes
            .affixes
            .iter()
            .find(|a| a.group == w.group && a.family == w.family)
            .ok_or_else(|| {
                ErreurAtelier::Objectif(format!("groupe {} absent de cette base", w.group))
            })?;
        let acceptes: Vec<&PalierAffixe> =
            g.paliers.iter().filter(|p| p.tier <= w.max_tier).collect();
        let reference = acceptes.last().copied().or(g.paliers.first());
        let Some(reference) = reference else { continue };
        for (rang, ligne) in reference.texte.split('\n').enumerate() {
            match filtre_ligne(ligne, rang, &acceptes) {
                Some(f) => filtres.push(f),
                None => ignorees.push(ligne.to_string()),
            }
        }
    }
    Ok(RechercheAchat {
        requete: requete(&base_nom, &filtres),
        base_nom,
        filtres,
        ignorees,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::WantedAffix;

    #[test]
    fn normalisation_identique_au_script() {
        assert_eq!(normaliser("+(10-19) to maximum Life"), "# to maximum life");
        assert_eq!(
            normaliser("Adds (3-4) to (5-8) Cold damage to Attacks"),
            "adds # to # cold damage to attacks"
        );
        assert_eq!(
            normaliser("(-10--5)% to Chaos Resistance"),
            "#% to chaos resistance"
        );
        assert_eq!(normaliser("15% increased Spirit"), "#% increased spirit");
    }

    #[test]
    fn bornes_des_plages_et_valeurs_fixes() {
        assert_eq!(bornes_basses("+(10-19) to maximum Life"), vec![10.0]);
        assert_eq!(
            bornes_basses("Adds (3-4) to (5-8) Cold damage to Attacks"),
            vec![3.0, 5.0]
        );
        assert_eq!(bornes_basses("Adds 1 to (2-3) Cold damage"), vec![1.0, 2.0]);
        assert_eq!(bornes_basses("Loads an additional bolt"), Vec::<f64>::new());
        assert_eq!(valeur_trade(&[3.0, 5.0]), Some(4.0));
    }

    #[test]
    fn chaque_ligne_de_mod_de_forge_est_reconnue_ou_listee() {
        let brut: Value = serde_json::from_str(TRADE_STATS).unwrap();
        let sans: Vec<String> = serde_json::from_value(brut["sans_stat"].clone()).unwrap();
        let d = atelier::donnees().unwrap();
        for m in &d.mods.mods {
            for l in m.texte.split('\n') {
                let n = normaliser(l);
                assert!(
                    n.is_empty() || table().stats.contains_key(&n) || sans.contains(&n),
                    "ligne inconnue de trade-stats.json : {n} (régénérer avec trade_stats.py)"
                );
            }
        }
    }

    #[test]
    fn recherche_vie_au_tier_vise() {
        let base = atelier::bases()
            .unwrap()
            .into_iter()
            .find(|b| b.classe == "Body Armour")
            .unwrap();
        let aff = atelier::affixes(&base.id, 82).unwrap();
        let vie = aff
            .affixes
            .iter()
            .find(|a| a.libelle.ends_with("to maximum Life"))
            .unwrap();
        let t2 = vie.paliers.iter().find(|p| p.tier == 2).unwrap();
        let demande = DemandePlan {
            base_id: base.id.clone(),
            ilvl: 82,
            voulus: vec![WantedAffix {
                group: vie.group,
                family: vie.family,
                max_tier: 2,
            }],
            exclues: vec![],
            objet_depart: None,
        };
        let r = recherche(&demande).unwrap();
        assert_eq!(r.base_nom, base.nom);
        assert!(r.ignorees.is_empty());
        assert_eq!(r.filtres.len(), 1);
        let f = &r.filtres[0];
        assert_eq!(f.ids, vec!["explicit.stat_3299347043".to_string()]);
        assert_eq!(f.min, valeur_trade(&bornes_basses(&t2.texte)));
        assert_eq!(r.requete["query"]["type"], base.nom.as_str());
        assert_eq!(
            r.requete["query"]["stats"][0]["filters"][0]["id"],
            "explicit.stat_3299347043"
        );
        assert_eq!(r.requete["sort"]["price"], "asc");
    }

    #[test]
    fn stat_inversee_donne_un_maximum() {
        let p = |tier, texte: &str| PalierAffixe {
            tier,
            niveau: 1,
            texte: texte.into(),
            poids: 1,
        };
        let (t1, t2) = (
            p(1, "(25-30)% reduced Flask Charges used"),
            p(2, "(15-20)% reduced Flask Charges used"),
        );
        let f = filtre_ligne(&t2.texte, 0, &[&t1, &t2]).unwrap();
        assert_eq!(f.ids, vec!["explicit.stat_644456512".to_string()]);
        assert_eq!((f.min, f.max), (None, Some(-15.0)));
    }

    #[test]
    fn stat_globale_ou_locale_dans_un_groupe_count() {
        let f = FiltreAchat {
            libelle: "+(10-15) to Armour".into(),
            ids: vec!["explicit.stat_a".into(), "explicit.stat_b".into()],
            min: Some(10.0),
            max: None,
        };
        let q = requete("Base", &[f]);
        let g = &q["query"]["stats"][1];
        assert_eq!(g["type"], "count");
        assert_eq!(g["value"]["min"], 1);
        assert_eq!(g["filters"][1]["value"]["min"], 10.0);
    }
}
