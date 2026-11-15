//! « Crafter ou acheter » pour la Forge PoE 1 : recherche de l'objet visé sur le site de trade de
//! PoE 1 (même base, influence, chaque mod voulu au tier visé ou mieux), à comparer au coût moyen
//! du plan. Même principe que la Forge PoE 2 (`crate::forge::achat`) : Augur prépare la recherche,
//! l'appli l'ouvre dans le navigateur, aucune requête n'est envoyée au site.
//!
//! Identifiants des stats : `data/trade-stats-poe1.json`, généré par
//! `tools/gamedata/trade_stats_poe1.py` à partir de `TradeSiteStats.lua` de Path of Building
//! Community (PoE 1, MIT).

use std::sync::OnceLock;

use serde_json::json;

use super::{DemandePoe1, ErreurForgePoe1, affixes, bases};
use crate::forge::achat::{RechercheAchat, TableStats, filtres_voulus, requete};

const TRADE_STATS_POE1: &str = include_str!("../../../data/trade-stats-poe1.json");

fn table() -> &'static TableStats {
    static T: OnceLock<TableStats> = OnceLock::new();
    T.get_or_init(|| TableStats::lire(TRADE_STATS_POE1))
}

/// Filtre du site de trade de PoE 1 pour une influence de la Forge (Shaper, Elder…).
fn filtre_influence(influence: &str) -> Option<&'static str> {
    Some(match influence {
        "Shaper" => "shaper_item",
        "Elder" => "elder_item",
        "Crusader" => "crusader_item",
        "Hunter" => "hunter_item",
        "Redeemer" => "redeemer_item",
        "Warlord" => "warlord_item",
        _ => return None,
    })
}

/// Recherche de l'objet que la demande vise.
pub fn recherche(d: &DemandePoe1) -> Result<RechercheAchat, ErreurForgePoe1> {
    let base_nom = bases()?
        .into_iter()
        .find(|b| b.id == d.base_id)
        .map(|b| b.nom)
        .ok_or_else(|| ErreurForgePoe1::BaseInconnue(d.base_id.clone()))?;
    let a = affixes(&d.base_id, d.ilvl, d.influence.as_deref())?;
    let (filtres, ignorees) =
        filtres_voulus(table(), &a.affixes, &d.voulus).map_err(ErreurForgePoe1::Objectif)?;
    let mut req = requete(&base_nom, &filtres);
    if let Some(f) = d.influence.as_deref().and_then(filtre_influence)
        && let Some(misc) = req["query"]["filters"]["misc_filters"]["filters"].as_object_mut()
    {
        misc.insert(f.into(), json!({ "option": "true" }));
    }
    Ok(RechercheAchat {
        requete: req,
        base_nom,
        filtres,
        ignorees,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::WantedAffix;

    fn demande(base: &str, influence: Option<&str>, textes: &[&str]) -> DemandePoe1 {
        let a = affixes(base, 84, influence).unwrap();
        let voulus = textes
            .iter()
            .map(|t| {
                let g = a
                    .affixes
                    .iter()
                    .find(|g| g.libelle.contains(t))
                    .unwrap_or_else(|| panic!("{t} absent"));
                WantedAffix {
                    group: g.group,
                    family: g.family,
                    max_tier: 2,
                }
            })
            .collect();
        DemandePoe1 {
            base_id: base.into(),
            ilvl: 84,
            influence: influence.map(str::to_string),
            voulus,
            prix: Default::default(),
            objet_depart: None,
        }
    }

    fn base_anneau() -> String {
        bases()
            .unwrap()
            .into_iter()
            .find(|b| b.nom == "Two-Stone Ring")
            .unwrap()
            .id
    }

    #[test]
    fn cherche_la_vie_et_les_resistances() {
        let d = demande(&base_anneau(), None, &["maximum Life", "Cold Resistance"]);
        let r = recherche(&d).unwrap();
        assert_eq!(r.base_nom, "Two-Stone Ring");
        assert!(r.ignorees.is_empty(), "{:?}", r.ignorees);
        let vie = r
            .filtres
            .iter()
            .find(|f| f.libelle.contains("Life"))
            .unwrap();
        assert_eq!(vie.ids, ["explicit.stat_3299347043"]);
        assert!(vie.min.unwrap() > 50.0, "{vie:?}");
        assert_eq!(r.requete["query"]["type"], "Two-Stone Ring");
        assert_eq!(r.requete["sort"]["price"], "asc");
    }

    #[test]
    fn filtre_l_influence() {
        let base = base_anneau();
        let d = demande(&base, Some("Shaper"), &["maximum Life"]);
        let r = recherche(&d).unwrap();
        assert_eq!(
            r.requete["query"]["filters"]["misc_filters"]["filters"]["shaper_item"]["option"],
            "true"
        );
    }
}
