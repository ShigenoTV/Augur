//! Écart entre le personnage actuel et le personnage cible du jeu actif
//! (carte « Écart » de Copilot) : « où j'en suis, où je vais ».
//!
//! Le détail (objets, mods, gemmes, passifs) vient de `poe2_core::cible`.
//! Les chiffres viennent du moteur d'Augur en PoE 2 (mêmes calculs que
//! l'overlay) et des stats exportées par Path of Building en PoE 1.

use poe2_core::calc::stats::TypeDegats;
use poe2_core::cible::{self, ChiffresPob, Ecart, Statut};
use poe2_core::comparaison::{self, Chiffres};
use poe2_core::pob::BuildPob;
use serde::Serialize;

use crate::contracts::{AppError, Element, ErrorCode, app_error};
use crate::game::Game;
use poe2_core::parser::{self, Rarete};

/// D'où viennent les chiffres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NumbersSource {
    /// Moteur d'Augur (PoE 2).
    Augur,
    /// Stats exportées avec le code Path of Building (PoE 1).
    Pob,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapResistance {
    pub element: Element,
    pub value: f64,
    /// Plafond, quand il est connu.
    pub cap: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildNumbers {
    pub skill: Option<String>,
    pub dps: Option<f64>,
    pub ehp: Option<f64>,
    pub life: Option<f64>,
    pub energy_shield: Option<f64>,
    pub resistances: Vec<GapResistance>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapNumbers {
    pub source: NumbersSource,
    pub approximate: bool,
    pub current: BuildNumbers,
    pub target: BuildNumbers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GapStatus {
    Reached,
    Improve,
    OtherItem,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeakerMod {
    pub target: String,
    pub current: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapItem {
    /// Emplacement PoB (« Helmet », « Ring 1 »…), traduit par l'interface.
    pub slot: String,
    pub current: Option<String>,
    pub target: Option<String>,
    pub target_unique: bool,
    pub status: GapStatus,
    pub target_mods: usize,
    pub missing_mods: Vec<String>,
    pub weaker_mods: Vec<WeakerMod>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingSupport {
    pub skill: String,
    pub support: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapPassives {
    pub to_take: usize,
    pub to_remove: usize,
    pub current: usize,
    pub target: usize,
}

/// Résultat de `get_gap`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapReport {
    pub numbers: GapNumbers,
    pub other_class: bool,
    pub current_class: String,
    pub target_class: String,
    pub items: Vec<GapItem>,
    pub missing_skills: Vec<String>,
    pub missing_supports: Vec<MissingSupport>,
    pub passives: GapPassives,
}

fn element(t: TypeDegats) -> Option<Element> {
    Some(match t {
        TypeDegats::Feu => Element::Fire,
        TypeDegats::Froid => Element::Cold,
        TypeDegats::Foudre => Element::Lightning,
        TypeDegats::Chaos => Element::Chaos,
        TypeDegats::Physique => return None,
    })
}

fn depuis_augur(c: &Chiffres) -> BuildNumbers {
    let d = &c.defenses;
    BuildNumbers {
        skill: c.competence.clone(),
        dps: c.dps,
        ehp: Some(d.ehp()),
        life: Some(d.vie),
        energy_shield: Some(d.bouclier_energie),
        resistances: d
            .resistances
            .iter()
            .filter_map(|r| {
                Some(GapResistance {
                    element: element(r.type_degats)?,
                    value: r.valeur(),
                    cap: Some(r.maximale),
                })
            })
            .collect(),
    }
}

fn depuis_pob(build: &BuildPob, c: &ChiffresPob) -> BuildNumbers {
    BuildNumbers {
        skill: build.competence_principale().map(|g| g.nom.clone()),
        dps: c.dps,
        ehp: c.ehp,
        life: c.vie,
        energy_shield: c.bouclier_energie,
        resistances: c
            .resistances
            .iter()
            .filter_map(|(t, v)| {
                Some(GapResistance {
                    element: element(*t)?,
                    value: *v,
                    cap: None,
                })
            })
            .collect(),
    }
}

fn numbers(game: Game, actuel: &BuildPob, cible: &BuildPob) -> GapNumbers {
    match game {
        Game::Poe2 => {
            let (a, c) = (comparaison::chiffres(actuel), comparaison::chiffres(cible));
            GapNumbers {
                source: NumbersSource::Augur,
                approximate: a.approximatif || c.approximatif,
                current: depuis_augur(&a),
                target: depuis_augur(&c),
            }
        }
        Game::Poe1 => GapNumbers {
            source: NumbersSource::Pob,
            approximate: false,
            current: depuis_pob(actuel, &cible::chiffres_pob(&actuel.stats)),
            target: depuis_pob(cible, &cible::chiffres_pob(&cible.stats)),
        },
    }
}

fn status(s: Statut) -> GapStatus {
    match s {
        Statut::Atteint => GapStatus::Reached,
        Statut::AAmeliorer => GapStatus::Improve,
        Statut::AutreObjet => GapStatus::OtherItem,
        Statut::Vide => GapStatus::Empty,
    }
}

/// Écart entre `actuel` et `cible` pour le jeu `game`.
pub fn report(game: Game, actuel: &BuildPob, cible: &BuildPob) -> GapReport {
    let Ecart {
        autre_classe,
        objets,
        competences_manquantes,
        supports_manquants,
        passifs_a_prendre,
        passifs_a_retirer,
        passifs_actuels,
        passifs_cible,
    } = cible::ecart(actuel, cible);
    GapReport {
        numbers: numbers(game, actuel, cible),
        other_class: autre_classe,
        current_class: actuel.classe_affichee().to_string(),
        target_class: cible.classe_affichee().to_string(),
        items: objets
            .into_iter()
            .map(|o| GapItem {
                slot: o.emplacement,
                current: o.actuel,
                target: o.cible,
                target_unique: o.cible_unique,
                status: status(o.statut),
                target_mods: o.mods_cible,
                missing_mods: o.mods_manquants,
                weaker_mods: o
                    .mods_plus_faibles
                    .into_iter()
                    .map(|m| WeakerMod {
                        target: m.cible,
                        current: m.actuel,
                    })
                    .collect(),
            })
            .collect(),
        missing_skills: competences_manquantes,
        missing_supports: supports_manquants
            .into_iter()
            .map(|s| MissingSupport {
                skill: s.competence,
                support: s.support,
            })
            .collect(),
        passives: GapPassives {
            to_take: passifs_a_prendre,
            to_remove: passifs_a_retirer,
            current: passifs_actuels,
            target: passifs_cible,
        },
    }
}

// --- Objet copié en jeu face à la cible (overlay et Alt+U) ----------------

/// Ce que l'overlay affiche sous l'objet copié : où il en est par rapport à
/// l'objet de la cible pour son emplacement.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProgress {
    /// Emplacement PoB (« Helmet », « Ring 1 »…).
    pub slot: String,
    pub target: Option<String>,
    pub target_unique: bool,
    pub status: GapStatus,
    pub reached: usize,
    pub total: usize,
    pub missing_mods: Vec<String>,
    pub weaker_mods: Vec<WeakerMod>,
    /// Même décompte pour l'objet porté (`None` : rien d'équipé ou pas de personnage).
    pub current_reached: Option<usize>,
}

/// Recherche de l'objet cible ouverte (Alt+U « vers la cible »).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetSearch {
    pub url: String,
    pub slot: String,
    pub unique: Option<String>,
    /// Mods cherchés (à au moins 80 % de la valeur de la cible).
    pub filters: Vec<String>,
    /// Mods de la cible que le site de trade ne sait pas chercher.
    pub ignored: Vec<String>,
}

fn rarete(r: Rarete) -> &'static str {
    match r {
        Rarete::Unique => "UNIQUE",
        Rarete::Rare => "RARE",
        Rarete::Magique => "MAGIC",
        _ => "NORMAL",
    }
}

/// Objet copié (texte du presse-papier) face à l'objet cible de son emplacement.
/// `None` : objet hors équipement, illisible, ou pas d'objet cible à cet emplacement.
pub fn progress(
    actuel: Option<&BuildPob>,
    cible: &BuildPob,
    texte: &str,
) -> Option<TargetProgress> {
    let objet = parser::parser(texte).ok()?;
    if objet.non_identifie {
        return None;
    }
    let lignes: Vec<String> = objet.mods.iter().map(|m| m.texte.clone()).collect();
    let p = cible::progression(
        actuel,
        cible,
        objet.classe.as_deref()?,
        rarete(objet.rarete),
        objet.nom.as_deref(),
        &objet.base,
        &lignes,
    )?;
    Some(TargetProgress {
        slot: p.emplacement,
        target: p.cible,
        target_unique: p.cible_unique,
        status: status(p.statut),
        reached: p.atteints,
        total: p.total,
        missing_mods: p.mods_manquants,
        weaker_mods: p
            .mods_plus_faibles
            .into_iter()
            .map(|m| WeakerMod {
                target: m.cible,
                current: m.actuel,
            })
            .collect(),
        current_reached: p.actuel_atteints,
    })
}

/// Budget de la recherche (réglages de la recherche d'upgrades du jeu).
fn budget(game: Game, settings: &crate::settings::Settings) -> Option<(u32, &'static str)> {
    match game {
        Game::Poe2 => settings
            .upgrade_budget
            .filter(|b| *b > 0)
            .map(|b| (b, "exalted")),
        Game::Poe1 => settings.poe1.upgrade_budget.filter(|b| *b > 0).map(|b| {
            let devise = match settings.poe1.upgrade_budget_currency.as_deref() {
                Some("divine") => "divine",
                _ => "chaos",
            };
            (b, devise)
        }),
    }
}

/// Prépare la recherche de l'objet cible de l'emplacement `slot` (sans l'ouvrir).
pub fn target_search(
    game: Game,
    cible: &BuildPob,
    slot: &str,
    settings: &crate::settings::Settings,
) -> Result<TargetSearch, AppError> {
    let o = cible::objet_cible(cible, slot).ok_or_else(|| {
        app_error(
            ErrorCode::Unsupported,
            "Ta cible n'a pas d'objet à cet emplacement.",
        )
    })?;
    let r = poe2_core::forge::achat::recherche_objet_cible(&o, game == Game::Poe1);
    if o.unique.is_none() && r.filtres.is_empty() {
        return Err(app_error(
            ErrorCode::Unsupported,
            "Aucun mod de l'objet cible ne se cherche sur le site de trade.",
        ));
    }
    let mut requete = r.requete;
    if let Some((max, devise)) = budget(game, settings)
        && let Some(filters) = requete["query"]
            .as_object_mut()
            .map(|q| q.entry("filters").or_insert_with(|| serde_json::json!({})))
            .and_then(|f| f.as_object_mut())
    {
        filters.insert(
            "trade_filters".into(),
            serde_json::json!({ "filters": { "price": { "max": max, "option": devise } } }),
        );
    }
    let ligue = settings.league_for(game);
    let url = match game {
        Game::Poe2 => crate::upgrade_search::adresse_requete(ligue, &requete),
        Game::Poe1 => crate::upgrade_search::adresse_requete_poe1(ligue, &requete),
    };
    Ok(TargetSearch {
        url,
        slot: o.emplacement,
        unique: o.unique,
        filters: r.filtres.into_iter().map(|f| f.libelle).collect(),
        ignored: r.ignorees,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use poe2_core::pob;

    fn build(racine: &str, life: f64, casque: &str) -> BuildPob {
        let xml = format!(
            r#"<{racine}><Build level="90" className="Sorceress" ascendClassName="None" mainSocketGroup="1"><PlayerStat stat="Life" value="{life}"/><PlayerStat stat="CombinedDPS" value="1000"/><PlayerStat stat="FireResist" value="75"/></Build>
<Skills activeSkillSet="1"><SkillSet id="1"><Skill enabled="true"><Gem nameSpec="Arc" skillId="ArcPlayer" level="20"/></Skill></SkillSet></Skills>
<Items activeItemSet="1"><Item id="1">Rarity: RARE
X
Voltaic Helm
Implicits: 0
{casque}</Item><ItemSet id="1"><Slot name="Helmet" itemId="1"/></ItemSet></Items></{racine}>"#
        );
        if racine == "PathOfBuilding" {
            pob::importer_poe1(&pob::encoder(&xml)).unwrap()
        } else {
            pob::lire_xml(&xml).unwrap()
        }
    }

    #[test]
    fn poe1_lit_les_stats_de_pob() {
        let a = build("PathOfBuilding", 3000.0, "+50 to maximum Life");
        let c = build("PathOfBuilding", 4500.0, "+90 to maximum Life");
        let r = report(Game::Poe1, &a, &c);
        assert_eq!(r.numbers.source, NumbersSource::Pob);
        assert_eq!(r.numbers.current.life, Some(3000.0));
        assert_eq!(r.numbers.target.life, Some(4500.0));
        assert_eq!(r.numbers.target.dps, Some(1000.0));
        assert_eq!(r.numbers.current.skill.as_deref(), Some("Arc"));
        assert_eq!(r.numbers.current.resistances[0].element, Element::Fire);
        assert_eq!(r.items[0].status, GapStatus::Improve);
        assert_eq!(r.items[0].weaker_mods[0].target, "+90 to maximum Life");
    }

    #[test]
    fn objet_copie_et_recherche_de_la_cible() {
        let a = build("PathOfBuilding2", 3000.0, "+50 to maximum Life");
        let c = build(
            "PathOfBuilding2",
            3000.0,
            "+90 to maximum Life\n+40% to Fire Resistance",
        );
        let texte = "Item Class: Helmets\nRarity: Rare\nGale Crown\nVoltaic Helm\n--------\nItem Level: 80\n--------\n+85 to maximum Life\n+12% to Cold Resistance\n";
        let p = progress(Some(&a), &c, texte).unwrap();
        assert_eq!(p.slot, "Helmet");
        assert_eq!((p.reached, p.total), (1, 2));
        assert_eq!(p.missing_mods, vec!["+40% to Fire Resistance"]);
        assert_eq!(p.current_reached, Some(0));
        assert!(progress(Some(&a), &c, "pas un objet").is_none());

        let s = crate::settings::Settings {
            upgrade_budget: Some(5),
            ..Default::default()
        };
        let r = target_search(Game::Poe2, &c, "Helmet", &s).unwrap();
        assert_eq!(r.filters.len(), 2);
        assert!(
            r.url
                .starts_with("https://www.pathofexile.com/trade2/search/poe2/")
        );
        assert!(r.url.contains("exalted"));
        assert!(target_search(Game::Poe2, &c, "Gloves", &s).is_err());
    }

    #[test]
    fn poe2_passe_par_le_moteur() {
        let a = build("PathOfBuilding2", 3000.0, "+50 to maximum Life");
        let c = build("PathOfBuilding2", 3000.0, "+50 to maximum Life");
        let r = report(Game::Poe2, &a, &c);
        assert_eq!(r.numbers.source, NumbersSource::Augur);
        assert_eq!(r.numbers.current.dps, r.numbers.target.dps);
        assert!(r.numbers.current.dps.is_some());
        assert_eq!(r.items[0].status, GapStatus::Reached);
        assert!(!r.other_class);
    }
}
