//! Pool d'affixes d'une base, construit à partir des données d'Augur :
//!
//! - **éligibilité** et structure (groupes d'exclusion, familles, niveaux) : tables RePoE
//!   produites par `tools/gamedata/import_repoe.py` (`tables/mods.json`, `tables/bases.json`) ;
//! - **poids** : relevé poe2db de `tools/theorycraft/poids-poe2db.json` (estimations de
//!   Krakenbul, CC BY-NC-SA 3.0 : l'attribution suit ces données partout).
//!
//! Règle du cadrage (`docs/forge/cadrage.md`) : **jamais de poids inventé**. Un mod éligible
//! sans poids mesuré n'entre pas dans le pool ; il est listé dans [`PoolBase::sans_poids`] et
//! toute probabilité calculée sur ce pool est alors incomplète ([`PoolBase::complet`]).
//!
//! Même règle d'éligibilité que `tools/theorycraft/craft_proba.py` : premier poids RePoE dont
//! l'étiquette est `default` ou portée par la base, non nul ; mod hors Essence ; niveau requis au
//! plus égal au niveau d'objet.

use super::moteur::{Affix, AffixPool, GroupId, Slot};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Version du schéma des tables RePoE comprise par ce module.
pub const SCHEMA_TABLES: u32 = 1;

/// `tables/mods.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct TableMods {
    pub schema: u32,
    pub mods: Vec<ModTable>,
}

/// Un mod (un palier) de `tables/mods.json` ; seuls les champs utiles à Forge sont lus.
#[derive(Clone, Debug, Deserialize)]
pub struct ModTable {
    pub id: String,
    #[serde(default)]
    pub nom: String,
    pub domaine: String,
    pub slot: String,
    pub groupe: String,
    #[serde(default)]
    pub groupes: Vec<String>,
    #[serde(default, rename = "type")]
    pub famille: String,
    pub niveau: u32,
    #[serde(default)]
    pub texte: String,
    pub spawn: Vec<PoidsEtiquette>,
    #[serde(default)]
    pub essence_seulement: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PoidsEtiquette {
    pub tag: String,
    pub weight: i64,
}

/// `tables/bases.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct TableBases {
    pub schema: u32,
    pub bases: Vec<BaseTable>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BaseTable {
    pub id: String,
    pub nom: String,
    pub classe: String,
    pub tags: Vec<String>,
    /// Niveau d'apparition de la base (absent des anciennes tables).
    #[serde(default)]
    pub niveau_drop: u32,
}

/// Relevé de poids poe2db (`poids-poe2db.json`).
#[derive(Clone, Debug, Deserialize)]
pub struct RelevePoids {
    pub date_releve: String,
    pub licence: String,
    #[serde(default)]
    pub repoe_version: String,
    pub source: String,
    pub pages: BTreeMap<String, PagePoids>,
    /// Poids de Craft of Exile pour les mods sans poids chez poe2db (`poids-coe.json`).
    #[serde(default)]
    pub complement: Option<ComplementPoids>,
}

/// Relevé de Craft of Exile qui complète poe2db (décision de Max du 08/11/2026) : seulement les
/// mods que poe2db n'a pas mesurés. Les poids communs aux deux sources sont identiques.
#[derive(Clone, Debug, Deserialize)]
pub struct ComplementPoids {
    pub source: String,
    pub date_releve: String,
    #[serde(default)]
    pub version_donnees: String,
    pub pages: BTreeMap<String, PagePoids>,
}

impl RelevePoids {
    /// La page porte au moins un poids, chez poe2db ou en complément.
    pub fn page_utilisable(&self, page: &str) -> bool {
        let a_des_poids = |p: Option<&PagePoids>| p.is_some_and(|p| !p.poids.is_empty());
        a_des_poids(self.pages.get(page))
            || a_des_poids(self.complement.as_ref().and_then(|c| c.pages.get(page)))
    }

    /// Page de poids d'une base : sa page propre si une source en a une (ex. `Trarthan_Cannon`,
    /// arbalète à grenades dont les mods diffèrent des autres arbalètes), sinon celle de sa classe.
    pub fn page_de(&self, base: &BaseTable) -> Option<String> {
        let propre = base.nom.replace(' ', "_");
        if self.page_utilisable(&propre) {
            return Some(propre);
        }
        page_poe2db(base)
    }
}

/// Une page poe2db : poids mesurés et mods listés sans poids.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct PagePoids {
    pub poids: BTreeMap<String, u32>,
    #[serde(default)]
    pub inconnus: Vec<String>,
}

/// Mod éligible écarté du pool faute de poids mesuré.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SansPoids {
    pub id: String,
    pub slot: Slot,
    pub niveau: u32,
}

/// Pool d'affixes d'une base à un niveau d'objet, prêt pour le moteur.
#[derive(Clone, Debug)]
pub struct PoolBase {
    pub pool: AffixPool,
    /// Page poe2db dont viennent les poids.
    pub page: String,
    /// Source et licence des poids, à afficher avec toute probabilité.
    pub source_poids: String,
    pub licence_poids: String,
    pub date_releve: String,
    /// Mods éligibles sans poids mesuré (non tirés par le moteur).
    pub sans_poids: Vec<SansPoids>,
    /// Poids pris dans le complément Craft of Exile, et sa source.
    pub poids_complement: usize,
    pub source_complement: Option<String>,
}

impl PoolBase {
    /// Toutes les probabilités du pool normal sont calculables (aucun mod éligible sans poids).
    pub fn complet(&self) -> bool {
        self.sans_poids.is_empty()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErreurPool {
    #[error("schéma de tables {0} non pris en charge (attendu {SCHEMA_TABLES})")]
    Schema(u32),
    #[error("classe d'objet sans page de poids poe2db : {0}")]
    BaseNonSupportee(String),
    #[error("page de poids poe2db absente du relevé : {0}")]
    PageAbsente(String),
    #[error("le mod {0} appartient à plusieurs groupes d'exclusion, non géré par le moteur")]
    GroupesMultiples(String),
    #[error("trop d'affixes ou de groupes pour le moteur (65 535 au plus)")]
    TropGrand,
}

/// Page poe2db d'une base : nom de la classe au pluriel, suivi des attributs de défense pour
/// les armures (`Body_Armours_int`, `Shields_str_dex`…). `None` si poe2db n'a pas de page pour
/// cette base.
pub fn page_poe2db(base: &BaseTable) -> Option<String> {
    let page = match base.classe.as_str() {
        "Amulet" => "Amulets",
        "Belt" => "Belts",
        "Bow" => "Bows",
        "Buckler" => "Bucklers",
        "Claw" => "Claws",
        "Crossbow" => "Crossbows",
        "Dagger" => "Daggers",
        "Flail" => "Flails",
        "Focus" => "Foci",
        "One Hand Axe" => "One_Hand_Axes",
        "One Hand Mace" => "One_Hand_Maces",
        "One Hand Sword" => "One_Hand_Swords",
        "Quiver" => "Quivers",
        "Ring" => "Rings",
        "Sceptre" => "Sceptres",
        "Spear" => "Spears",
        "Staff" => "Staves",
        "Two Hand Axe" => "Two_Hand_Axes",
        "Two Hand Mace" => "Two_Hand_Maces",
        "Two Hand Sword" => "Two_Hand_Swords",
        "Wand" => "Wands",
        "Warstaff" => "Quarterstaves",
        "Body Armour" => return page_armure("Body_Armours", base),
        "Boots" => return page_armure("Boots", base),
        "Gloves" => return page_armure("Gloves", base),
        "Helmet" => return page_armure("Helmets", base),
        "Shield" => return page_armure("Shields", base),
        _ => return None,
    };
    Some(page.to_string())
}

/// Attributs de défense d'une armure, d'après son étiquette `<attributs>_armour`.
fn page_armure(prefixe: &str, base: &BaseTable) -> Option<String> {
    const ATTRIBUTS: [&str; 7] = [
        "str",
        "dex",
        "int",
        "str_dex",
        "str_int",
        "dex_int",
        "str_dex_int",
    ];
    let attr = base
        .tags
        .iter()
        .filter_map(|t| t.strip_suffix("_armour"))
        .find(|a| ATTRIBUTS.contains(a))?;
    Some(format!("{prefixe}_{attr}"))
}

/// Poids RePoE (0 ou 1, éligibilité seulement) : première étiquette `default` ou portée par la
/// base.
fn eligible(m: &ModTable, tags: &[String]) -> bool {
    m.spawn
        .iter()
        .find(|s| s.tag == "default" || tags.contains(&s.tag))
        .is_some_and(|s| s.weight > 0)
}

/// Construit le pool d'une base pour un objet de niveau `ilvl`.
///
/// Seul le pool **normal** (domaine `item`) est construit : les mods désacralisés n'ont pas de
/// poids chez poe2db, la Désécration n'est donc pas encore calculable.
pub fn construire_pool(
    mods: &TableMods,
    base: &BaseTable,
    ilvl: u32,
    releve: &RelevePoids,
) -> Result<PoolBase, ErreurPool> {
    if mods.schema != SCHEMA_TABLES {
        return Err(ErreurPool::Schema(mods.schema));
    }
    let page_nom = releve
        .page_de(base)
        .ok_or_else(|| ErreurPool::BaseNonSupportee(base.nom.clone()))?;
    let vide = PagePoids::default();
    let complement = releve
        .complement
        .as_ref()
        .and_then(|c| c.pages.get(&page_nom));
    let page = match (releve.pages.get(&page_nom), complement) {
        (Some(p), _) => p,
        (None, Some(_)) => &vide,
        (None, None) => return Err(ErreurPool::PageAbsente(page_nom.clone())),
    };
    let mut poids_complement = 0;

    // Mods éligibles sur la base, tous niveaux : les tiers se numérotent sur la famille entière
    // (T1 = niveau le plus haut), puis le niveau d'objet filtre.
    let candidats: Vec<&ModTable> = mods
        .mods
        .iter()
        .filter(|m| m.domaine == "item" && !m.essence_seulement && eligible(m, &base.tags))
        .collect();

    // groupe -> famille -> paliers, ordre déterministe (BTreeMap).
    let mut par_groupe: BTreeMap<&str, BTreeMap<&str, Vec<&ModTable>>> = BTreeMap::new();
    for m in &candidats {
        par_groupe
            .entry(m.groupe.as_str())
            .or_default()
            .entry(m.famille.as_str())
            .or_default()
            .push(m);
    }

    let mut affixes = Vec::new();
    let mut sans_poids = Vec::new();
    for (gi, familles) in par_groupe.values().enumerate() {
        let group = GroupId::try_from(gi).map_err(|_| ErreurPool::TropGrand)?;
        for (fi, paliers) in familles.values().enumerate() {
            let family_id = u16::try_from(fi).map_err(|_| ErreurPool::TropGrand)?;
            let mut paliers = paliers.clone();
            paliers.sort_by(|a, b| b.niveau.cmp(&a.niveau).then_with(|| a.id.cmp(&b.id)));
            for (ti, m) in paliers.iter().enumerate() {
                if m.niveau > ilvl {
                    continue;
                }
                let slot = match m.slot.as_str() {
                    "prefix" => Slot::Prefix,
                    _ => Slot::Suffix,
                };
                let weight = match (
                    page.poids.get(&m.id),
                    complement.and_then(|c| c.poids.get(&m.id)),
                ) {
                    (Some(&w), _) => Some(w),
                    (None, Some(&w)) => {
                        poids_complement += 1;
                        Some(w)
                    }
                    (None, None) => None,
                };
                let Some(weight) = weight else {
                    sans_poids.push(SansPoids {
                        id: m.id.clone(),
                        slot,
                        niveau: m.niveau,
                    });
                    continue;
                };
                if m.groupes.len() > 1 {
                    return Err(ErreurPool::GroupesMultiples(m.id.clone()));
                }
                affixes.push(Affix {
                    id: m.id.clone(),
                    name: m.nom.clone(),
                    family: m.famille.clone(),
                    text: m.texte.clone(),
                    group,
                    family_id,
                    slot,
                    tier: u8::try_from(ti + 1).unwrap_or(u8::MAX),
                    req_ilvl: u8::try_from(m.niveau).unwrap_or(u8::MAX),
                    weight,
                    tags: 0,
                    desecrated: false,
                    cap_shift: (0, 0),
                });
            }
        }
    }
    if affixes.len() > usize::from(u16::MAX) {
        return Err(ErreurPool::TropGrand);
    }
    sans_poids.sort_by(|a, b| a.id.cmp(&b.id));

    // Le plafond propre à la base (implicites « +1 Prefix Modifier allowed ») n'est pas encore
    // extrait des tables : plafond 3/3 d'un Rare ordinaire.
    Ok(PoolBase {
        pool: AffixPool::new(affixes),
        page: page_nom,
        source_poids: releve.source.clone(),
        licence_poids: releve.licence.clone(),
        date_releve: releve.date_releve.clone(),
        sans_poids,
        poids_complement,
        source_complement: (poids_complement > 0)
            .then(|| releve.complement.as_ref().map(|c| c.source.clone()))
            .flatten(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::moteur::{DrawFilter, ItemState, Mod, Rarity};

    fn m(id: &str, groupe: &str, slot: &str, niveau: u32, spawn: &[(&str, i64)]) -> ModTable {
        ModTable {
            id: id.into(),
            nom: id.into(),
            domaine: "item".into(),
            slot: slot.into(),
            groupe: groupe.into(),
            groupes: vec![groupe.into()],
            famille: groupe.into(),
            niveau,
            texte: String::new(),
            spawn: spawn
                .iter()
                .map(|(t, w)| PoidsEtiquette {
                    tag: (*t).into(),
                    weight: *w,
                })
                .collect(),
            essence_seulement: false,
        }
    }

    fn robe() -> BaseTable {
        BaseTable {
            id: "Metadata/Robe".into(),
            nom: "Robe".into(),
            classe: "Body Armour".into(),
            tags: vec![
                "int_armour".into(),
                "body_armour".into(),
                "armour".into(),
                "default".into(),
            ],
            niveau_drop: 1,
        }
    }

    fn releve(poids: &[(&str, u32)], inconnus: &[&str]) -> RelevePoids {
        let page = PagePoids {
            poids: poids.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
            inconnus: inconnus.iter().map(|s| (*s).into()).collect(),
        };
        RelevePoids {
            date_releve: "2026-11-05".into(),
            licence: "CC BY-NC-SA 3.0".into(),
            repoe_version: String::new(),
            source: "poe2db".into(),
            pages: [("Body_Armours_int".to_string(), page)].into(),
            complement: None,
        }
    }

    fn complement(page: &str, poids: &[(&str, u32)]) -> ComplementPoids {
        ComplementPoids {
            source: "Craft of Exile".into(),
            date_releve: "2026-11-08".into(),
            version_donnees: "0.5.5.5".into(),
            pages: [(
                page.to_string(),
                PagePoids {
                    poids: poids.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
                    inconnus: vec![],
                },
            )]
            .into(),
        }
    }

    #[test]
    fn complement_craft_of_exile() {
        let mods = TableMods {
            schema: 1,
            mods: vec![
                m("Life1", "Life", "prefix", 1, &[("default", 1)]),
                m("Mana1", "Mana", "prefix", 1, &[("default", 1)]),
            ],
        };
        // poe2db prime ; le complément comble le mod sans poids
        let mut r = releve(&[("Life1", 1000)], &["Mana1"]);
        r.complement = Some(complement(
            "Body_Armours_int",
            &[("Life1", 1), ("Mana1", 700)],
        ));
        let pb = construire_pool(&mods, &robe(), 60, &r).unwrap();
        assert!(pb.complet());
        let w = |id: &str| pb.pool.affixes.iter().find(|a| a.id == id).unwrap().weight;
        assert_eq!((w("Life1"), w("Mana1")), (1000, 700));
        assert_eq!(pb.poids_complement, 1);
        assert_eq!(pb.source_complement.as_deref(), Some("Craft of Exile"));

        // page absente chez poe2db mais présente dans le complément
        let mut b = robe();
        b.tags = vec!["str_dex_int_armour".into(), "default".into()];
        let mut r = releve(&[], &[]);
        assert!(!r.page_utilisable("Body_Armours_str_dex_int"));
        r.complement = Some(complement(
            "Body_Armours_str_dex_int",
            &[("Life1", 1000), ("Mana1", 700)],
        ));
        assert!(r.page_utilisable("Body_Armours_str_dex_int"));
        let pb = construire_pool(&mods, &b, 60, &r).unwrap();
        assert_eq!(pb.pool.affixes.len(), 2);
        assert_eq!(pb.poids_complement, 2);
    }

    #[test]
    fn page_selon_classe_et_attributs() {
        assert_eq!(page_poe2db(&robe()).as_deref(), Some("Body_Armours_int"));
        let mut b = robe();
        b.classe = "Shield".into();
        b.tags = vec!["str_dex_armour".into(), "str_dex_shield".into()];
        assert_eq!(page_poe2db(&b).as_deref(), Some("Shields_str_dex"));
        b.tags = vec!["shield".into()];
        assert_eq!(page_poe2db(&b), None, "armure sans attribut de défense");
        b.classe = "Warstaff".into();
        assert_eq!(page_poe2db(&b).as_deref(), Some("Quarterstaves"));
        b.classe = "Jewel".into();
        assert_eq!(page_poe2db(&b), None);
    }

    #[test]
    fn tiers_poids_et_mods_sans_poids() {
        let mods = TableMods {
            schema: 1,
            mods: vec![
                m("Life1", "Life", "prefix", 1, &[("body_armour", 1)]),
                m("Life2", "Life", "prefix", 40, &[("body_armour", 1)]),
                m("Life3", "Life", "prefix", 80, &[("body_armour", 1)]),
                // première étiquette portée par la base : poids 0, jamais éligible
                m(
                    "NoInt",
                    "Str",
                    "suffix",
                    1,
                    &[("int_armour", 0), ("default", 1)],
                ),
                m("Fire1", "Fire", "suffix", 1, &[("default", 1)]),
                m("Mana1", "Mana", "prefix", 1, &[("default", 1)]),
            ],
        };
        let r = releve(
            &[
                ("Life1", 1000),
                ("Life2", 500),
                ("Life3", 100),
                ("Fire1", 800),
            ],
            &["Mana1"],
        );
        let pb = construire_pool(&mods, &robe(), 60, &r).unwrap();
        let ids: Vec<_> = pb.pool.affixes.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            ["Fire1", "Life2", "Life1"],
            "Life3 au-dessus du niveau 60"
        );
        let tier = |id: &str| pb.pool.affixes.iter().find(|a| a.id == id).unwrap().tier;
        assert_eq!(
            (tier("Life2"), tier("Life1")),
            (2, 3),
            "T1 = Life3, même filtré"
        );
        assert_eq!(
            pb.sans_poids,
            [SansPoids {
                id: "Mana1".into(),
                slot: Slot::Prefix,
                niveau: 1
            }]
        );
        assert!(!pb.complet());

        // distribution exacte du prochain mod sur un Rare vide
        let item = ItemState::new(Rarity::Rare, 60);
        let d = pb.pool.distribution(&item, &DrawFilter::default());
        let total = 800.0 + 500.0 + 1000.0;
        assert_eq!(d.len(), 3);
        assert!((d[0].1 - 800.0 / total).abs() < 1e-12);

        // un mod de Vie présent exclut tout son groupe
        let mut item = item;
        item.push(Mod {
            idx: 1,
            fractured: false,
            crafted: false,
        });
        let d = pb.pool.distribution(&item, &DrawFilter::default());
        assert_eq!(d, [(0, 1.0)]);
    }

    #[test]
    fn base_ou_page_non_supportee() {
        let mods = TableMods {
            schema: 1,
            mods: vec![],
        };
        let mut b = robe();
        b.classe = "Jewel".into();
        assert!(matches!(
            construire_pool(&mods, &b, 80, &releve(&[], &[])),
            Err(ErreurPool::BaseNonSupportee(_))
        ));
        b.classe = "Body Armour".into();
        b.tags = vec!["str_armour".into()];
        assert_eq!(
            construire_pool(&mods, &b, 80, &releve(&[], &[])).unwrap_err(),
            ErreurPool::PageAbsente("Body_Armours_str".into())
        );
        let vieux = TableMods {
            schema: 2,
            mods: vec![],
        };
        assert_eq!(
            construire_pool(&vieux, &robe(), 80, &releve(&[], &[])).unwrap_err(),
            ErreurPool::Schema(2)
        );
    }
}
