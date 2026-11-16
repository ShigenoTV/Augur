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

use crate::contracts::Element;
use crate::game::Game;

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
