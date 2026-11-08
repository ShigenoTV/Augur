//! Suivi d'un craft en jeu : l'objet copié avec Ctrl+Alt+C est rapproché des affixes du pool,
//! projeté sur l'état abstrait du solveur, et le plan actif donne la prochaine étape.
//!
//! Rapprochement d'un mod : côté (préfixe / suffixe) et nom d'affixe de l'en-tête avancé
//! (`{ Prefix Modifier "Athlete's" (Tier: 1) — Life }`), puis texte du mod sans les nombres en cas
//! d'homonymes, puis tier. Un mod qu'on ne retrouve pas dans le pool (mod sans poids connu, mod
//! désacralisé ou artisanal) compte comme un mauvais mod de son côté : il prend une place sans
//! servir l'objectif.

use super::moteur::{AffixIdx, ItemState, Mod, Rarity, Slot};
use super::solveur::{Advice, Model, Solution, advise};
use crate::parser::{Affixe, GenreMod, ModObjet, Objet, Rarete};
use serde::Serialize;

/// Plan calculé dans le panneau Forge, gardé pour l'overlay.
pub struct PlanActif {
    pub base_id: String,
    pub base_nom: String,
    pub ilvl: u8,
    /// Libellé de chaque affixe voulu, dans l'ordre de l'objectif.
    pub labels: Vec<String>,
    pub expected_cost: f64,
    pub model: Model,
    pub sol: Solution,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErreurSuivi {
    #[error("objet d'une autre base que le plan ({0})")]
    AutreBase(String),
    #[error(
        "copie l'objet avec Ctrl+Alt+C : la copie simple ne dit pas quels mods sont des préfixes"
    )]
    CopieSimple,
    #[error("rareté non suivie par Forge (objet normal, magique ou rare seulement)")]
    Rarete,
    #[error("objet corrompu : plus aucune monnaie ne s'applique")]
    Corrompu,
    #[error("état de l'objet hors du plan calculé (trop de mods ?)")]
    HorsPlan,
}

/// Un mod de l'objet tel que Forge l'a compris.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLu {
    pub texte: String,
    pub slot: Slot,
    /// Indice de l'affixe voulu que ce mod remplit (tier accepté).
    pub voulu: Option<u8>,
    /// Mod du même groupe qu'un voulu, mais d'un tier trop bas.
    pub tier_trop_bas: bool,
    /// Retrouvé dans le pool (sinon compté comme mauvais mod).
    pub reconnu: bool,
    pub fracture: bool,
}

/// Ce que l'overlay affiche.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConseilForge {
    pub base_nom: String,
    pub labels: Vec<String>,
    /// Coût espéré du plan entier, depuis une base neuve.
    pub cout_plan: f64,
    pub mods: Vec<ModLu>,
    /// L'objet porte un niveau d'objet plus bas que celui du plan : conseil à prendre avec
    /// prudence (des paliers manquent).
    pub ilvl_objet: Option<u32>,
    pub ilvl_plan: u8,
    pub conseil: Advice,
}

/// Texte d'un mod sans ses nombres ni ses fourchettes, en minuscules : « +# to maximum life ».
pub fn gabarit(texte: &str) -> String {
    let mut out = String::with_capacity(texte.len());
    let mut chars = texte.chars().peekable();
    while let Some(c) = chars.next() {
        let nombre = c.is_ascii_digit()
            || (c == '('
                && chars
                    .peek()
                    .is_some_and(|n| n.is_ascii_digit() || *n == '-'));
        if nombre {
            let mut paren = c == '(';
            while let Some(&n) = chars.peek() {
                if paren {
                    chars.next();
                    if n == ')' {
                        paren = false;
                    }
                } else if n.is_ascii_digit() || n == '.' {
                    chars.next();
                } else if n == '(' {
                    paren = true;
                    chars.next();
                } else {
                    break;
                }
            }
            if !out.ends_with('#') {
                out.push('#');
            }
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Un mod de l'objet : lignes consécutives sous le même en-tête.
struct ModGroupe<'a> {
    slot: Slot,
    nom: Option<&'a str>,
    palier: Option<u32>,
    lignes: Vec<&'a ModObjet>,
}

fn grouper(objet: &Objet) -> Vec<ModGroupe<'_>> {
    let mut groupes: Vec<ModGroupe> = Vec::new();
    let mut dernier: Option<&str> = None;
    for m in &objet.mods {
        let utile = matches!(
            m.genre,
            GenreMod::Explicite | GenreMod::Fracture | GenreMod::Desacralise | GenreMod::Artisanal
        );
        let Some(e) = m.entete.as_ref().filter(|_| utile) else {
            dernier = None;
            continue;
        };
        let slot = match e.affixe {
            Affixe::Prefixe => Slot::Prefix,
            Affixe::Suffixe => Slot::Suffix,
            _ => {
                dernier = None;
                continue;
            }
        };
        if dernier == Some(e.brut.as_str())
            && let Some(g) = groupes.last_mut()
        {
            g.lignes.push(m);
            continue;
        }
        dernier = Some(e.brut.as_str());
        groupes.push(ModGroupe {
            slot,
            nom: e.nom.as_deref(),
            palier: e.palier,
            lignes: vec![m],
        });
    }
    groupes
}

/// Affixe du pool qui correspond à un mod de l'objet.
fn retrouver(model: &Model, g: &ModGroupe) -> Option<AffixIdx> {
    let nom = g.nom?.to_lowercase();
    let lignes: Vec<String> = g.lignes.iter().map(|l| gabarit(&l.texte)).collect();
    let candidats: Vec<(usize, &super::moteur::Affix)> = model
        .pool
        .affixes
        .iter()
        .enumerate()
        .filter(|(_, a)| a.slot == g.slot && a.name.to_lowercase() == nom)
        .collect();
    let meme_texte = |a: &super::moteur::Affix| {
        let t: Vec<String> = a.text.lines().map(gabarit).collect();
        t == lignes
    };
    let choisi = match candidats.len() {
        0 => None,
        1 => Some(candidats[0]),
        _ => {
            let par_texte: Vec<_> = candidats.iter().filter(|(_, a)| meme_texte(a)).collect();
            par_texte
                .iter()
                .find(|(_, a)| g.palier == Some(u32::from(a.tier)))
                .or(par_texte.first())
                .map(|c| **c)
        }
    };
    choisi.and_then(|(i, _)| AffixIdx::try_from(i).ok())
}

/// Prochaine étape du plan actif pour l'objet copié.
pub fn conseiller(actif: &PlanActif, objet: &Objet) -> Result<ConseilForge, ErreurSuivi> {
    let base = objet.base.as_str();
    let meme_base = match objet.rarete {
        // Un objet magique s'affiche avec ses affixes autour du nom de la base.
        Rarete::Magique => base.contains(actif.base_nom.as_str()),
        _ => base == actif.base_nom,
    };
    if !meme_base {
        return Err(ErreurSuivi::AutreBase(actif.base_nom.clone()));
    }
    let rarity = match objet.rarete {
        Rarete::Normal => Rarity::Normal,
        Rarete::Magique => Rarity::Magic,
        Rarete::Rare => Rarity::Rare,
        _ => return Err(ErreurSuivi::Rarete),
    };
    if objet.corrompu {
        return Err(ErreurSuivi::Corrompu);
    }
    let groupes = grouper(objet);
    let explicites = objet
        .mods
        .iter()
        .any(|m| matches!(m.genre, GenreMod::Explicite | GenreMod::Fracture));
    if explicites && !objet.avance {
        return Err(ErreurSuivi::CopieSimple);
    }

    let mut item = ItemState::new(rarity, actif.ilvl);
    let (mut inconnus_p, mut inconnus_s) = (0u8, 0u8);
    let mut mods = Vec::new();
    for g in &groupes {
        let fracture = g.lignes.iter().any(|l| l.genre == GenreMod::Fracture);
        let artisanal = g
            .lignes
            .iter()
            .any(|l| matches!(l.genre, GenreMod::Desacralise | GenreMod::Artisanal));
        let idx = (!artisanal).then(|| retrouver(&actif.model, g)).flatten();
        let texte = g
            .lignes
            .iter()
            .map(|l| l.texte.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        let mut lu = ModLu {
            texte,
            slot: g.slot,
            voulu: None,
            tier_trop_bas: false,
            reconnu: idx.is_some(),
            fracture,
        };
        match idx {
            Some(i) if item.mods().len() < 6 => {
                item.push(Mod {
                    idx: i,
                    fractured: fracture,
                    crafted: false,
                });
                match actif.model.goal.classify(&actif.model.pool, i) {
                    super::objectif::Class::Wanted(k) => lu.voulu = u8::try_from(k).ok(),
                    super::objectif::Class::Blocked(_) => lu.tier_trop_bas = true,
                    super::objectif::Class::Other => {}
                }
            }
            _ => match g.slot {
                Slot::Prefix => inconnus_p += 1,
                Slot::Suffix => inconnus_s += 1,
            },
        }
        mods.push(lu);
    }

    let mut etat = actif.model.project(&item).ok_or(ErreurSuivi::HorsPlan)?;
    etat.bad_p += inconnus_p;
    etat.bad_s += inconnus_s;
    let conseil =
        advise(&actif.model, &actif.sol, &etat, &actif.labels).ok_or(ErreurSuivi::HorsPlan)?;
    Ok(ConseilForge {
        base_nom: actif.base_nom.clone(),
        labels: actif.labels.clone(),
        cout_plan: actif.expected_cost,
        mods,
        ilvl_objet: objet.niveau_objet,
        ilvl_plan: actif.ilvl,
        conseil,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gabarits() {
        assert_eq!(gabarit("+136 to maximum Life"), "+# to maximum life");
        assert_eq!(gabarit("+(120-149) to maximum Life"), "+# to maximum life");
        assert_eq!(
            gabarit("Adds (1-2) to (4-6) Lightning damage to Attacks"),
            "adds # to # lightning damage to attacks"
        );
        assert_eq!(
            gabarit("Adds 1 to 4 Lightning damage to Attacks"),
            "adds # to # lightning damage to attacks"
        );
        assert_eq!(
            gabarit("+(41-45)% to Fire Resistance"),
            "+#% to fire resistance"
        );
        assert_eq!(
            gabarit("(5-7)% increased Attack Speed"),
            "#% increased attack speed"
        );
        assert_eq!(
            gabarit("Recover 1.5% of maximum Life"),
            "recover #% of maximum life"
        );
    }
}
