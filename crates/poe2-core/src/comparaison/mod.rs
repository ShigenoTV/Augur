//! Comparaison d'un objet copié en jeu avec l'équipement du personnage
//! importé de Path of Building : « que m'apporte cet objet ? ».
//!
//! Le personnage est reconstruit pour le moteur à partir du build PoB :
//! gemme active (Lightning Bolt ou Arc, données RePoE au niveau effectif),
//! supports, mods des objets équipés et passifs alloués, chaque ligne de
//! texte passant par [`traduction`]. On calcule le DPS avec l'équipement
//! actuel, puis avec l'objet copié à la place de celui de son emplacement.
//!
//! Les conditions sont celles du panneau d'information du jeu : cible sans
//! résistance, mana plein, infusion comme le panneau. Tout ce qui n'est pas
//! traduit est compté dans le [`Rapport`] : le résultat est alors
//! « approximatif », jamais présenté comme exact.

pub mod donnees;
pub mod traduction;

use std::collections::BTreeSet;

use thiserror::Error;

use crate::calc::{self, Competence, Conditions, GenreCompetence, Modificateur, Personnage};
use crate::pob::{BuildPob, GemmePob, ObjetPob};
use traduction::{Niveaux, Traduction, traduire};

/// Compétences dont le moteur sait calculer le DPS (archétypes vérifiés).
pub const COMPETENCES_SUPPORTEES: &[&str] = &["LightningBoltPlayer", "ArcPlayer"];

#[derive(Debug, Error, PartialEq)]
pub enum ErreurComparaison {
    #[error("aucune compétence active dans le build")]
    AucuneCompetence,
    #[error("Augur ne calcule pas encore le DPS de {0} (seulement Lightning Bolt et Arc)")]
    CompetenceNonSupportee(String),
    #[error("objet non comparé : {0}")]
    EmplacementNonSupporte(String),
    #[error("calcul impossible : {0}")]
    Calcul(String),
}

/// Ce qui n'a pas pu être pris en compte.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rapport {
    /// Lignes d'objets pertinentes mais non traduites, avec leur objet.
    pub mods_non_traduits: Vec<String>,
    /// Lignes de l'objet comparé non traduites.
    pub mods_candidat_non_traduits: Vec<String>,
    pub passifs_non_traduits: usize,
    /// Lignes de passifs non traduites, sans doublon, avec leur nombre.
    pub lignes_passifs_non_traduites: Vec<(String, usize)>,
    pub passifs_inconnus: usize,
    /// Supports dont un effet n'est pas modélisé.
    pub supports_non_modelises: BTreeSet<String>,
    /// L'arbre du build n'est pas celui des données embarquées.
    pub arbre_autre_version: bool,
}

impl Rapport {
    pub fn complet(&self) -> bool {
        self.mods_non_traduits.is_empty()
            && self.mods_candidat_non_traduits.is_empty()
            && self.passifs_non_traduits == 0
            && self.passifs_inconnus == 0
            && self.supports_non_modelises.is_empty()
            && !self.arbre_autre_version
    }
}

/// Résultat de [`comparer`].
#[derive(Debug, Clone, PartialEq)]
pub struct Comparaison {
    /// Emplacement PoB remplacé (« Weapon 1 », « Ring 2 »…).
    pub emplacement: String,
    /// Nom de l'objet remplacé.
    pub remplace: Option<String>,
    /// Compétence comparée, avec son niveau effectif avant et après.
    pub competence: String,
    pub niveau_avant: u32,
    pub niveau_apres: u32,
    pub dps_avant: f64,
    pub dps_apres: f64,
    pub rapport: Rapport,
}

/// Groupe et gemme comparés : la compétence principale du build si c'est un
/// archétype vérifié, sinon le premier groupe actif qui en contient un,
/// sinon la compétence principale telle quelle (`false` : non supportée).
pub fn competence_comparee(build: &BuildPob) -> Option<(usize, &GemmePob, bool)> {
    let supportee = |g: &GemmePob| COMPETENCES_SUPPORTEES.contains(&g.skill_id.as_str());
    let principale = build.groupe_principal.zip(build.competence_principale());
    if let Some((i, g)) = principale
        && supportee(g)
    {
        return Some((i, g, true));
    }
    build
        .groupes
        .iter()
        .enumerate()
        .filter(|(_, gr)| gr.actif)
        .find_map(|(i, gr)| gr.actives().find(|g| supportee(g)).map(|g| (i, g, true)))
        .or(principale.map(|(i, g)| (i, g, false)))
}

/// Emplacements PoB possibles pour une classe d'objet du jeu, et ceux que
/// l'objet libère en plus (arme à deux mains).
pub fn emplacements(classe: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    const W1: &[&str] = &["Weapon 1"];
    const W2: &[&str] = &["Weapon 2"];
    Some(match classe {
        "Wands" | "Daggers" | "Claws" | "Spears" | "Flails" | "One Hand Maces"
        | "One Hand Swords" | "One Hand Axes" => (W1, &[]),
        // Un sceptre se tient dans l'une ou l'autre main, à côté d'une baguette
        // par exemple (règle de Path of Building) : il n'apporte souvent que de
        // l'Esprit et ne doit pas être comparé qu'à l'arme qui porte le DPS.
        "Sceptres" => (&["Weapon 1", "Weapon 2"], &[]),
        "Staves" | "Warstaves" | "Quarterstaves" | "Two Hand Maces" | "Two Hand Swords"
        | "Two Hand Axes" => (W1, W2),
        "Bows" | "Crossbows" => (W1, &[]),
        "Foci" | "Shields" | "Bucklers" | "Quivers" => (W2, &[]),
        "Rings" => (&["Ring 1", "Ring 2"], &[]),
        "Amulets" => (&["Amulet"], &[]),
        "Belts" => (&["Belt"], &[]),
        "Gloves" => (&["Gloves"], &[]),
        "Boots" => (&["Boots"], &[]),
        "Helmets" => (&["Helmet"], &[]),
        "Body Armours" => (&["Body Armour"], &[]),
        _ => return None,
    })
}

/// La main gauche peut-elle recevoir un sceptre ? Non si l'arme équipée
/// en main droite se tient à deux mains (bâton, arc, arbalète…). Une base
/// inconnue des données est supposée à une main.
fn main_gauche_libre(build: &BuildPob) -> bool {
    let Some(arme) = build.objet("Weapon 1") else {
        return true;
    };
    let Ok(donnees) = crate::forge::atelier::donnees() else {
        return true;
    };
    let noms: Vec<&str> = [arme.base.as_deref(), arme.nom.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    !donnees
        .bases
        .bases
        .iter()
        .filter(|b| b.tags.iter().any(|t| t == "twohand"))
        // Les objets magiques portent leurs affixes autour du nom de base.
        .any(|b| noms.iter().any(|n| n.contains(b.nom.as_str())))
}

/// Emplacements qui comptent pour le personnage : ni set d'armes de
/// rechange, ni flacons, ni charmes.
fn compte(emplacement: &str) -> bool {
    !(emplacement.ends_with("Swap")
        || emplacement.starts_with("Flask")
        || emplacement.starts_with("Charm"))
}

/// Lignes de mods d'un objet PoB : après la ligne « Implicits: N » si elle
/// existe (les lignes d'avant sont l'en-tête), sinon tout le texte.
fn lignes_objet(o: &ObjetPob) -> Vec<&str> {
    let lignes: Vec<&str> = o.texte.lines().collect();
    match lignes.iter().position(|l| l.starts_with("Implicits:")) {
        Some(i) => lignes[i + 1..].to_vec(),
        None => lignes,
    }
}

#[derive(Default)]
struct Accumulateur {
    mods: Vec<Modificateur>,
    niveaux: Niveaux,
}

impl Accumulateur {
    /// Ajoute une ligne ; `true` si elle est pertinente mais non traduite.
    fn ligne(&mut self, ligne: &str, source: &str) -> bool {
        match traduire(ligne, source) {
            Traduction::Mods(v) => self.mods.extend(v),
            Traduction::Niveaux(n) => {
                self.niveaux.toutes += n.toutes;
                self.niveaux.sorts += n.sorts;
                self.niveaux.foudre += n.foudre;
            }
            Traduction::NonTraduite => return true,
            Traduction::SansEffet => {}
        }
        false
    }
}

/// Personnage du moteur pour ce build, l'objet `candidat` (lignes de mods)
/// prenant la place des emplacements `retires`.
fn modeliser(
    build: &BuildPob,
    groupe: usize,
    gemme: &GemmePob,
    retires: &[&str],
    candidat: Option<(&str, &[String])>,
    rapport: &mut Rapport,
) -> Result<(Personnage, u32), ErreurComparaison> {
    let donnees = donnees::gemmes();
    let active = donnees
        .actives
        .get(&gemme.skill_id)
        .ok_or_else(|| ErreurComparaison::CompetenceNonSupportee(gemme.nom.clone()))?;
    let mut acc = Accumulateur::default();

    for o in build
        .objets
        .iter()
        .filter(|o| compte(&o.emplacement) && !retires.contains(&o.emplacement.as_str()))
    {
        let source = format!(
            "{} ({})",
            o.nom.as_deref().unwrap_or("objet"),
            o.emplacement
        );
        for l in lignes_objet(o) {
            if acc.ligne(l, &source) {
                rapport.mods_non_traduits.push(format!("{source} : {l}"));
            }
        }
    }
    if let Some((nom, lignes)) = candidat {
        let source = format!("{nom} (objet comparé)");
        for l in lignes {
            if acc.ligne(l, &source) {
                rapport.mods_candidat_non_traduits.push(l.clone());
            }
        }
    }

    let arbre = donnees::arbre();
    rapport.arbre_autre_version = build
        .version_arbre
        .as_deref()
        .is_some_and(|v| v != arbre.version);
    for id in &build.passifs {
        match arbre.passifs.get(&id.to_string()) {
            Some(lignes) => {
                let source = format!("Passif {id}");
                for l in lignes {
                    if acc.ligne(l, &source) {
                        rapport.passifs_non_traduits += 1;
                        let lignes = &mut rapport.lignes_passifs_non_traduites;
                        match lignes.iter_mut().find(|(t, _)| t == l) {
                            Some((_, n)) => *n += 1,
                            None => lignes.push((l.clone(), 1)),
                        }
                    }
                }
            }
            // Les nœuds sans stats (départs de classe) ne sont pas dans les
            // données : seuls ceux d'un autre arbre manquent vraiment.
            None if rapport.arbre_autre_version => rapport.passifs_inconnus += 1,
            None => {}
        }
    }

    if let Some(gr) = build.groupes.get(groupe) {
        for s in gr.supports() {
            let source = format!("Support : {}", s.nom);
            match donnees.supports.get(&s.skill_id) {
                Some(lignes) => {
                    for l in lignes {
                        if acc.ligne(l, &source) {
                            rapport.supports_non_modelises.insert(s.nom.clone());
                        }
                    }
                }
                None => {
                    rapport.supports_non_modelises.insert(s.nom.clone());
                }
            }
        }
    }

    let niveau_voulu = gemme.niveau as i32 + acc.niveaux.pour_sort_de_foudre();
    let (niveau, data) = active
        .niveau(niveau_voulu)
        .ok_or_else(|| ErreurComparaison::Calcul(format!("{} sans données", active.nom)))?;
    let competence = Competence {
        nom: format!("{} (niveau {niveau})", active.nom),
        genre: GenreCompetence::Sort,
        temps_incantation: active.temps_incantation,
        critique_base: active.critique_base,
        efficacite_degats_ajoutes: 100.0,
        degats: [(calc::stats::TypeDegats::Foudre, data.foudre)].into(),
        conversion: Default::default(),
        gain: Default::default(),
        chaines_base: data.chaines,
        qualite: gemme.qualite as f64,
        chaines_par_qualite: active.chaines_par_qualite,
        infusion: active.infusion.clone(),
    };
    Ok((
        Personnage {
            niveau: Some(build.niveau),
            classe: Some(build.classe_affichee().to_string()),
            competence: Some(competence),
            modificateurs: acc.mods,
            penalite_campagne: 0.0,
            objets: Vec::new(),
        },
        niveau,
    ))
}

fn dps(perso: &Personnage) -> Result<f64, ErreurComparaison> {
    let r = calc::calculer(perso, &Conditions::default()).map_err(|e| {
        ErreurComparaison::Calcul(
            e.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ; "),
        )
    })?;
    r.valeur("dps")
        .ok_or_else(|| ErreurComparaison::Calcul("DPS non calculé".into()))
}

/// Compare un objet (classe du jeu, nom, lignes de mods telles qu'affichées)
/// à l'équipement du build. Pour une bague, l'emplacement retenu est celui
/// où l'objet rapporte le plus.
pub fn comparer(
    build: &BuildPob,
    classe: &str,
    nom: &str,
    lignes: &[String],
) -> Result<Comparaison, ErreurComparaison> {
    let (groupe, gemme, supportee) =
        competence_comparee(build).ok_or(ErreurComparaison::AucuneCompetence)?;
    if !supportee {
        return Err(ErreurComparaison::CompetenceNonSupportee(gemme.nom.clone()));
    }
    let (choix, liberes) = emplacements(classe)
        .ok_or_else(|| ErreurComparaison::EmplacementNonSupporte(format!("classe « {classe} »")))?;

    let mut rapport_avant = Rapport::default();
    let (avant, niveau_avant) = modeliser(build, groupe, gemme, &[], None, &mut rapport_avant)?;
    let dps_avant = dps(&avant)?;

    let mut meilleur: Option<Comparaison> = None;
    for emplacement in choix {
        if *emplacement == "Weapon 2" && classe == "Sceptres" && !main_gauche_libre(build) {
            continue;
        }
        let mut retires = vec![*emplacement];
        retires.extend_from_slice(liberes);
        let mut rapport = rapport_avant.clone();
        rapport.mods_non_traduits.clear();
        rapport.passifs_non_traduits = 0;
        rapport.lignes_passifs_non_traduites.clear();
        rapport.passifs_inconnus = 0;
        rapport.supports_non_modelises.clear();
        let (apres, niveau_apres) = modeliser(
            build,
            groupe,
            gemme,
            &retires,
            Some((nom, lignes)),
            &mut rapport,
        )?;
        // Ce qui manque avant ou après rend le résultat approximatif.
        for m in &rapport_avant.mods_non_traduits {
            if !rapport.mods_non_traduits.contains(m) {
                rapport.mods_non_traduits.push(m.clone());
            }
        }
        rapport
            .supports_non_modelises
            .extend(rapport_avant.supports_non_modelises.iter().cloned());
        let c = Comparaison {
            emplacement: emplacement.to_string(),
            remplace: build.objet(emplacement).and_then(|o| o.nom.clone()),
            competence: gemme.nom.clone(),
            niveau_avant,
            niveau_apres,
            dps_avant,
            dps_apres: dps(&apres)?,
            rapport,
        };
        if meilleur.as_ref().is_none_or(|m| c.dps_apres > m.dps_apres) {
            meilleur = Some(c);
        }
    }
    meilleur.ok_or_else(|| ErreurComparaison::EmplacementNonSupporte(classe.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pob;

    fn build(objets: &str, supports: &str, passifs: &str) -> BuildPob {
        let xml = format!(
            r#"<PathOfBuilding2>
<Build level="90" className="Sorceress" ascendClassName="Stormweaver" mainSocketGroup="1"/>
<Tree activeSpec="1"><Spec nodes="{passifs}" treeVersion="0_5"/></Tree>
<Skills activeSkillSet="1"><SkillSet id="1">
<Skill enabled="true" mainActiveSkill="1">
<Gem nameSpec="Lightning Bolt" skillId="LightningBoltPlayer" level="20" quality="0"/>{supports}
</Skill></SkillSet></Skills>
<Items activeItemSet="1">{objets}</Items>
</PathOfBuilding2>"#
        );
        pob::lire_xml(&xml).unwrap()
    }

    const BAGUETTE: &str = r#"<Item id="1">Rarity: RARE
Doom Song
Withered Wand
Implicits: 0
60% increased Lightning Damage
+1 to Level of all Spell Skills
+30 to maximum Mana
</Item>
<ItemSet id="1"><Slot name="Weapon 1" itemId="1"/></ItemSet>"#;

    fn lignes(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn remplace_l_arme() {
        let b = build(BAGUETTE, "", "");
        let c = comparer(
            &b,
            "Wands",
            "Storm Song",
            &lignes(&[
                "100% increased Lightning Damage",
                "+2 to Level of all Spell Skills",
            ]),
        )
        .unwrap();
        assert_eq!(c.emplacement, "Weapon 1");
        assert_eq!(c.remplace.as_deref(), Some("Doom Song"));
        assert_eq!((c.niveau_avant, c.niveau_apres), (21, 22));
        // Niveau 21 : 34 à 642, +60 % ; cadence 1/0,85 arrondie à 1,18 ; critique 10 % × 2.
        let attendu_avant = (34.0 + 642.0) / 2.0 * 1.6 * 1.1 * 1.18;
        assert!(
            (c.dps_avant - attendu_avant).abs() < 0.01,
            "{}",
            c.dps_avant
        );
        assert!(c.dps_apres > c.dps_avant);
        assert!(c.rapport.complet(), "{:?}", c.rapport);
    }

    #[test]
    fn choisit_la_meilleure_bague_et_signale_les_lignes_inconnues() {
        let objets = r#"<Item id="1">Rarity: RARE
A
Ruby Ring
Implicits: 0
10% increased Lightning Damage
</Item>
<Item id="2">Rarity: RARE
B
Ruby Ring
Implicits: 0
40% increased Lightning Damage
</Item>
<ItemSet id="1"><Slot name="Ring 1" itemId="1"/><Slot name="Ring 2" itemId="2"/></ItemSet>"#;
        let b = build(objets, "", "");
        let c = comparer(
            &b,
            "Rings",
            "C",
            &lignes(&[
                "30% increased Lightning Damage",
                "Triggered Spells deal 16% increased Spell Damage",
            ]),
        )
        .unwrap();
        assert_eq!(c.emplacement, "Ring 1");
        assert!(c.dps_apres > c.dps_avant);
        assert_eq!(
            c.rapport.mods_candidat_non_traduits,
            vec!["Triggered Spells deal 16% increased Spell Damage"]
        );
        assert!(!c.rapport.complet());
    }

    #[test]
    fn applique_supports_et_passifs() {
        let zenith = r#"<Gem nameSpec="Zenith II" skillId="SupportZenithPlayerTwo" level="1"/>"#;
        let sans = comparer(&build(BAGUETTE, "", ""), "Amulets", "X", &[]).unwrap();
        let avec = comparer(&build(BAGUETTE, zenith, ""), "Amulets", "X", &[]).unwrap();
        assert!((avec.dps_avant / sans.dps_avant - 1.3).abs() < 1e-9);
        // Un passif à « 12% increased Lightning Damage » de l'arbre embarqué.
        let (id, _) = donnees::arbre()
            .passifs
            .iter()
            .find(|(_, l)| l == &&vec!["12% increased Lightning Damage".to_string()])
            .unwrap();
        let p = comparer(&build(BAGUETTE, "", id), "Amulets", "X", &[]).unwrap();
        assert!((p.dps_avant / sans.dps_avant - 1.72 / 1.6).abs() < 1e-9);
    }

    const SCEPTRE: [&str; 3] = [
        "58% increased Spirit",
        "+28 to maximum Mana",
        "Allies in your Presence have 9% increased Attack Speed",
    ];

    #[test]
    fn un_sceptre_va_en_main_gauche_a_cote_de_la_baguette() {
        let b = build(BAGUETTE, "", "");
        let c = comparer(&b, "Sceptres", "Morbid Chant", &lignes(&SCEPTRE)).unwrap();
        assert_eq!(c.emplacement, "Weapon 2");
        assert_eq!(c.remplace, None);
        // Esprit et mana n'apportent rien au DPS : pas de fausse régression.
        assert!((c.dps_apres - c.dps_avant).abs() < 1e-9, "{c:?}");
    }

    #[test]
    fn un_sceptre_remplace_le_focus_s_il_y_en_a_un() {
        let objets = BAGUETTE.replace(
            r#"<ItemSet id="1"><Slot name="Weapon 1" itemId="1"/>"#,
            r#"<Item id="2">Rarity: RARE
Gloom Veil
Twig Focus
Implicits: 0
20% increased Lightning Damage
</Item>
<ItemSet id="1"><Slot name="Weapon 1" itemId="1"/><Slot name="Weapon 2" itemId="2"/>"#,
        );
        let c = comparer(&build(&objets, "", ""), "Sceptres", "X", &lignes(&SCEPTRE)).unwrap();
        assert_eq!(c.emplacement, "Weapon 2");
        assert_eq!(c.remplace.as_deref(), Some("Gloom Veil"));
        assert!(c.dps_apres < c.dps_avant);
    }

    #[test]
    fn pas_de_sceptre_en_main_gauche_avec_un_baton() {
        let baton = r#"<Item id="1">Rarity: RARE
Doom Song
Ashen Staff
Implicits: 0
60% increased Lightning Damage
</Item>
<ItemSet id="1"><Slot name="Weapon 1" itemId="1"/></ItemSet>"#;
        let c = comparer(&build(baton, "", ""), "Sceptres", "X", &lignes(&SCEPTRE)).unwrap();
        assert_eq!(c.emplacement, "Weapon 1");
        assert_eq!(c.remplace.as_deref(), Some("Doom Song"));
    }

    #[test]
    fn refuse_les_cas_non_couverts() {
        let b = build(BAGUETTE, "", "");
        assert!(matches!(
            comparer(&b, "Life Flasks", "X", &[]),
            Err(ErreurComparaison::EmplacementNonSupporte(_))
        ));
        let orb = pob::lire_xml(
            r#"<PathOfBuilding2><Build level="1" className="Witch" mainSocketGroup="1"/>
<Skills activeSkillSet="1"><SkillSet id="1"><Skill enabled="true"><Gem nameSpec="Orb of Storms" skillId="OrbOfStormsPlayer" level="20"/></Skill></SkillSet></Skills></PathOfBuilding2>"#,
        )
        .unwrap();
        assert_eq!(
            comparer(&orb, "Wands", "X", &[]),
            Err(ErreurComparaison::CompetenceNonSupportee(
                "Orb of Storms".into()
            ))
        );
    }
}
