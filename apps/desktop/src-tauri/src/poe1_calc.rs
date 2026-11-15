//! « Cet objet est-il mieux que le mien ? » pour PoE 1 : Path of Building
//! Community (PoE 1) tourne dans Augur (crate `pob-poe1`) et compare l'objet
//! copié à celui porté par le build importé.
//!
//! PoB démarre en tâche de fond (quelques secondes) dès qu'un build PoE 1 est
//! importé ou que l'appli démarre en mode PoE 1 avec un build enregistré. Ensuite
//! chaque comparaison prend quelques dizaines de millisecondes.

use std::sync::OnceLock;

use pob_poe1::{Comparaison, Emplacement, ErreurPob, Moteur, Resume};
use poe2_core::parser::Rarete;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::character::CharacterState;
use crate::contracts::{
    AppError, Element, ErrorCode, ItemDelta, ResistanceDelta, StatDelta, SupportLevel, app_error,
};

#[derive(Default)]
pub struct Poe1Calc {
    moteur: OnceLock<Moteur>,
}

impl Poe1Calc {
    fn moteur(&self, app: &AppHandle) -> &Moteur {
        self.moteur.get_or_init(|| {
            let cache = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("augur"))
                .join("pob-poe1");
            log::info!(
                "démarrage de Path of Building PoE 1 ({})",
                pob_poe1::VERSION.trim()
            );
            Moteur::demarrer(cache)
        })
    }
}

/// Charge le build PoE 1 importé dans PoB s'il ne l'est pas déjà. Bloquant.
fn assurer_build(app: &AppHandle) -> Result<(), ErreurPob> {
    let xml = app
        .state::<CharacterState>()
        .poe1_xml()
        .ok_or(ErreurPob::SansBuild)?;
    let moteur = app.state::<Poe1Calc>().inner().moteur(app);
    if !moteur.est_charge(&xml) {
        let info = moteur.charger(&xml)?;
        log::info!(
            "build PoE 1 chargé dans PoB : {} {}, {}",
            info.ascendancy
                .as_deref()
                .or(info.class_name.as_deref())
                .unwrap_or("?"),
            info.level.unwrap_or(0),
            info.main_skill
                .as_deref()
                .unwrap_or("sans compétence principale")
        );
    }
    Ok(())
}

/// Prépare PoB en tâche de fond (au démarrage ou après un import PoE 1).
pub fn prechauffer(app: &AppHandle) {
    if app.state::<CharacterState>().poe1_xml().is_none() {
        return;
    }
    let h = app.clone();
    let _ = std::thread::Builder::new()
        .name("pob-poe1-prechauffe".into())
        .spawn(move || {
            if let Err(e) = assurer_build(&h) {
                log::warn!("Path of Building PoE 1 : {e}");
            }
        });
}

/// Ce que l'overlay PoE 1 affiche sous la cote : le delta (même panneau que PoE 2)
/// et les lignes de comparaison de PoB.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Poe1Evaluation {
    pub delta: ItemDelta,
    /// Autres emplacements essayés (second anneau, autre main).
    pub others: Vec<ItemDelta>,
    /// Lignes de PoB qui changent (« Life », « Total DPS »…), pour l'emplacement retenu.
    pub lines: Vec<Poe1Line>,
    pub pob_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Poe1Line {
    pub label: String,
    pub before: f64,
    pub after: f64,
    pub lower_is_better: bool,
}

fn resistances(avant: &Resume, apres: &Resume) -> Vec<ResistanceDelta> {
    let paires = [
        (
            Element::Fire,
            avant.resistances.fire,
            apres.resistances.fire,
        ),
        (
            Element::Cold,
            avant.resistances.cold,
            apres.resistances.cold,
        ),
        (
            Element::Lightning,
            avant.resistances.lightning,
            apres.resistances.lightning,
        ),
        (
            Element::Chaos,
            avant.resistances.chaos,
            apres.resistances.chaos,
        ),
    ];
    paires
        .into_iter()
        // Comme en PoE 2 : seules les résistances qui bougent sont montrées.
        .filter(|(_, a, b)| (a.value - b.value).abs() > 0.01 || (a.cap - b.cap).abs() > 0.01)
        .map(|(element, a, b)| ResistanceDelta {
            element,
            before: a.value,
            after: b.value,
            cap: b.cap,
        })
        .collect()
}

fn delta(c: &Comparaison, e: &Emplacement) -> ItemDelta {
    ItemDelta {
        slot: crate::evaluation::slot_id(&e.slot),
        equipped_name: e.equipped.clone(),
        weapon_set: None,
        // Calcul de Path of Building lui-même : pas d'approximation d'Augur.
        support: SupportLevel::Verified,
        dps: StatDelta {
            before: c.before.dps(),
            after: e.after.dps(),
        },
        ehp: StatDelta {
            before: c.before.ehp,
            after: e.after.ehp,
        },
        resistances: resistances(&c.before, &e.after),
        spirit: None,
        mechanics: Vec::new(),
        compute_ms: c.compute_ms,
    }
}

pub fn evaluation(c: &Comparaison) -> Option<Poe1Evaluation> {
    let meilleur = c.meilleur()?;
    Some(Poe1Evaluation {
        delta: delta(c, meilleur),
        others: c
            .slots
            .iter()
            .filter(|e| e.slot != meilleur.slot)
            .map(|e| delta(c, e))
            .collect(),
        lines: meilleur
            .lines
            .iter()
            .map(|l| Poe1Line {
                label: l.label.clone(),
                before: l.before,
                after: l.after,
                lower_is_better: l.lower_is_better,
            })
            .collect(),
        pob_version: pob_poe1::VERSION.trim().to_string(),
    })
}

fn erreur(e: ErreurPob) -> AppError {
    let code = match e {
        ErreurPob::SansBuild => ErrorCode::NoCharacter,
        _ => ErrorCode::Internal,
    };
    app_error(code, e.to_string())
}

/// Compare l'objet copié en dernier au build PoE 1 importé. Bloquant.
pub fn evaluer(app: &AppHandle) -> Result<Poe1Evaluation, AppError> {
    let texte = app.state::<crate::AppState>().last_text().ok_or_else(|| {
        app_error(
            ErrorCode::Internal,
            "Aucun objet copié : fais Ctrl+C sur un objet en jeu.",
        )
    })?;
    // Devises, gemmes, cartes : rien à comparer à l'équipement.
    if let Ok(objet) = poe2_core::parser::parser(&texte)
        && matches!(
            objet.rarete,
            Rarete::Devise | Rarete::Gemme | Rarete::CarteDivination
        )
    {
        return Err(app_error(
            ErrorCode::Unsupported,
            "Pas un objet d'équipement.",
        ));
    }
    assurer_build(app).map_err(erreur)?;
    let moteur = app.state::<Poe1Calc>().inner().moteur(app);
    let c = moteur.comparer(&texte).map_err(erreur)?;
    evaluation(&c).ok_or_else(|| {
        app_error(
            ErrorCode::Unsupported,
            "Ton build n'a pas d'emplacement pour cet objet.",
        )
    })
}

/// Budget PoE 1 des réglages, au format du site de trade.
fn budget(settings: &crate::settings::Settings) -> Option<pob_poe1::Budget> {
    let max = settings.poe1.upgrade_budget.filter(|b| *b > 0)?;
    let devise = match settings.poe1.upgrade_budget_currency.as_deref() {
        Some("divine") => "divine",
        _ => "chaos",
    };
    Some(pob_poe1::Budget {
        max,
        devise: devise.into(),
    })
}

/// Alt+U en PoE 1 : PoB pèse chaque mod possible de l'emplacement de l'objet
/// copié (gain de Full DPS et d'EHP), puis Augur ouvre la recherche pondérée.
/// Sans build ou pour un objet hors équipement, la recherche de l'objet lui-même.
pub fn rechercher_upgrade(
    app: &AppHandle,
) -> Result<crate::upgrade_search::UpgradeSearch, AppError> {
    let simple = |app: &AppHandle| {
        crate::poe1_items::open_trade(app).map(|url| crate::upgrade_search::UpgradeSearch {
            url,
            stats: Vec::new(),
            with_dps: false,
        })
    };
    let evaluation = match evaluer(app) {
        Ok(e) => e,
        Err(e) if matches!(e.code, ErrorCode::NoCharacter | ErrorCode::Unsupported) => {
            return simple(app);
        }
        Err(e) => return Err(e),
    };
    let slot = pob_slot(&evaluation.delta.slot);
    let settings = app.state::<crate::settings::SettingsStore>().get();
    let moteur = app.state::<Poe1Calc>().inner().moteur(app);
    let r = match moteur.recherche_upgrade(&slot, budget(&settings)) {
        Ok(r) => r,
        Err(ErreurPob::Calcul(m)) => {
            log::warn!("recherche d'upgrade PoE 1 ({slot}) : {m}");
            return simple(app);
        }
        Err(e) => return Err(erreur(e)),
    };
    log::info!(
        "recherche d'upgrade PoE 1 : {slot}, {} stats pesées en {} ms",
        r.filtres,
        r.compute_ms
    );
    let url = crate::upgrade_search::adresse_requete_poe1(
        settings.league_for(crate::game::Game::Poe1),
        &r.requete,
    );
    crate::upgrade_search::ouvrir(&url)?;
    Ok(crate::upgrade_search::UpgradeSearch {
        url,
        stats: r.stats,
        with_dps: evaluation.delta.dps.before > 0.0,
    })
}

/// Nom PoB d'un emplacement à partir de son identifiant de contrat.
fn pob_slot(id: &str) -> String {
    match id {
        "weapon1" => "Weapon 1",
        "offhand1" => "Weapon 2",
        "helmet" => "Helmet",
        "bodyArmour" => "Body Armour",
        "gloves" => "Gloves",
        "boots" => "Boots",
        "belt" => "Belt",
        "amulet" => "Amulet",
        "ring1" => "Ring 1",
        "ring2" => "Ring 2",
        autre => autre,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pob_poe1::{Ligne, Resistance, Resistances};

    fn resume(dps: f64, ehp: f64, chaos: f64) -> Resume {
        let r = |value| Resistance { value, cap: 75.0 };
        Resume {
            full_dps: 0.0,
            combined_dps: dps,
            life: 4000.0,
            energy_shield: 0.0,
            mana: 600.0,
            ehp,
            resistances: Resistances {
                fire: r(75.0),
                cold: r(75.0),
                lightning: r(75.0),
                chaos: r(chaos),
            },
        }
    }

    #[test]
    fn retient_le_meilleur_anneau() {
        let c = Comparaison {
            before: resume(1000.0, 20000.0, -30.0),
            slots: vec![
                Emplacement {
                    slot: "Ring 1".into(),
                    equipped: Some("Blight Eye".into()),
                    after: resume(900.0, 21000.0, -18.0),
                    lines: vec![],
                },
                Emplacement {
                    slot: "Ring 2".into(),
                    equipped: Some("Blight Whorl".into()),
                    after: resume(1100.0, 20500.0, -18.0),
                    lines: vec![Ligne {
                        label: "Life".into(),
                        before: 4000.0,
                        after: 4072.0,
                        lower_is_better: false,
                    }],
                },
            ],
            compute_ms: 30,
        };
        let e = evaluation(&c).unwrap();
        assert_eq!(e.delta.slot, "ring2");
        assert_eq!(e.delta.equipped_name.as_deref(), Some("Blight Whorl"));
        assert_eq!(e.delta.support, SupportLevel::Verified);
        assert_eq!(e.delta.dps.after, 1100.0);
        // Seule la résistance au chaos bouge.
        assert_eq!(e.delta.resistances.len(), 1);
        assert_eq!(e.delta.resistances[0].element, Element::Chaos);
        assert_eq!(e.others.len(), 1);
        assert_eq!(e.others[0].slot, "ring1");
        assert_eq!(e.lines.len(), 1);
    }

    #[test]
    fn emplacements_aller_retour() {
        for s in [
            "Weapon 1",
            "Weapon 2",
            "Helmet",
            "Body Armour",
            "Gloves",
            "Boots",
            "Belt",
            "Amulet",
            "Ring 1",
            "Ring 2",
        ] {
            assert_eq!(pob_slot(&crate::evaluation::slot_id(s)), s);
        }
    }

    #[test]
    fn budget_poe1() {
        let mut s = crate::settings::Settings::default();
        assert!(budget(&s).is_none());
        s.poe1.upgrade_budget = Some(3);
        assert_eq!(budget(&s).unwrap().devise, "chaos");
        s.poe1.upgrade_budget_currency = Some("divine".into());
        let b = budget(&s).unwrap();
        assert_eq!((b.max, b.devise.as_str()), (3, "divine"));
    }

    #[test]
    fn sans_emplacement() {
        let c = Comparaison {
            before: resume(1.0, 1.0, 0.0),
            slots: vec![],
            compute_ms: 0,
        };
        assert!(evaluation(&c).is_none());
    }
}
