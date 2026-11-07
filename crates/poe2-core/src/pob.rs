//! Lecture d'un code d'export Path of Building PoE 2.
//!
//! Un code PoB est le XML du build compressé en zlib puis encodé en base64
//! « URL » (`-` et `_` à la place de `+` et `/`). On en tire ce dont Augur a
//! besoin pour comparer un objet au personnage : niveau, classe, compétence
//! principale (gemme, niveau, qualité, supports), objets équipés par
//! emplacement et passifs alloués.
//!
//! Le code est une entrée non fiable (collé par le joueur) : la taille du code
//! et celle du XML décompressé sont bornées, et rien ici ne panique.

use std::io::Read;

use base64::Engine as _;
use serde::Serialize;
use thiserror::Error;

/// Taille maximale d'un code accepté (un gros build fait ~50 Ko).
pub const TAILLE_MAX_CODE: usize = 2 * 1024 * 1024;
/// Taille maximale du XML décompressé.
pub const TAILLE_MAX_XML: u64 = 16 * 1024 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErreurPob {
    #[error("le code est vide")]
    Vide,
    #[error(
        "c'est un lien : ouvre-le et copie le code d'export (bouton « Copy » de pobb.in ou « Generate » de Path of Building)"
    )]
    Lien,
    #[error("le code est trop long ({0} caractères)")]
    TropLong(usize),
    #[error("ce n'est pas un code Path of Building (base64 invalide)")]
    Base64,
    #[error("ce n'est pas un code Path of Building (données compressées illisibles)")]
    Compression,
    #[error("le build décompressé dépasse la taille autorisée")]
    TropGros,
    #[error("XML du build illisible : {0}")]
    Xml(String),
    #[error("ce code vient de Path of Building pour PoE 1, pas pour PoE 2")]
    PoE1,
    #[error("le build ne contient pas de section {0}")]
    SectionManquante(&'static str),
}

/// Gemme d'un groupe de compétences.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemmePob {
    /// Nom affiché (« Arc », « Zenith II »).
    pub nom: String,
    /// Identifiant interne de la compétence (« ArcPlayer »).
    pub skill_id: String,
    pub niveau: u32,
    pub qualite: u32,
    pub support: bool,
    pub active: bool,
}

/// Un groupe de compétences (une gemme active et ses supports).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupePob {
    pub actif: bool,
    /// Emplacement d'objet qui fournit la compétence, s'il y en a un.
    pub emplacement: Option<String>,
    pub gemmes: Vec<GemmePob>,
}

impl GroupePob {
    /// Gemmes actives (non supports) du groupe.
    pub fn actives(&self) -> impl Iterator<Item = &GemmePob> {
        self.gemmes.iter().filter(|g| !g.support)
    }

    /// Supports activés du groupe.
    pub fn supports(&self) -> impl Iterator<Item = &GemmePob> {
        self.gemmes.iter().filter(|g| g.support && g.active)
    }
}

/// Objet équipé dans un emplacement du set d'objets actif.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjetPob {
    /// Emplacement PoB (« Weapon 1 », « Body Armour », « Ring 1 »…).
    pub emplacement: String,
    /// Rareté (« RARE », « UNIQUE »…).
    pub rarete: Option<String>,
    /// Nom de l'objet (rare ou unique) ou base (normal, magique).
    pub nom: Option<String>,
    /// Base de l'objet, si distincte du nom.
    pub base: Option<String>,
    /// Texte brut de l'objet tel que PoB l'enregistre.
    pub texte: String,
}

/// Ce qu'Augur retient d'un build Path of Building PoE 2.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildPob {
    pub niveau: u32,
    pub classe: String,
    pub ascendance: Option<String>,
    /// Groupes de compétences du set actif, dans l'ordre de PoB.
    pub groupes: Vec<GroupePob>,
    /// Index (0) du groupe principal dans `groupes`, s'il existe.
    pub groupe_principal: Option<usize>,
    /// Index (0) de la gemme principale parmi les gemmes actives du groupe principal.
    pub gemme_principale: usize,
    pub objets: Vec<ObjetPob>,
    /// Passifs alloués (identifiants de nœuds de l'arbre).
    pub passifs: Vec<u32>,
    /// Version de l'arbre passif (« 0_4 »).
    pub version_arbre: Option<String>,
}

impl BuildPob {
    /// Gemme principale du build (celle que PoB affiche dans le panneau de calcul).
    pub fn competence_principale(&self) -> Option<&GemmePob> {
        let groupe = self.groupes.get(self.groupe_principal?)?;
        let mut actives = groupe.actives();
        let n = groupe.actives().count();
        if n == 0 {
            return None;
        }
        actives.nth(self.gemme_principale.min(n - 1))
    }

    /// Supports du groupe principal.
    pub fn supports_principaux(&self) -> Vec<&GemmePob> {
        self.groupe_principal
            .and_then(|i| self.groupes.get(i))
            .map(|g| g.supports().collect())
            .unwrap_or_default()
    }

    /// Objet équipé dans cet emplacement PoB.
    pub fn objet(&self, emplacement: &str) -> Option<&ObjetPob> {
        self.objets.iter().find(|o| o.emplacement == emplacement)
    }

    /// Classe affichée : l'ascendance si elle est choisie, sinon la classe.
    pub fn classe_affichee(&self) -> &str {
        self.ascendance.as_deref().unwrap_or(&self.classe)
    }
}

/// Décode un code d'export en XML.
pub fn decoder(code: &str) -> Result<String, ErreurPob> {
    let code = code.trim();
    if code.is_empty() {
        return Err(ErreurPob::Vide);
    }
    if code.starts_with("http://") || code.starts_with("https://") {
        return Err(ErreurPob::Lien);
    }
    if code.len() > TAILLE_MAX_CODE {
        return Err(ErreurPob::TropLong(code.len()));
    }
    // Espaces et retours à la ligne ajoutés par un copier-coller.
    let propre: String = code
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '+' => '-',
            '/' => '_',
            c => c,
        })
        .collect();
    let propre = propre.trim_end_matches('=');
    let octets = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(propre)
        .map_err(|_| ErreurPob::Base64)?;
    let mut xml = String::new();
    let lu = flate2::read::ZlibDecoder::new(octets.as_slice())
        .take(TAILLE_MAX_XML + 1)
        .read_to_string(&mut xml)
        .map_err(|_| ErreurPob::Compression)?;
    if lu as u64 > TAILLE_MAX_XML {
        return Err(ErreurPob::TropGros);
    }
    Ok(xml)
}

/// Encode un XML en code d'export (pour les tests et les fixtures).
pub fn encoder(xml: &str) -> String {
    use std::io::Write;
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    // L'écriture dans un Vec ne peut pas échouer.
    let _ = e.write_all(xml.as_bytes());
    let octets = e.finish().unwrap_or_default();
    base64::engine::general_purpose::URL_SAFE.encode(octets)
}

/// Décode et lit un code d'export.
pub fn importer(code: &str) -> Result<BuildPob, ErreurPob> {
    lire_xml(&decoder(code)?)
}

fn attr_u32(n: roxmltree::Node, nom: &str) -> Option<u32> {
    n.attribute(nom).and_then(|v| v.trim().parse().ok())
}

fn attr_bool(n: roxmltree::Node, nom: &str, defaut: bool) -> bool {
    match n.attribute(nom) {
        Some("true") => true,
        Some("false") => false,
        _ => defaut,
    }
}

fn enfants<'a, 'i>(
    n: roxmltree::Node<'a, 'i>,
    nom: &'static str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> {
    n.children().filter(move |c| c.has_tag_name(nom))
}

/// Lit le XML d'un build PoB PoE 2.
pub fn lire_xml(xml: &str) -> Result<BuildPob, ErreurPob> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| ErreurPob::Xml(e.to_string()))?;
    let racine = doc.root_element();
    if racine.has_tag_name("PathOfBuilding") {
        return Err(ErreurPob::PoE1);
    }
    if !racine.has_tag_name("PathOfBuilding2") {
        return Err(ErreurPob::Xml(format!(
            "racine inattendue « {} »",
            racine.tag_name().name()
        )));
    }

    let build = enfants(racine, "Build")
        .next()
        .ok_or(ErreurPob::SectionManquante("Build"))?;
    let niveau = attr_u32(build, "level").unwrap_or(1).clamp(1, 100);
    let classe = build.attribute("className").unwrap_or("").to_string();
    let ascendance = build
        .attribute("ascendClassName")
        .filter(|a| !a.is_empty() && *a != "None")
        .map(str::to_string);
    let socket_principal = attr_u32(build, "mainSocketGroup").unwrap_or(1);

    let (groupes, gemme_principale) = lire_competences(racine);
    let groupe_principal = (socket_principal as usize)
        .checked_sub(1)
        .filter(|&i| i < groupes.len());
    let gemme_principale = groupe_principal
        .and_then(|i| gemme_principale.get(i).copied())
        .unwrap_or(0);

    let (passifs, version_arbre) = lire_arbre(racine);

    Ok(BuildPob {
        niveau,
        classe,
        ascendance,
        groupes,
        groupe_principal,
        gemme_principale,
        objets: lire_objets(racine, &passifs),
        passifs,
        version_arbre,
    })
}

/// Groupes du set de compétences actif, et pour chacun l'index de sa gemme
/// active principale (`mainActiveSkill`, 1 dans PoB).
fn lire_competences(racine: roxmltree::Node) -> (Vec<GroupePob>, Vec<usize>) {
    let Some(skills) = enfants(racine, "Skills").next() else {
        return (Vec::new(), Vec::new());
    };
    let set_actif = skills.attribute("activeSkillSet").unwrap_or("1");
    // Anciens builds : les <Skill> sont directement sous <Skills>.
    let conteneur = enfants(skills, "SkillSet")
        .find(|s| s.attribute("id") == Some(set_actif))
        .or_else(|| enfants(skills, "SkillSet").next())
        .unwrap_or(skills);

    let mut groupes = Vec::new();
    let mut principales = Vec::new();
    for skill in enfants(conteneur, "Skill") {
        let gemmes = enfants(skill, "Gem")
            .map(|g| {
                let skill_id = g.attribute("skillId").unwrap_or("").to_string();
                let gem_id = g.attribute("gemId").unwrap_or("");
                GemmePob {
                    nom: g
                        .attribute("nameSpec")
                        .filter(|n| !n.is_empty())
                        .unwrap_or(&skill_id)
                        .to_string(),
                    support: skill_id.starts_with("Support") || gem_id.contains("/SupportGem"),
                    skill_id,
                    niveau: attr_u32(g, "level").unwrap_or(1),
                    qualite: attr_u32(g, "quality").unwrap_or(0),
                    active: attr_bool(g, "enabled", true),
                }
            })
            .collect();
        principales.push(
            attr_u32(skill, "mainActiveSkill")
                .unwrap_or(1)
                .saturating_sub(1) as usize,
        );
        groupes.push(GroupePob {
            actif: attr_bool(skill, "enabled", true),
            emplacement: skill
                .attribute("slot")
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            gemmes,
        });
    }
    (groupes, principales)
}

/// Spec actif de l'arbre.
fn spec_actif<'a, 'i>(racine: roxmltree::Node<'a, 'i>) -> Option<roxmltree::Node<'a, 'i>> {
    let tree = enfants(racine, "Tree").next()?;
    let actif: usize = attr_u32(tree, "activeSpec").unwrap_or(1).max(1) as usize;
    enfants(tree, "Spec")
        .nth(actif - 1)
        .or_else(|| enfants(tree, "Spec").next())
}

/// Passifs du spec actif.
fn lire_arbre(racine: roxmltree::Node) -> (Vec<u32>, Option<String>) {
    let Some(spec) = spec_actif(racine) else {
        return (Vec::new(), None);
    };
    let passifs = spec
        .attribute("nodes")
        .unwrap_or("")
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect();
    (passifs, spec.attribute("treeVersion").map(str::to_string))
}

/// Texte d'un <Item> : le premier nœud texte ; les <ModRange> suivent.
fn texte_item(item: roxmltree::Node) -> String {
    let texte: String = item
        .children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect::<Vec<_>>()
        .join("\n");
    texte
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Objets équipés dans le set d'objets actif (emplacements vides ignorés),
/// plus les joyaux sertis dans l'arbre (« Jewel <nœud> »).
fn lire_objets(racine: roxmltree::Node, passifs: &[u32]) -> Vec<ObjetPob> {
    let Some(items) = enfants(racine, "Items").next() else {
        return Vec::new();
    };
    let item = |id: &str| {
        enfants(items, "Item")
            .find(|i| i.attribute("id") == Some(id))
            .map(texte_item)
    };
    let set_actif = items.attribute("activeItemSet").unwrap_or("1");
    let set = enfants(items, "ItemSet")
        .find(|s| s.attribute("id") == Some(set_actif))
        .or_else(|| enfants(items, "ItemSet").next());
    let mut objets: Vec<ObjetPob> = set
        .into_iter()
        .flat_map(|set| enfants(set, "Slot"))
        .filter_map(|slot| {
            let id = slot.attribute("itemId").filter(|i| *i != "0")?;
            let emplacement = slot.attribute("name")?.to_string();
            Some(entete_objet(emplacement, item(id)?))
        })
        .collect();
    if let Some(spec) = spec_actif(racine) {
        for socket in enfants(spec, "Sockets").flat_map(|s| enfants(s, "Socket")) {
            let (Some(noeud), Some(id)) = (attr_u32(socket, "nodeId"), socket.attribute("itemId"))
            else {
                continue;
            };
            if id == "0" || !passifs.contains(&noeud) {
                continue;
            }
            if let Some(texte) = item(id) {
                objets.push(entete_objet(format!("Jewel {noeud}"), texte));
            }
        }
    }
    objets.sort_by(|a, b| a.emplacement.cmp(&b.emplacement));
    objets
}

fn entete_objet(emplacement: String, texte: String) -> ObjetPob {
    let mut lignes = texte.lines();
    let mut rarete = None;
    let mut premiere = lignes.next();
    if let Some(r) = premiere.and_then(|l| l.strip_prefix("Rarity:")) {
        rarete = Some(r.trim().to_uppercase());
        premiere = lignes.next();
    }
    let nom = premiere.map(str::to_string);
    // Rares et uniques : nom puis base ; normaux et magiques : base seule.
    let base = match rarete.as_deref() {
        Some("RARE") | Some("UNIQUE") | Some("RELIC") => lignes.next().map(str::to_string),
        _ => None,
    };
    ObjetPob {
        emplacement,
        rarete,
        nom,
        base,
        texte,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding2>
  <Build level="92" className="Witch" ascendClassName="Infernalist" mainSocketGroup="2" targetVersion="0_1">
    <PlayerStat stat="TotalDPS" value="1234"/>
  </Build>
  <Tree activeSpec="2">
    <Spec nodes="1,2,3" treeVersion="0_3"/>
    <Spec nodes="47359, 18348,29240" treeVersion="0_4">
      <Sockets><Socket nodeId="18348" itemId="3"/><Socket nodeId="999" itemId="2"/></Sockets>
    </Spec>
  </Tree>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill enabled="true" mainActiveSkill="1">
        <Gem nameSpec="Frost Wall" skillId="FrostWallPlayer" gemId="Metadata/Items/Gems/SkillGemFrostWall" level="20" quality="20" enabled="true"/>
      </Skill>
      <Skill enabled="true" mainActiveSkill="1" slot="">
        <Gem nameSpec="Arc" skillId="ArcPlayer" gemId="Metadata/Items/Gems/SkillGemArc" level="21" quality="20" enabled="true"/>
        <Gem nameSpec="Zenith II" skillId="SupportZenithPlayerTwo" gemId="Metadata/Items/Gem/SupportGemZenithTwo" level="1" quality="0" enabled="true"/>
        <Gem nameSpec="Pinpoint Critical" skillId="SupportPinpointCriticalPlayer" gemId="Metadata/Items/Gems/SupportGemPinpointCritical" level="1" quality="0" enabled="false"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Items activeItemSet="1">
    <Item id="1">
      Rarity: RARE
Vengeance Song
Dueling Wand
Item Level: 82
Quality: 20
Implicits: 0
104% increased Lightning Damage
52% increased Critical Hit Chance for Spells
      <ModRange range="0.5" id="1"/>
    </Item>
    <Item id="2">
      Rarity: MAGIC
Sapphire Ring of the Hare
+20% to Cold Resistance
    </Item>
    <Item id="3">
      Rarity: RARE
Storm Eye
Sapphire
Implicits: 0
12% increased Lightning Damage
    </Item>
    <ItemSet id="1">
      <Slot name="Weapon 1" itemId="1"/>
      <Slot name="Ring 1" itemId="2"/>
      <Slot name="Ring 2" itemId="0"/>
    </ItemSet>
  </Items>
</PathOfBuilding2>"#;

    #[test]
    fn aller_retour_code() {
        let code = encoder(XML);
        assert_eq!(decoder(&code).unwrap(), XML);
        // Copier-coller avec retours à la ligne et sans remplissage.
        let coupe = format!("  {}\n{}  ", &code[..10], code[10..].trim_end_matches('='));
        assert_eq!(decoder(&coupe).unwrap(), XML);
    }

    #[test]
    fn lit_le_build() {
        let b = importer(&encoder(XML)).unwrap();
        assert_eq!(b.niveau, 92);
        assert_eq!(b.classe_affichee(), "Infernalist");
        assert_eq!(b.passifs, vec![47359, 18348, 29240]);
        assert_eq!(b.version_arbre.as_deref(), Some("0_4"));
        let arc = b.competence_principale().unwrap();
        assert_eq!((arc.nom.as_str(), arc.niveau, arc.qualite), ("Arc", 21, 20));
        let supports: Vec<_> = b
            .supports_principaux()
            .iter()
            .map(|g| g.nom.as_str())
            .collect();
        assert_eq!(supports, vec!["Zenith II"]);
        assert_eq!(b.objets.len(), 3);
        assert_eq!(
            b.objet("Jewel 18348").unwrap().nom.as_deref(),
            Some("Storm Eye")
        );
        let arme = b.objet("Weapon 1").unwrap();
        assert_eq!(arme.nom.as_deref(), Some("Vengeance Song"));
        assert_eq!(arme.base.as_deref(), Some("Dueling Wand"));
        assert!(
            arme.texte
                .ends_with("52% increased Critical Hit Chance for Spells")
        );
        assert!(!arme.texte.contains("ModRange"));
        let bague = b.objet("Ring 1").unwrap();
        assert_eq!(bague.rarete.as_deref(), Some("MAGIC"));
        assert_eq!(bague.base, None);
    }

    #[test]
    fn refuse_les_entrees_invalides() {
        assert_eq!(decoder("  "), Err(ErreurPob::Vide));
        assert_eq!(decoder("https://pobb.in/abc"), Err(ErreurPob::Lien));
        assert_eq!(decoder("pas du base64 !"), Err(ErreurPob::Base64));
        assert_eq!(decoder("aGVsbG8"), Err(ErreurPob::Compression));
        assert_eq!(
            importer(&encoder("<PathOfBuilding><Build/></PathOfBuilding>")),
            Err(ErreurPob::PoE1)
        );
        assert_eq!(
            importer(&encoder("<PathOfBuilding2/>")),
            Err(ErreurPob::SectionManquante("Build"))
        );
        assert!(matches!(importer(&encoder("<a")), Err(ErreurPob::Xml(_))));
    }

    #[test]
    fn borne_le_xml_decompresse() {
        let enorme = "a".repeat(TAILLE_MAX_XML as usize + 10);
        assert_eq!(decoder(&encoder(&enorme)), Err(ErreurPob::TropGros));
    }

    #[test]
    fn build_sans_competences_ni_objets() {
        let b = importer(&encoder(
            r#"<PathOfBuilding2><Build level="1" className="Monk"/></PathOfBuilding2>"#,
        ))
        .unwrap();
        assert_eq!(b.competence_principale(), None);
        assert!(b.objets.is_empty() && b.passifs.is_empty());
        assert_eq!(b.classe_affichee(), "Monk");
    }
}
