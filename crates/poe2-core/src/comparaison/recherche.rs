//! Recherche d'upgrades : ce que vaut chaque stat pour le build, dans
//! l'emplacement de l'objet copié, puis une recherche de trade pondérée.
//!
//! On vide l'emplacement (sauf l'arme, qui porte les dégâts des attaques), puis
//! on ajoute une stat à la fois (« +50 to maximum Life », « 30% increased
//! Spell Damage »…) : le gain de DPS et d'EHP qu'elle apporte, divisé par sa
//! valeur, donne son poids (en % de gain par point). La somme pondérée des
//! stats d'un objet estime donc ce qu'il rapporte. Le minimum de la recherche
//! est ce que rapporte l'objet équipé : les résultats font au moins aussi bien.
//!
//! Le DPS et l'EHP comptent autant (1 % de l'un vaut 1 % de l'autre). Quand la
//! compétence n'est pas calculée par le moteur, seule la défense compte.
//!
//! Identifiants de stats du site de trade (`explicit.stat_…`) : relevés dans
//! les données d'Exiled Exchange 2 (licence MIT), voir
//! `docs/copilot/recherche-upgrades.md`.

use super::{
    ErreurComparaison, ObjetCompare, Rapport, bilan_defense, competence_comparee, defense, dps,
    emplacements, modeliser,
};
use crate::pob::BuildPob;

/// Une stat que la recherche sait pondérer.
#[derive(Debug, Clone, Copy)]
pub struct StatTrade {
    /// Identifiant du site de trade.
    pub id: &'static str,
    /// Ligne de mod testée, telle que le moteur la lit.
    pub ligne: &'static str,
    /// Valeur de la ligne testée (le poids est le gain divisé par elle).
    pub valeur: f64,
    /// Libellé affiché (en jeu, en anglais).
    pub libelle: &'static str,
    /// Stat seulement globale hors pièces d'armure (sur une armure, la même
    /// ligne est locale et a un autre identifiant).
    pub hors_armure: bool,
}

const fn stat(
    id: &'static str,
    ligne: &'static str,
    valeur: f64,
    libelle: &'static str,
) -> StatTrade {
    StatTrade {
        id,
        ligne,
        valeur,
        libelle,
        hors_armure: false,
    }
}

/// Stats candidates. Seules celles qui rapportent quelque chose au build
/// entrent dans la recherche.
pub const STATS: &[StatTrade] = &[
    stat(
        "explicit.stat_3299347043",
        "+50 to maximum Life",
        50.0,
        "+# to maximum Life",
    ),
    stat(
        "explicit.stat_983749596",
        "8% increased maximum Life",
        8.0,
        "#% increased maximum Life",
    ),
    StatTrade {
        hors_armure: true,
        ..stat(
            "explicit.stat_3489782002",
            "+40 to maximum Energy Shield",
            40.0,
            "+# to maximum Energy Shield",
        )
    },
    stat(
        "explicit.stat_2482852589",
        "15% increased maximum Energy Shield",
        15.0,
        "#% increased maximum Energy Shield",
    ),
    stat(
        "explicit.stat_3372524247",
        "+20% to Fire Resistance",
        20.0,
        "+#% to Fire Resistance",
    ),
    stat(
        "explicit.stat_4220027924",
        "+20% to Cold Resistance",
        20.0,
        "+#% to Cold Resistance",
    ),
    stat(
        "explicit.stat_1671376347",
        "+20% to Lightning Resistance",
        20.0,
        "+#% to Lightning Resistance",
    ),
    stat(
        "explicit.stat_2923486259",
        "+13% to Chaos Resistance",
        13.0,
        "+#% to Chaos Resistance",
    ),
    stat(
        "explicit.stat_2901986750",
        "+10% to all Elemental Resistances",
        10.0,
        "+#% to all Elemental Resistances",
    ),
    stat(
        "explicit.stat_4080418644",
        "+20 to Strength",
        20.0,
        "+# to Strength",
    ),
    stat(
        "explicit.stat_1379411836",
        "+10 to all Attributes",
        10.0,
        "+# to all Attributes",
    ),
    stat(
        "explicit.stat_2974417149",
        "40% increased Spell Damage",
        40.0,
        "#% increased Spell Damage",
    ),
    stat(
        "explicit.stat_2231156303",
        "30% increased Lightning Damage",
        30.0,
        "#% increased Lightning Damage",
    ),
    stat(
        "explicit.stat_3962278098",
        "30% increased Fire Damage",
        30.0,
        "#% increased Fire Damage",
    ),
    stat(
        "explicit.stat_3291658075",
        "30% increased Cold Damage",
        30.0,
        "#% increased Cold Damage",
    ),
    stat(
        "explicit.stat_736967255",
        "30% increased Chaos Damage",
        30.0,
        "#% increased Chaos Damage",
    ),
    stat(
        "explicit.stat_3141070085",
        "25% increased Elemental Damage",
        25.0,
        "#% increased Elemental Damage",
    ),
    stat(
        "explicit.stat_2843214518",
        "25% increased Attack Damage",
        25.0,
        "#% increased Attack Damage",
    ),
    stat(
        "explicit.stat_1839076647",
        "25% increased Projectile Damage",
        25.0,
        "#% increased Projectile Damage",
    ),
    stat(
        "explicit.stat_1589917703",
        "Minions deal 25% increased Damage",
        25.0,
        "Minions deal #% increased Damage",
    ),
    stat(
        "explicit.stat_2891184298",
        "15% increased Cast Speed",
        15.0,
        "#% increased Cast Speed",
    ),
    StatTrade {
        hors_armure: true,
        ..stat(
            "explicit.stat_681332047",
            "10% increased Attack Speed",
            10.0,
            "#% increased Attack Speed",
        )
    },
    stat(
        "explicit.stat_587431675",
        "30% increased Critical Hit Chance",
        30.0,
        "#% increased Critical Hit Chance",
    ),
    stat(
        "explicit.stat_737908626",
        "30% increased Critical Hit Chance for Spells",
        30.0,
        "#% increased Critical Hit Chance for Spells",
    ),
    stat(
        "explicit.stat_3556824919",
        "25% increased Critical Damage Bonus",
        25.0,
        "#% increased Critical Damage Bonus",
    ),
    stat(
        "explicit.stat_274716455",
        "25% increased Critical Spell Damage Bonus",
        25.0,
        "#% increased Critical Spell Damage Bonus",
    ),
    stat(
        "explicit.stat_124131830",
        "+1 to Level of all Spell Skills",
        1.0,
        "+# to Level of all Spell Skills",
    ),
    stat(
        "explicit.stat_1545858329",
        "+1 to Level of all Lightning Spell Skills",
        1.0,
        "+# to Level of all Lightning Spell Skills",
    ),
    stat(
        "explicit.stat_591105508",
        "+1 to Level of all Fire Spell Skills",
        1.0,
        "+# to Level of all Fire Spell Skills",
    ),
    stat(
        "explicit.stat_2254480358",
        "+1 to Level of all Cold Spell Skills",
        1.0,
        "+# to Level of all Cold Spell Skills",
    ),
    stat(
        "explicit.stat_4226189338",
        "+1 to Level of all Chaos Spell Skills",
        1.0,
        "+# to Level of all Chaos Spell Skills",
    ),
    stat(
        "explicit.stat_2162097452",
        "+1 to Level of all Minion Skills",
        1.0,
        "+# to Level of all Minion Skills",
    ),
    stat(
        "explicit.stat_1202301673",
        "+1 to Level of all Projectile Skills",
        1.0,
        "+# to Level of all Projectile Skills",
    ),
    stat(
        "explicit.stat_9187492",
        "+1 to Level of all Melee Skills",
        1.0,
        "+# to Level of all Melee Skills",
    ),
];

/// Gain minimal (en %) d'une stat testée pour entrer dans la recherche.
const GAIN_MIN_PCT: f64 = 0.1;

/// Poids d'une stat dans la recherche.
#[derive(Debug, Clone, PartialEq)]
pub struct Poids {
    pub id: String,
    pub libelle: String,
    /// % de gain (DPS + EHP) par point de la stat.
    pub poids: f64,
    /// Gain de DPS et d'EHP, en %, de la valeur testée.
    pub gain_dps_pct: f64,
    pub gain_ehp_pct: f64,
    pub valeur_testee: f64,
}

/// Recherche pondérée pour un emplacement.
#[derive(Debug, Clone, PartialEq)]
pub struct Recherche {
    /// Emplacement PoB (« Body Armour »).
    pub emplacement: String,
    /// Catégorie du site de trade (« armour.chest »).
    pub categorie: Option<String>,
    /// Poids, du plus fort au plus faible.
    pub poids: Vec<Poids>,
    /// Somme pondérée de l'objet équipé (0 si l'emplacement est vide).
    pub minimum: f64,
    /// Le DPS a compté (compétence calculée par le moteur).
    pub avec_dps: bool,
    /// Défenses locales minimales (pièce d'armure) : celles de l'objet équipé.
    pub defenses_min: defense::DefensesLocales,
}

/// Catégorie du site de trade pour une classe d'objet du jeu.
pub fn categorie(classe: &str) -> Option<&'static str> {
    Some(match classe {
        "Wands" => "weapon.wand",
        "Staves" => "weapon.staff",
        "Sceptres" => "weapon.sceptre",
        "Bows" => "weapon.bow",
        "Crossbows" => "weapon.crossbow",
        "Quarterstaves" | "Warstaves" => "weapon.warstaff",
        "One Hand Maces" => "weapon.onemace",
        "Two Hand Maces" => "weapon.twomace",
        "One Hand Swords" => "weapon.onesword",
        "Two Hand Swords" => "weapon.twosword",
        "One Hand Axes" => "weapon.oneaxe",
        "Two Hand Axes" => "weapon.twoaxe",
        "Spears" => "weapon.spear",
        "Flails" => "weapon.flail",
        "Claws" => "weapon.claw",
        "Daggers" => "weapon.dagger",
        "Foci" => "armour.focus",
        "Shields" => "armour.shield",
        "Bucklers" => "armour.buckler",
        "Quivers" => "armour.quiver",
        "Rings" => "accessory.ring",
        "Amulets" => "accessory.amulet",
        "Belts" => "accessory.belt",
        "Gloves" => "armour.gloves",
        "Boots" => "armour.boots",
        "Helmets" => "armour.helmet",
        "Body Armours" => "armour.chest",
        _ => return None,
    })
}

fn est_armure(classe: &str) -> bool {
    matches!(
        classe,
        "Gloves" | "Boots" | "Helmets" | "Body Armours" | "Shields" | "Bucklers" | "Foci"
    )
}

/// Évaluateur du build pour un jeu de lignes ajoutées à l'emplacement vidé.
struct Evaluateur<'a> {
    build: &'a BuildPob,
    retires: Vec<&'static str>,
    classe: &'a str,
    dps_base: Option<f64>,
    bilan_base: defense::Bilan,
}

impl Evaluateur<'_> {
    fn dps(&self, lignes: &[String]) -> Option<f64> {
        let (groupe, gemme, supportee) = competence_comparee(self.build)?;
        if !supportee {
            return None;
        }
        let objet = ObjetCompare {
            classe: self.classe,
            nom: "recherche",
            base: None,
            qualite: None,
            lignes,
            defenses: Default::default(),
        };
        let candidat = (!lignes.is_empty()).then_some(&objet);
        let (perso, _) = modeliser(
            self.build,
            groupe,
            gemme,
            &self.retires,
            candidat,
            &mut Rapport::default(),
        )
        .ok()?;
        dps(&perso).ok().map(|(d, _)| d).filter(|d| *d > 0.0)
    }

    /// Gains de DPS et d'EHP (en %) de `lignes` et `locales` ajoutées.
    fn gains(&self, lignes: &[String], locales: defense::DefensesLocales) -> (f64, f64) {
        let dps = match (self.dps_base, self.dps(lignes)) {
            (Some(b), Some(a)) => (a / b - 1.0) * 100.0,
            _ => 0.0,
        };
        let mut bilan = self.bilan_base.clone();
        bilan.objet(locales, lignes.iter().map(String::as_str));
        let d = defense::comparer(self.build, &self.bilan_base, &bilan);
        let ehp = (d.apres.ehp() / d.avant.ehp() - 1.0) * 100.0;
        (dps, if ehp.is_finite() { ehp } else { 0.0 })
    }
}

/// Recherche pondérée pour un objet de classe `classe` (« Body Armours »).
pub fn recherche(build: &BuildPob, classe: &str) -> Result<Recherche, ErreurComparaison> {
    let (choix, liberes) = emplacements(classe)
        .ok_or_else(|| ErreurComparaison::EmplacementNonSupporte(format!("classe « {classe} »")))?;
    // Bagues : l'emplacement de la bague la moins utile serait plus juste ;
    // on prend le premier, la valeur des stats y est la même.
    let emplacement = choix[0];
    let arme = emplacement.starts_with("Weapon");
    let mut retires: Vec<&'static str> = Vec::new();
    if !arme {
        retires.push(emplacement);
        retires.extend_from_slice(liberes);
    }
    let mut ev = Evaluateur {
        build,
        retires,
        classe,
        dps_base: None,
        bilan_base: bilan_defense(build, &[], None),
    };
    ev.bilan_base = bilan_defense(build, &ev.retires, None);
    ev.dps_base = ev.dps(&[]);
    let armure = est_armure(classe);

    let mut poids: Vec<Poids> = STATS
        .iter()
        .filter(|s| !(armure && s.hors_armure))
        .filter_map(|s| {
            let (dps, ehp) = ev.gains(&[s.ligne.to_string()], Default::default());
            let gain = dps + ehp;
            (gain >= GAIN_MIN_PCT).then(|| Poids {
                id: s.id.to_string(),
                libelle: s.libelle.to_string(),
                poids: gain / s.valeur,
                gain_dps_pct: dps,
                gain_ehp_pct: ehp,
                valeur_testee: s.valeur,
            })
        })
        .collect();
    poids.sort_by(|a, b| b.poids.total_cmp(&a.poids));

    // Minimum : ce que rapporte l'objet équipé, lu avec les mêmes poids.
    let equipe = build.objet(emplacement);
    let minimum = equipe.map_or(0.0, |o| {
        let lignes: Vec<String> = super::lignes_objet(o)
            .iter()
            .map(|l| l.to_string())
            .collect();
        valeur_ponderee(&poids, &lignes)
    });
    let defenses_min = equipe
        .filter(|_| armure)
        .map(|o| defense::DefensesLocales::depuis_texte_pob(&o.texte))
        .unwrap_or_default();

    Ok(Recherche {
        emplacement: emplacement.to_string(),
        categorie: categorie(classe).map(str::to_string),
        poids,
        minimum,
        avec_dps: ev.dps_base.is_some(),
        defenses_min,
    })
}

/// Somme pondérée des lignes d'un objet (lignes reconnues par `STATS`).
pub fn valeur_ponderee(poids: &[Poids], lignes: &[String]) -> f64 {
    lignes
        .iter()
        .filter_map(|l| {
            let (modele, valeur) = modele(l)?;
            let s = STATS.iter().find(|s| s.libelle == modele)?;
            let p = poids.iter().find(|p| p.id == s.id)?;
            Some(p.poids * valeur)
        })
        .sum()
}

/// Modèle d'une ligne (« +# to maximum Life ») et sa valeur.
fn modele(ligne: &str) -> Option<(String, f64)> {
    let l = super::traduction::nettoyer(ligne);
    let mut valeur = None;
    let mut out = String::new();
    let mut nombre = String::new();
    let flush = |nombre: &mut String, out: &mut String, valeur: &mut Option<f64>| {
        if !nombre.is_empty() {
            if valeur.is_none() {
                *valeur = nombre.parse().ok();
            }
            out.push('#');
            nombre.clear();
        }
    };
    for c in l.chars() {
        if c.is_ascii_digit() || (c == '.' && !nombre.is_empty()) {
            nombre.push(c);
        } else {
            flush(&mut nombre, &mut out, &mut valeur);
            out.push(c);
        }
    }
    flush(&mut nombre, &mut out, &mut valeur);
    Some((out, valeur?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pob;

    fn build(objets: &str) -> BuildPob {
        pob::lire_xml(&format!(
            r#"<PathOfBuilding2><Build level="90" className="Sorceress" mainSocketGroup="1"/>
<Skills activeSkillSet="1"><SkillSet id="1"><Skill enabled="true"><Gem nameSpec="Arc" skillId="ArcPlayer" level="20"/></Skill></SkillSet></Skills>
<Items activeItemSet="1">{objets}</Items></PathOfBuilding2>"#
        ))
        .unwrap()
    }

    const OBJETS: &str = r#"<Item id="1">Rarity: RARE
Old
Withered Wand
Implicits: 0
20% increased Lightning Damage
</Item>
<Item id="2">Rarity: RARE
Band
Gold Ring
Implicits: 0
+40 to maximum Life
+30% to Fire Resistance
</Item>
<ItemSet id="1"><Slot name="Weapon 1" itemId="1"/><Slot name="Ring 1" itemId="2"/></ItemSet>"#;

    #[test]
    fn modele_d_une_ligne() {
        assert_eq!(
            modele("+28(25-30)% to Fire Resistance"),
            Some(("+#% to Fire Resistance".into(), 28.0))
        );
        assert_eq!(modele("Withered Wand"), None);
    }

    #[test]
    fn poids_d_une_bague() {
        let r = recherche(&build(OBJETS), "Rings").unwrap();
        assert_eq!(r.emplacement, "Ring 1");
        assert_eq!(r.categorie.as_deref(), Some("accessory.ring"));
        assert!(r.avec_dps);
        let p = |id: &str| r.poids.iter().find(|p| p.id == id);
        // Dégâts de foudre et vie comptent ; la vitesse d'attaque non (sort).
        assert!(p("explicit.stat_2231156303").unwrap().gain_dps_pct > 0.0);
        assert!(p("explicit.stat_3299347043").unwrap().gain_ehp_pct > 0.0);
        assert!(p("explicit.stat_681332047").is_none());
        // La bague équipée rapporte : 40 de vie et 30 de feu.
        let attendu = 40.0 * p("explicit.stat_3299347043").unwrap().poids
            + 30.0 * p("explicit.stat_3372524247").unwrap().poids;
        assert!((r.minimum - attendu).abs() < 1e-9);
        // Du plus fort au plus faible.
        assert!(r.poids.windows(2).all(|w| w[0].poids >= w[1].poids));
    }

    #[test]
    fn competence_non_calculee_defense_seule() {
        let b = pob::lire_xml(
            r#"<PathOfBuilding2><Build level="90" className="Ranger" mainSocketGroup="1"/>
<Skills activeSkillSet="1"><SkillSet id="1"><Skill enabled="true"><Gem nameSpec="Lightning Arrow" skillId="LightningArrowPlayer" level="20"/></Skill></SkillSet></Skills>
<Items activeItemSet="1"></Items></PathOfBuilding2>"#,
        )
        .unwrap();
        let r = recherche(&b, "Body Armours").unwrap();
        assert!(!r.avec_dps);
        assert!(r.poids.iter().all(|p| p.gain_dps_pct == 0.0));
        assert!(r.poids.iter().any(|p| p.id == "explicit.stat_3299347043"));
        // Sur une armure, le bouclier plat est local : pas ce modèle global.
        assert!(r.poids.iter().all(|p| p.id != "explicit.stat_3489782002"));
        assert_eq!(r.minimum, 0.0);
    }
}
