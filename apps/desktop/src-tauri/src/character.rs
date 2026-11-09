//! Personnage du joueur, importé d'un code Path of Building PoE 2.
//!
//! Le code collé est gardé tel quel dans `personnage.json` (dossier de
//! données d'Augur) et relu au démarrage : c'est la seule chose persistée.
//! Il ne contient aucune donnée de compte.

use std::path::PathBuf;
use std::sync::Mutex;

use poe2_core::comparaison::competence_comparee;
use poe2_core::pob::{self, BuildPob};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::contracts::{CharacterSource, CharacterSummary};
use crate::time::rfc3339_now;

/// Événement émis à chaque import ou suppression (charge : `Option<CharacterInfo>`).
pub const EVENT_CHANGED: &str = "augur://character-changed";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fichier {
    code: String,
    imported_at: String,
}

/// Gemme résumée pour l'interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemInfo {
    pub name: String,
    pub level: u32,
    pub quality: u32,
}

/// Ce que l'interface montre du personnage importé.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterInfo {
    pub summary: CharacterSummary,
    pub imported_at: String,
    /// Compétence comparée par l'overlay.
    pub skill: Option<GemInfo>,
    pub supports: Vec<String>,
    /// La compétence comparée est un archétype vérifié (Lightning Bolt, Arc).
    pub skill_supported: bool,
    /// Emplacements équipés (noms PoB : « Weapon 1 », « Body Armour »…).
    pub equipped_slots: Vec<String>,
    pub passives: usize,
    pub tree_version: Option<String>,
}

struct Charge {
    build: BuildPob,
    info: CharacterInfo,
}

#[derive(Default)]
pub struct CharacterState(Mutex<Option<Charge>>);

fn fichier(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("personnage.json"))
}

/// Résumé d'un build importé.
pub fn describe(build: &BuildPob, imported_at: String) -> CharacterInfo {
    let comparee = competence_comparee(build);
    let supports = comparee
        .and_then(|(i, _, _)| build.groupes.get(i))
        .map(|gr| gr.supports().map(|g| g.nom.clone()).collect())
        .unwrap_or_default();
    CharacterInfo {
        summary: CharacterSummary {
            name: comparee
                .map(|(_, g, _)| format!("Build {}", g.nom))
                .unwrap_or_else(|| "Build Path of Building".into()),
            class: build.classe_affichee().to_string(),
            level: build.niveau,
            league: String::new(),
            source: CharacterSource::PobCode,
        },
        imported_at,
        skill: comparee.map(|(_, g, _)| GemInfo {
            name: g.nom.clone(),
            level: g.niveau,
            quality: g.qualite,
        }),
        supports,
        skill_supported: comparee.is_some_and(|(_, _, s)| s),
        equipped_slots: build.objets.iter().map(|o| o.emplacement.clone()).collect(),
        passives: build.passifs.len(),
        tree_version: build.version_arbre.clone(),
    }
}

impl CharacterState {
    /// Relit le personnage enregistré, s'il y en a un.
    pub fn load(app: &AppHandle) -> Self {
        let charge = fichier(app)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Fichier>(&b).ok())
            .and_then(|f| match pob::importer(&f.code) {
                Ok(build) => {
                    let info = describe(&build, f.imported_at);
                    Some(Charge { build, info })
                }
                Err(e) => {
                    log::warn!("personnage enregistré illisible : {e}");
                    None
                }
            });
        Self(Mutex::new(charge))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Charge>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn info(&self) -> Option<CharacterInfo> {
        self.lock().as_ref().map(|c| c.info.clone())
    }

    pub fn summary(&self) -> Option<CharacterSummary> {
        self.lock().as_ref().map(|c| c.info.summary.clone())
    }

    /// Exécute `f` sur le build chargé (pour le moteur de calcul).
    pub fn with_build<T>(&self, f: impl FnOnce(&BuildPob) -> T) -> Option<T> {
        self.lock().as_ref().map(|c| f(&c.build))
    }
}

/// Importe un code PoB, l'enregistre et prévient les fenêtres.
pub fn import(app: &AppHandle, code: &str) -> Result<CharacterInfo, String> {
    let build = pob::importer(code).map_err(|e| format!("Code Path of Building refusé : {e}."))?;
    let imported_at = rfc3339_now();
    let info = describe(&build, imported_at.clone());
    if let Some(path) = fichier(app) {
        let contenu = Fichier {
            code: code.trim().to_string(),
            imported_at,
        };
        let ecrit = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                let json = serde_json::to_vec_pretty(&contenu).map_err(std::io::Error::other)?;
                std::fs::write(&path, json)
            });
        if let Err(e) = ecrit {
            return Err(format!("Personnage non enregistré ({e})."));
        }
    }
    *app.state::<CharacterState>().lock() = Some(Charge {
        build,
        info: info.clone(),
    });
    let _ = app.emit(EVENT_CHANGED, Some(&info));
    Ok(info)
}

/// Oublie le personnage importé.
pub fn clear(app: &AppHandle) -> Result<(), String> {
    if let Some(path) = fichier(app) {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Personnage non supprimé ({e}).")),
        }
    }
    *app.state::<CharacterState>().lock() = None;
    let _ = app.emit(EVENT_CHANGED, None::<CharacterInfo>);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(groupes: &str, principal: u32) -> BuildPob {
        let xml = format!(
            r#"<PathOfBuilding2><Build level="90" className="Sorceress" ascendClassName="Stormweaver" mainSocketGroup="{principal}"/>
<Skills activeSkillSet="1"><SkillSet id="1">{groupes}</SkillSet></Skills></PathOfBuilding2>"#
        );
        pob::lire_xml(&xml).unwrap()
    }

    const ORB: &str = r#"<Skill enabled="true"><Gem nameSpec="Herald of Ash" skillId="HeraldOfAshPlayer" level="20"/></Skill>"#;
    const ARC: &str = r#"<Skill enabled="true"><Gem nameSpec="Arc" skillId="ArcPlayer" level="21" quality="20"/><Gem nameSpec="Zenith II" skillId="SupportZenithPlayerTwo"/></Skill>"#;

    #[test]
    fn prefere_l_archetype_verifie() {
        let b = build(&format!("{ORB}{ARC}"), 1);
        let info = describe(&b, "t".into());
        assert_eq!(info.skill.as_ref().unwrap().name, "Arc");
        assert!(info.skill_supported);
        assert_eq!(info.supports, vec!["Zenith II"]);
        assert_eq!(info.summary.name, "Build Arc");
        assert_eq!(info.summary.class, "Stormweaver");
        assert_eq!(info.summary.source, CharacterSource::PobCode);
    }

    #[test]
    fn garde_la_principale_si_aucun_archetype() {
        let b = build(ORB, 1);
        let info = describe(&b, "t".into());
        assert_eq!(info.skill.as_ref().unwrap().name, "Herald of Ash");
        assert!(!info.skill_supported);
    }
}
