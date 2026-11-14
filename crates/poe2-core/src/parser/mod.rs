//! Parseur d'objets : texte copié en jeu avec Ctrl+C (simple) ou
//! Ctrl+Alt+C (avancé, avec en-têtes de mods, paliers et plages).
//!
//! Le texte du presse-papier est une **entrée non fiable** : n'importe quel
//! texte peut y arriver. Le parseur ne panique jamais, refuse les textes de
//! plus de [`TAILLE_MAX`] octets et renvoie une erreur explicite quand le
//! texte n'est pas un objet.
//!
//! Seul le client anglais est pris en charge au sprint 1. Le texte des mods
//! est conservé tel quel avec un **modèle** (`+#% to Fire Resistance`) qui
//! servira de clé vers les `stat_id` une fois les descriptions de stats
//! extraites des données du jeu.

mod valeurs;

use serde::Serialize;
use thiserror::Error;

pub use valeurs::{ValeursMod, extraire_valeurs};

/// Taille maximale acceptée : un objet réel fait moins de 4 Ko.
pub const TAILLE_MAX: usize = 16 * 1024;

const SEPARATEUR: &str = "--------";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ErreurParse {
    #[error("texte vide")]
    Vide,
    #[error("texte trop long ({0} octets) : ce n'est pas un objet")]
    TropLong(usize),
    #[error(
        "client du jeu dans une autre langue que l'anglais : non pris en charge pour l'instant"
    )]
    LangueNonPriseEnCharge,
    #[error("ce texte n'est pas un objet de Path of Exile (ligne « Rarity: » absente)")]
    PasUnObjet,
    #[error("rareté inconnue : « {0} »")]
    RareteInconnue(String),
    #[error("nom de l'objet absent")]
    NomAbsent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rarete {
    Normal,
    Magique,
    Rare,
    Unique,
    Devise,
    Gemme,
    /// Carte de divination (PoE 1).
    CarteDivination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenreMod {
    Implicite,
    Explicite,
    Rune,
    Enchantement,
    Artisanal,
    Desacralise,
    Fracture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Affixe {
    Prefixe,
    Suffixe,
    Implicite,
    Unique,
    Autre,
}

/// En-tête d'un mod en mode avancé :
/// `{ Prefix Modifier "Heated" (Tier: 7) — Damage, Fire }`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnteteMod {
    pub affixe: Affixe,
    pub nom: Option<String>,
    pub palier: Option<u32>,
    pub tags: Vec<String>,
    /// En-tête brut, pour diagnostic.
    pub brut: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModObjet {
    pub genre: GenreMod,
    /// Texte du mod sans les plages ni le suffixe de genre.
    pub texte: String,
    /// Texte avec chaque nombre remplacé par `#` : clé vers les `stat_id`.
    pub modele: String,
    pub valeurs: Vec<f64>,
    /// Plage de roll de chaque valeur (mode avancé uniquement).
    pub plages: Vec<Option<(f64, f64)>>,
    pub entete: Option<EnteteMod>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Propriete {
    pub nom: String,
    pub valeur: String,
    /// Valeur modifiée par les mods de l'objet (`(augmented)`).
    pub augmentee: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Exigences {
    pub niveau: Option<u32>,
    pub force: Option<u32>,
    pub dexterite: Option<u32>,
    pub intelligence: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Objet {
    pub classe: Option<String>,
    pub rarete: Rarete,
    /// Nom propre (rares et uniques).
    pub nom: Option<String>,
    /// Type de base (ou nom complet pour les objets magiques).
    pub base: String,
    pub qualite: Option<i32>,
    pub niveau_objet: Option<u32>,
    pub exigences: Exigences,
    pub proprietes: Vec<Propriete>,
    pub emplacements: Option<String>,
    pub taille_pile: Option<(u32, u32)>,
    pub mods: Vec<ModObjet>,
    pub corrompu: bool,
    pub non_identifie: bool,
    pub miroir: bool,
    pub sanctifie: bool,
    /// Influences et marques de PoE 1 (« Shaper Item », « Fractured Item »…).
    pub influences: Vec<String>,
    pub texte_saveur: Option<String>,
    pub note: Option<String>,
    /// Texte copié avec Ctrl+Alt+C.
    pub avance: bool,
    /// Lignes non reconnues, conservées pour améliorer le parseur.
    pub lignes_inconnues: Vec<String>,
}

impl Objet {
    pub fn mods_de_genre(&self, genre: GenreMod) -> impl Iterator<Item = &ModObjet> {
        self.mods.iter().filter(move |m| m.genre == genre)
    }

    pub fn propriete(&self, nom: &str) -> Option<&Propriete> {
        self.proprietes.iter().find(|p| p.nom == nom)
    }
}

const SUFFIXES_GENRE: [(&str, GenreMod); 6] = [
    ("(implicit)", GenreMod::Implicite),
    ("(rune)", GenreMod::Rune),
    ("(enchant)", GenreMod::Enchantement),
    ("(crafted)", GenreMod::Artisanal),
    ("(desecrated)", GenreMod::Desacralise),
    ("(fractured)", GenreMod::Fracture),
];

/// Qualificatifs d'en-tête avancé qui donnent le genre du mod. Seul
/// « Desecrated » est vérifié sur un vrai objet ; « Fractured » est supposé
/// sur le même modèle, « Master Crafted » vient du format de PoE 1.
const QUALIFICATIFS_ENTETE: [(&str, GenreMod); 3] = [
    ("Desecrated", GenreMod::Desacralise),
    ("Fractured", GenreMod::Fracture),
    ("Master Crafted", GenreMod::Artisanal),
];

/// Lignes d'influence et de marque des objets de PoE 1, seules dans leur section.
const INFLUENCES: [&str; 10] = [
    "Shaper Item",
    "Elder Item",
    "Crusader Item",
    "Hunter Item",
    "Redeemer Item",
    "Warlord Item",
    "Searing Exarch Item",
    "Eater of Worlds Item",
    "Synthesised Item",
    "Fractured Item",
];

/// Propriétés connues des blocs de propriétés (armes, armures, gemmes…),
/// celles de PoE 2 puis celles propres à PoE 1.
const PROPRIETES: [&str; 37] = [
    "Quality",
    "Physical Damage",
    "Elemental Damage",
    "Fire Damage",
    "Cold Damage",
    "Lightning Damage",
    "Chaos Damage",
    "Critical Hit Chance",
    "Attacks per Second",
    "Reload Time",
    "Armour",
    "Evasion Rating",
    "Energy Shield",
    "Block chance",
    "Spirit",
    "Charm Slots",
    "Limited to",
    "Radius",
    "Level",
    "Cost",
    "Cast Time",
    "Waystone Tier",
    // PoE 1
    "Critical Strike Chance",
    "Chance to Block",
    "Ward",
    "Weapon Range",
    "Map Tier",
    "Atlas Region",
    "Item Quantity",
    "Item Rarity",
    "Monster Pack Size",
    "More Scarabs",
    "More Currency",
    "Mana Cost",
    "Mana Reserved",
    "Cooldown Time",
    "Experience",
];

fn rarete(s: &str) -> Result<Rarete, ErreurParse> {
    Ok(match s.trim() {
        "Normal" => Rarete::Normal,
        "Magic" => Rarete::Magique,
        "Rare" => Rarete::Rare,
        "Unique" => Rarete::Unique,
        "Currency" => Rarete::Devise,
        "Gem" => Rarete::Gemme,
        "Divination Card" => Rarete::CarteDivination,
        autre => {
            return Err(ErreurParse::RareteInconnue(
                autre.chars().take(40).collect(),
            ));
        }
    })
}

fn entier(s: &str) -> Option<u32> {
    let chiffres: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    chiffres.parse().ok()
}

fn enlever_suffixe<'a>(ligne: &'a str, suffixe: &str) -> Option<&'a str> {
    ligne.strip_suffix(suffixe).map(str::trim_end)
}

/// `Requires: Level 52, 89 Int` (format PoE 2) ou bloc `Requirements:`.
/// Les marques `(unmet)` et `(augmented)` sont ignorées.
fn lire_exigences(lignes: &[&str], ex: &mut Exigences) {
    let mut morceaux: Vec<String> = Vec::new();
    for l in lignes {
        let l = l.trim();
        if let Some(r) = l.strip_prefix("Requires:") {
            morceaux.extend(r.split(',').map(|s| s.trim().to_string()));
        } else if l != "Requirements:" {
            morceaux.push(l.to_string());
        }
    }
    for m in morceaux {
        // Un vrai objet porte « 39 (augmented) Str » quand l'exigence est
        // modifiée par ses mods (vu sur des objets copiés en jeu).
        let m = m
            .replace("(unmet)", "")
            .replace("(augmented)", "")
            .replace(':', "");
        let m = m.split_whitespace().collect::<Vec<_>>().join(" ");
        let m = m.as_str();
        let (tete, queue) = m.split_once(' ').unwrap_or((m, ""));
        // « Level 52 » ou « 89 Int ».
        match (tete, queue.trim()) {
            ("Level", v) => ex.niveau = entier(v),
            ("Str", v) => ex.force = entier(v),
            ("Dex", v) => ex.dexterite = entier(v),
            ("Int", v) => ex.intelligence = entier(v),
            (v, "Str") => ex.force = entier(v),
            (v, "Dex") => ex.dexterite = entier(v),
            (v, "Int") => ex.intelligence = entier(v),
            _ => {}
        }
    }
}

/// Lit un en-tête avancé `{ … }`.
fn lire_entete(ligne: &str) -> Option<EnteteMod> {
    let interieur = ligne.trim().strip_prefix('{')?.strip_suffix('}')?.trim();
    let (gauche, tags) = match interieur.split_once('—') {
        Some((g, t)) => (
            g.trim(),
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        ),
        None => (interieur, Vec::new()),
    };
    // Qualificatif devant le type d'affixe, vu sur un vrai objet :
    // `{ Desecrated Suffix Modifier "of Magma" (Tier: 2) — … }`.
    let qualifie = QUALIFICATIFS_ENTETE
        .iter()
        .find_map(|(q, _)| gauche.strip_prefix(q))
        .map(str::trim_start)
        .unwrap_or(gauche);
    // PoE 1 met d'autres mots devant le type (« Master Crafted Prefix », « Shaper's
    // Suffix », « Eldritch Implicit ») : on cherche le type avant le nom entre guillemets.
    let avant_nom = qualifie.split('"').next().unwrap_or(qualifie);
    let affixe = if qualifie.starts_with("Prefix") || avant_nom.contains("Prefix Modifier") {
        Affixe::Prefixe
    } else if qualifie.starts_with("Suffix") || avant_nom.contains("Suffix Modifier") {
        Affixe::Suffixe
    } else if gauche.starts_with("Implicit") || avant_nom.contains("Implicit Modifier") {
        Affixe::Implicite
    } else if gauche.starts_with("Unique") {
        Affixe::Unique
    } else {
        Affixe::Autre
    };
    let nom = gauche
        .split('"')
        .nth(1)
        .map(str::to_string)
        .filter(|s| !s.is_empty());
    let palier = gauche
        .find("Tier:")
        .and_then(|i| gauche[i + 5..].split(')').next())
        .and_then(entier);
    Some(EnteteMod {
        affixe,
        nom,
        palier,
        tags,
        brut: ligne.trim().to_string(),
    })
}

fn lire_mod(ligne: &str, entete: Option<&EnteteMod>) -> ModObjet {
    let mut texte = ligne.trim();
    let mut genre = GenreMod::Explicite;
    for (suffixe, g) in SUFFIXES_GENRE {
        if let Some(t) = enlever_suffixe(texte, suffixe) {
            texte = t;
            genre = g;
            break;
        }
    }
    if let Some(t) = texte.split(" — ").next() {
        // « — Unscalable Value » et autres annotations avancées.
        texte = t.trim_end();
    }
    if genre == GenreMod::Explicite {
        if let Some(e) = entete {
            if e.affixe == Affixe::Implicite {
                genre = GenreMod::Implicite;
            } else if let Some((_, g)) = QUALIFICATIFS_ENTETE
                .iter()
                .find(|(q, _)| e.brut.trim_start_matches('{').trim_start().starts_with(q))
            {
                genre = *g;
            }
        }
    }
    let v = extraire_valeurs(texte);
    ModObjet {
        genre,
        texte: v.texte,
        modele: v.modele,
        valeurs: v.valeurs,
        plages: v.plages,
        entete: entete.cloned(),
    }
}

fn est_drapeau(l: &str) -> bool {
    matches!(
        l,
        "Corrupted" | "Unidentified" | "Mirrored" | "Sanctified" | "Split"
    )
}

fn est_ligne_propriete(l: &str) -> bool {
    match l.split_once(':') {
        // « Quality (Attack Modifiers): +20% » : qualité d'un catalyseur (PoE 1).
        Some((nom, _)) => PROPRIETES.contains(&nom.trim()) || nom.trim().starts_with("Quality ("),
        None => false,
    }
}

/// Parse le texte d'un objet copié en jeu.
pub fn parser(texte: &str) -> Result<Objet, ErreurParse> {
    if texte.len() > TAILLE_MAX {
        return Err(ErreurParse::TropLong(texte.len()));
    }
    let texte = texte.trim_start_matches('\u{feff}');
    if texte.trim().is_empty() {
        return Err(ErreurParse::Vide);
    }
    if texte.contains("Rareté") || texte.contains("Seltenheit") || texte.contains("Raridade") {
        return Err(ErreurParse::LangueNonPriseEnCharge);
    }

    let lignes: Vec<&str> = texte
        .lines()
        .map(|l| l.trim_end_matches('\r').trim_end())
        .collect();
    let sections: Vec<Vec<&str>> = lignes
        .split(|l| l.trim() == SEPARATEUR)
        .map(|s| {
            s.iter()
                .copied()
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|s| !s.is_empty())
        .collect();

    // Section d'en-tête : Item Class, Rarity, nom, base.
    let tete = sections.first().ok_or(ErreurParse::PasUnObjet)?;
    let mut classe = None;
    let mut rar = None;
    let mut noms = Vec::new();
    for l in tete {
        if let Some(v) = l.strip_prefix("Item Class:") {
            classe = Some(v.trim().to_string());
        } else if let Some(v) = l.strip_prefix("Rarity:") {
            rar = Some(rarete(v)?);
        } else {
            noms.push(l.trim().to_string());
        }
    }
    let rarete = rar.ok_or(ErreurParse::PasUnObjet)?;
    let (nom, base) = match (rarete, noms.len()) {
        (_, 0) => return Err(ErreurParse::NomAbsent),
        (Rarete::Rare | Rarete::Unique, n) if n >= 2 => (Some(noms[0].clone()), noms[1].clone()),
        _ => (None, noms[0].clone()),
    };

    let mut o = Objet {
        classe,
        rarete,
        nom,
        base,
        qualite: None,
        niveau_objet: None,
        exigences: Exigences::default(),
        proprietes: Vec::new(),
        emplacements: None,
        taille_pile: None,
        mods: Vec::new(),
        corrompu: false,
        non_identifie: false,
        miroir: false,
        sanctifie: false,
        influences: Vec::new(),
        texte_saveur: None,
        note: None,
        avance: lignes
            .iter()
            .any(|l| l.trim_start().starts_with('{') && l.trim_end().ends_with('}')),
        lignes_inconnues: Vec::new(),
    };

    // Sections de texte libre (mods ou texte d'ambiance), traitées après.
    let mut sections_mods: Vec<&Vec<&str>> = Vec::new();
    for s in sections.iter().skip(1) {
        let premiere = s[0].trim();
        if premiere.starts_with("Requires") || premiere == "Requirements:" {
            lire_exigences(s, &mut o.exigences);
        } else if let Some(v) = premiere.strip_prefix("Item Level:") {
            o.niveau_objet = entier(v);
        } else if let Some(v) = premiere.strip_prefix("Sockets:") {
            o.emplacements = Some(v.trim().to_string());
        } else if let Some(v) = premiere.strip_prefix("Stack Size:") {
            let mut it = v.trim().split('/').map(entier);
            if let (Some(Some(a)), Some(Some(b))) = (it.next(), it.next()) {
                o.taille_pile = Some((a, b));
            }
        } else if let Some(v) = premiere.strip_prefix("Note:") {
            o.note = Some(v.trim().to_string());
        } else if s.iter().all(|l| INFLUENCES.contains(&l.trim())) {
            o.influences.extend(s.iter().map(|l| l.trim().to_string()));
        } else if s.iter().all(|l| est_drapeau(l.trim())) {
            for l in s {
                match l.trim() {
                    "Corrupted" => o.corrompu = true,
                    "Unidentified" => o.non_identifie = true,
                    "Mirrored" => o.miroir = true,
                    "Sanctified" => o.sanctifie = true,
                    _ => {}
                }
            }
        } else if est_ligne_propriete(premiere)
            || (s.len() > 1 && !premiere.contains(':') && est_ligne_propriete(s[1].trim()))
        {
            // Bloc de propriétés, éventuellement précédé d'une ligne de
            // catégorie (« Quarterstaff », « Spell, AoE, Fire »).
            for l in s {
                let l = l.trim();
                let Some((nom, valeur)) = l.split_once(':') else {
                    continue;
                };
                let augmentee = valeur.contains("(augmented)");
                let valeur = valeur.replace("(augmented)", "").trim().to_string();
                if nom == "Quality" || nom.starts_with("Quality (") {
                    o.qualite = valeur
                        .trim_start_matches('+')
                        .trim_end_matches('%')
                        .parse()
                        .ok();
                }
                o.proprietes.push(Propriete {
                    nom: nom.trim().to_string(),
                    valeur,
                    augmentee,
                });
            }
        } else {
            sections_mods.push(s);
        }
    }

    // Texte d'ambiance des uniques : dernière section libre sans aucun
    // chiffre ni en-tête avancé. Heuristique, documentée dans le corpus.
    if o.rarete == Rarete::Unique
        && let Some(derniere) = sections_mods.last()
        && sections_mods.len() > 1
        && derniere
            .iter()
            .all(|l| !l.chars().any(|c| c.is_ascii_digit()) && !l.trim_start().starts_with('{'))
    {
        let saveur = derniere
            .iter()
            .map(|l| l.trim())
            .collect::<Vec<_>>()
            .join("\n");
        o.texte_saveur = Some(saveur);
        sections_mods.pop();
    }

    for s in sections_mods {
        let mut entete: Option<EnteteMod> = None;
        for l in s {
            let lt = l.trim();
            if lt.starts_with('{') {
                match lire_entete(lt) {
                    Some(e) => entete = Some(e),
                    None => o.lignes_inconnues.push(lt.to_string()),
                }
                continue;
            }
            if lt.starts_with('(') && lt.ends_with(')') {
                // Texte de rappel (« (Bleeding deals … ) ») : pas un mod.
                continue;
            }
            o.mods.push(lire_mod(lt, entete.as_ref()));
        }
    }
    Ok(o)
}
