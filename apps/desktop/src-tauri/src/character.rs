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
use crate::game::Game;
use crate::time::rfc3339_now;

/// Événement émis à chaque import ou suppression (charge : `Option<CharacterInfo>`).
pub const EVENT_CHANGED: &str = "augur://character-changed";
/// Même chose pour le personnage cible (build visé).
pub const EVENT_TARGET_CHANGED: &str = "augur://target-changed";

/// Personnage actuel (celui que l'overlay compare) ou cible (build visé,
/// pour l'écart « où j'en suis, où je vais »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Actuel,
    Cible,
}

impl Role {
    fn event(self) -> &'static str {
        match self {
            Role::Actuel => EVENT_CHANGED,
            Role::Cible => EVENT_TARGET_CHANGED,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Role::Actuel => "Personnage",
            Role::Cible => "Personnage cible",
        }
    }
}

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
    /// XML du code PoB, pour le moteur PoE 1 (Path of Building embarqué).
    xml: Option<String>,
}

/// XML d'un code PoE 1 (le moteur PoE 2 n'en a pas besoin).
fn xml_pour(game: Game, code: &str) -> Option<String> {
    match game {
        Game::Poe1 => pob::decoder(code).ok(),
        Game::Poe2 => None,
    }
}

/// Personnage importé de chaque jeu. PoE 2 passe par le moteur d'Augur, PoE 1
/// par Path of Building embarqué (`poe1_calc`). La cible de chaque jeu ne
/// sert qu'à l'écart.
#[derive(Default)]
struct Slots {
    poe2: Option<Charge>,
    poe1: Option<Charge>,
    poe2_cible: Option<Charge>,
    poe1_cible: Option<Charge>,
}

impl Slots {
    fn slot(&mut self, game: Game, role: Role) -> &mut Option<Charge> {
        match (game, role) {
            (Game::Poe2, Role::Actuel) => &mut self.poe2,
            (Game::Poe1, Role::Actuel) => &mut self.poe1,
            (Game::Poe2, Role::Cible) => &mut self.poe2_cible,
            (Game::Poe1, Role::Cible) => &mut self.poe1_cible,
        }
    }
}

#[derive(Default)]
pub struct CharacterState(Mutex<Slots>);

fn fichier(app: &AppHandle, game: Game, role: Role) -> Option<PathBuf> {
    let nom = match (game, role) {
        (Game::Poe2, Role::Actuel) => "personnage.json",
        (Game::Poe1, Role::Actuel) => "personnage-poe1.json",
        (Game::Poe2, Role::Cible) => "personnage-cible.json",
        (Game::Poe1, Role::Cible) => "personnage-poe1-cible.json",
    };
    app.path().app_data_dir().ok().map(|d| d.join(nom))
}

fn lire_code(game: Game, code: &str) -> Result<BuildPob, pob::ErreurPob> {
    match game {
        Game::Poe2 => pob::importer(code),
        Game::Poe1 => pob::importer_poe1(code),
    }
}

/// Résumé d'un build PoE 1 : la compétence principale de PoB, sans comparaison.
fn describe_poe1(build: &BuildPob, imported_at: String) -> CharacterInfo {
    let principale = build.competence_principale();
    let supports = build
        .groupe_principal
        .and_then(|i| build.groupes.get(i))
        .map(|gr| gr.supports().map(|g| g.nom.clone()).collect())
        .unwrap_or_default();
    CharacterInfo {
        summary: CharacterSummary {
            name: principale
                .map(|g| format!("Build {}", g.nom))
                .unwrap_or_else(|| "Build Path of Building".into()),
            class: build.classe_affichee().to_string(),
            level: build.niveau,
            league: String::new(),
            source: CharacterSource::PobCode,
        },
        imported_at,
        skill: principale.map(|g| GemInfo {
            name: g.nom.clone(),
            level: g.niveau,
            quality: g.qualite,
        }),
        supports,
        skill_supported: false,
        equipped_slots: build.objets.iter().map(|o| o.emplacement.clone()).collect(),
        passives: build.passifs.len(),
        tree_version: build.version_arbre.clone(),
    }
}

fn describe_for(game: Game, build: &BuildPob, imported_at: String) -> CharacterInfo {
    match game {
        Game::Poe2 => describe(build, imported_at),
        Game::Poe1 => describe_poe1(build, imported_at),
    }
}

fn charger(app: &AppHandle, game: Game, role: Role) -> Option<Charge> {
    fichier(app, game, role)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Fichier>(&b).ok())
        .and_then(|f| match lire_code(game, &f.code) {
            Ok(build) => {
                let info = describe_for(game, &build, f.imported_at);
                let xml = xml_pour(game, &f.code);
                Some(Charge { build, info, xml })
            }
            Err(e) => {
                log::warn!(
                    "{} {} enregistré illisible : {e}",
                    role.label(),
                    game.label()
                );
                None
            }
        })
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
    /// Relit les personnages enregistrés (un par jeu), s'il y en a.
    pub fn load(app: &AppHandle) -> Self {
        Self(Mutex::new(Slots {
            poe2: charger(app, Game::Poe2, Role::Actuel),
            poe1: charger(app, Game::Poe1, Role::Actuel),
            poe2_cible: charger(app, Game::Poe2, Role::Cible),
            poe1_cible: charger(app, Game::Poe1, Role::Cible),
        }))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Slots> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Personnage du jeu actif.
    pub fn info(&self) -> Option<CharacterInfo> {
        self.info_role(Role::Actuel)
    }

    /// Personnage (actuel ou cible) du jeu actif.
    pub fn info_role(&self, role: Role) -> Option<CharacterInfo> {
        self.lock()
            .slot(crate::game::active(), role)
            .as_ref()
            .map(|c| c.info.clone())
    }

    /// Exécute `f` sur le build actuel et la cible du jeu `game`, s'ils sont importés tous les deux.
    pub fn with_pair<T>(&self, game: Game, f: impl FnOnce(&BuildPob, &BuildPob) -> T) -> Option<T> {
        let slots = self.lock();
        let (actuel, cible) = match game {
            Game::Poe2 => (&slots.poe2, &slots.poe2_cible),
            Game::Poe1 => (&slots.poe1, &slots.poe1_cible),
        };
        Some(f(&actuel.as_ref()?.build, &cible.as_ref()?.build))
    }

    /// Personnage PoE 2 (overlay et moteur de calcul).
    pub fn summary(&self) -> Option<CharacterSummary> {
        self.lock().poe2.as_ref().map(|c| c.info.summary.clone())
    }

    /// XML du build PoE 1 importé (pour Path of Building embarqué).
    pub fn poe1_xml(&self) -> Option<String> {
        self.lock().poe1.as_ref().and_then(|c| c.xml.clone())
    }

    /// Exécute `f` sur le build PoE 2 chargé (pour le moteur de calcul).
    pub fn with_build<T>(&self, f: impl FnOnce(&BuildPob) -> T) -> Option<T> {
        self.lock().poe2.as_ref().map(|c| f(&c.build))
    }
}

/// Importe un code PoB pour le jeu actif, l'enregistre et prévient les fenêtres.
pub fn import(app: &AppHandle, code: &str) -> Result<CharacterInfo, String> {
    import_role(app, code, Role::Actuel)
}

/// Importe le personnage actuel ou la cible du jeu actif.
pub fn import_role(app: &AppHandle, code: &str, role: Role) -> Result<CharacterInfo, String> {
    let game = crate::game::active();
    let build =
        lire_code(game, code).map_err(|e| format!("Code Path of Building refusé : {e}."))?;
    let imported_at = rfc3339_now();
    let info = describe_for(game, &build, imported_at.clone());
    if let Some(path) = fichier(app, game, role) {
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
            return Err(format!("{} non enregistré ({e}).", role.label()));
        }
    }
    *app.state::<CharacterState>().lock().slot(game, role) = Some(Charge {
        build,
        info: info.clone(),
        xml: match role {
            Role::Actuel => xml_pour(game, code),
            Role::Cible => None,
        },
    });
    let _ = app.emit(role.event(), Some(&info));
    Ok(info)
}

/// Oublie le personnage importé du jeu actif.
pub fn clear(app: &AppHandle) -> Result<(), String> {
    clear_role(app, Role::Actuel)
}

/// Oublie le personnage actuel ou la cible du jeu actif.
pub fn clear_role(app: &AppHandle, role: Role) -> Result<(), String> {
    let game = crate::game::active();
    if let Some(path) = fichier(app, game, role) {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{} non supprimé ({e}).", role.label())),
        }
    }
    *app.state::<CharacterState>().lock().slot(game, role) = None;
    let _ = app.emit(role.event(), None::<CharacterInfo>);
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
