//! Défense comparée : vie, bouclier d'énergie, armure, évasion, résistances
//! et EHP du personnage, avec l'équipement actuel puis avec l'objet copié.
//!
//! Les lignes de mods défensives des objets, des passifs et des joyaux sont
//! lues ici, à part de [`super::traduction`] (qui ne sert que le DPS). Seules
//! les lignes sans condition sont comptées.
//!
//! Formules (confiance `supposé`, constantes de Path of Building PoE 2) :
//!
//! - vie = (16 + 12 × niveau + 2 × force + vie plate) × (1 + increased) ;
//! - bouclier d'énergie, armure, évasion = (Σ valeurs locales des objets +
//!   plats globaux) × (1 + increased globaux) ; l'évasion part de 7 ;
//! - une pièce d'armure porte ses défenses déjà calculées (« Armour: 412 »
//!   dans PoB, « Armour: 412 (augmented) » en jeu) : ses lignes locales
//!   (« +40 to Armour », « 50% increased Energy Shield ») sont donc ignorées ;
//! - résistance = min(Σ résistances + pénalité, maximale), maximale =
//!   75 + bonus, plafond 90 ;
//! - EHP = moyenne des plus gros coups encaissables d'un type (physique avec
//!   l'armure, feu, froid, foudre avec les résistances, chaos avec la vie et
//!   la moitié du bouclier d'énergie), voir [`crate::calc::defense`].
//!
//! **Étalonnage.** Un code Path of Building exporte les stats calculées par
//! PoB (`<PlayerStat stat="Life" …/>`). Quand elles sont là, l'« avant » est
//! celui de PoB (bonus de quêtes, conditions de la configuration…) et
//! l'« après » lui ajoute l'écart calculé ici : seules les lignes des deux
//! objets échangés doivent alors être justes. Sans elles, les valeurs sont
//! celles d'Augur, avec la pénalité de fin de campagne (−60 %).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::calc::defense::{RESISTANCE_MAX_ABSOLUE, RESISTANCE_MAX_BASE, coup_physique_max};
use crate::calc::stats::TypeDegats;
use crate::pob::BuildPob;

use super::traduction::nettoyer;

/// Pénalité de résistance élémentaire en fin de campagne (mesurée, I-02).
pub const PENALITE_FIN_CAMPAGNE: f64 = -60.0;
/// Évasion de base du personnage (`base_evasion_rating`).
pub const EVASION_BASE: f64 = 7.0;
/// Types de résistance affichés, dans l'ordre de l'overlay.
pub const RESISTANCES: [TypeDegats; 4] = [
    TypeDegats::Feu,
    TypeDegats::Froid,
    TypeDegats::Foudre,
    TypeDegats::Chaos,
];

/// Défenses déjà calculées d'une pièce d'armure (valeurs affichées).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DefensesLocales {
    pub armure: f64,
    pub evasion: f64,
    pub bouclier_energie: f64,
}

impl DefensesLocales {
    pub fn est_vide(&self) -> bool {
        self.armure == 0.0 && self.evasion == 0.0 && self.bouclier_energie == 0.0
    }

    /// Lit les lignes de propriétés (« Armour: 412 », « Evasion Rating: 80
    /// (augmented) », « Energy Shield: 120 »).
    pub fn depuis_proprietes<'a>(lignes: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut d = Self::default();
        for (nom, valeur) in lignes {
            let v: f64 = valeur
                .replace("(augmented)", "")
                .trim()
                .parse()
                .unwrap_or(0.0);
            match nom.trim() {
                "Armour" => d.armure = v,
                "Evasion" | "Evasion Rating" => d.evasion = v,
                "Energy Shield" => d.bouclier_energie = v,
                _ => {}
            }
        }
        d
    }

    /// Défenses d'un objet PoB : lignes d'en-tête avant « Implicits: ».
    pub fn depuis_texte_pob(texte: &str) -> Self {
        Self::depuis_proprietes(
            texte
                .lines()
                .take_while(|l| !l.starts_with("Implicits:"))
                .filter_map(|l| l.split_once(':')),
        )
    }
}

/// Somme des lignes défensives d'un personnage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bilan {
    pub vie_plate: f64,
    pub vie_increased: f64,
    pub force: f64,
    pub locales: DefensesLocales,
    pub es_plat: f64,
    pub es_increased: f64,
    pub armure_plate: f64,
    pub armure_increased: f64,
    pub evasion_plate: f64,
    pub evasion_increased: f64,
    /// Résistances non plafonnées, par [`TypeDegats::index`].
    pub resistances: [f64; 5],
    /// Bonus de résistance maximale, par [`TypeDegats::index`].
    pub resistances_max: [f64; 5],
}

struct Motifs {
    vie_plate: Regex,
    vie_inc: Regex,
    attribut: Regex,
    tous_attributs: Regex,
    deux_attributs: Regex,
    plat: Regex,
    increased: Regex,
    resistance: Regex,
    deux_resistances: Regex,
    toutes_resistances: Regex,
    resistance_max: Regex,
    toutes_resistances_max: Regex,
}

fn motifs() -> &'static Motifs {
    static M: OnceLock<Motifs> = OnceLock::new();
    M.get_or_init(|| {
        let r = |s: &str| Regex::new(s).unwrap();
        Motifs {
            vie_plate: r(r"^\+(\d+) to maximum Life$"),
            vie_inc: r(r"^(\d+)% (increased|reduced) maximum Life$"),
            attribut: r(r"^\+(\d+) to Strength$"),
            tous_attributs: r(r"^\+(\d+) to all Attributes$"),
            deux_attributs: r(r"^\+(\d+) to Strength and (?:Dexterity|Intelligence)$"),
            plat: r(r"^\+(\d+) to (Armour|Evasion Rating|maximum Energy Shield)$"),
            increased: r(
                r"^(\d+)% (increased|reduced) (Armour|Evasion Rating|Energy Shield|maximum Energy Shield|Armour and Evasion(?: Rating)?|Armour and Energy Shield|Evasion and Energy Shield|Armour, Evasion and Energy Shield|Defences|Global Defences)$",
            ),
            resistance: r(r"^([+-]\d+)% to (Fire|Cold|Lightning|Chaos) Resistance$"),
            deux_resistances: r(
                r"^\+(\d+)% to (Fire|Cold|Lightning) and (Fire|Cold|Lightning|Chaos) Resistances$",
            ),
            toutes_resistances: r(r"^([+-]\d+)% to all Elemental Resistances$"),
            resistance_max: r(r"^\+(\d+)% to Maximum (Fire|Cold|Lightning|Chaos) Resistance$"),
            toutes_resistances_max: r(r"^\+(\d+)% to all Maximum Elemental Resistances$"),
        }
    })
}

fn type_degats(mot: &str) -> Option<TypeDegats> {
    Some(match mot {
        "Fire" => TypeDegats::Feu,
        "Cold" => TypeDegats::Froid,
        "Lightning" => TypeDegats::Foudre,
        "Chaos" => TypeDegats::Chaos,
        _ => return None,
    })
}

const ELEMENTAIRES: [TypeDegats; 3] = [TypeDegats::Feu, TypeDegats::Froid, TypeDegats::Foudre];

impl Bilan {
    /// Ajoute une ligne de mod ; `locales` : l'objet porte ses défenses
    /// calculées, ses lignes d'armure, d'évasion et de bouclier sont déjà
    /// comptées. Renvoie `true` si la ligne a été comptée.
    pub fn ligne(&mut self, ligne: &str, locales: bool) -> bool {
        let l = nettoyer(ligne);
        let m = motifs();
        let n = |s: &str| s.trim_start_matches('+').parse::<f64>().unwrap_or(0.0);
        let sens = |s: &str| if s == "reduced" { -1.0 } else { 1.0 };
        if let Some(c) = m.vie_plate.captures(&l) {
            self.vie_plate += n(&c[1]);
        } else if let Some(c) = m.vie_inc.captures(&l) {
            self.vie_increased += sens(&c[2]) * n(&c[1]);
        } else if let Some(c) = m
            .attribut
            .captures(&l)
            .or_else(|| m.tous_attributs.captures(&l))
            .or_else(|| m.deux_attributs.captures(&l))
        {
            self.force += n(&c[1]);
        } else if let Some(c) = m.plat.captures(&l) {
            if locales {
                return true;
            }
            let v = n(&c[1]);
            match &c[2] {
                "Armour" => self.armure_plate += v,
                "Evasion Rating" => self.evasion_plate += v,
                _ => self.es_plat += v,
            }
        } else if let Some(c) = m.increased.captures(&l) {
            if locales {
                return true;
            }
            let v = sens(&c[2]) * n(&c[1]);
            let cible = &c[3];
            if cible.contains("Armour") || cible.contains("Defences") {
                self.armure_increased += v;
            }
            if cible.contains("Evasion") || cible.contains("Defences") {
                self.evasion_increased += v;
            }
            if cible.contains("Energy Shield") || cible.contains("Defences") {
                self.es_increased += v;
            }
        } else if let Some(c) = m.resistance.captures(&l) {
            if let Some(t) = type_degats(&c[2]) {
                self.resistances[t.index()] += n(&c[1]);
            }
        } else if let Some(c) = m.deux_resistances.captures(&l) {
            for mot in [&c[2], &c[3]] {
                if let Some(t) = type_degats(mot) {
                    self.resistances[t.index()] += n(&c[1]);
                }
            }
        } else if let Some(c) = m.toutes_resistances.captures(&l) {
            for t in ELEMENTAIRES {
                self.resistances[t.index()] += n(&c[1]);
            }
        } else if let Some(c) = m.resistance_max.captures(&l) {
            if let Some(t) = type_degats(&c[2]) {
                self.resistances_max[t.index()] += n(&c[1]);
            }
        } else if let Some(c) = m.toutes_resistances_max.captures(&l) {
            for t in ELEMENTAIRES {
                self.resistances_max[t.index()] += n(&c[1]);
            }
        } else {
            return false;
        }
        true
    }

    /// Ajoute un objet : ses défenses calculées et ses lignes de mods.
    pub fn objet<'a>(
        &mut self,
        locales: DefensesLocales,
        lignes: impl IntoIterator<Item = &'a str>,
    ) {
        let a_locales = !locales.est_vide();
        self.locales.armure += locales.armure;
        self.locales.evasion += locales.evasion;
        self.locales.bouclier_energie += locales.bouclier_energie;
        for l in lignes {
            self.ligne(l, a_locales);
        }
    }

    /// Valeurs du personnage au `niveau` donné, avec la `penalite` de
    /// résistance élémentaire.
    pub fn defenses(&self, niveau: u32, penalite: f64) -> Defenses {
        let inc = |v: f64| (1.0 + v / 100.0).max(0.0);
        let vie = (16.0 + 12.0 * f64::from(niveau) + 2.0 * self.force + self.vie_plate)
            * inc(self.vie_increased);
        let bouclier_energie =
            (self.locales.bouclier_energie + self.es_plat) * inc(self.es_increased);
        let armure = (self.locales.armure + self.armure_plate) * inc(self.armure_increased);
        let evasion = (EVASION_BASE + self.locales.evasion + self.evasion_plate)
            * inc(self.evasion_increased);
        let resistances = RESISTANCES
            .iter()
            .map(|t| {
                let p = if t.est_elementaire() { penalite } else { 0.0 };
                Resistance {
                    type_degats: *t,
                    non_plafonnee: self.resistances[t.index()] + p,
                    maximale: (RESISTANCE_MAX_BASE + self.resistances_max[t.index()])
                        .min(RESISTANCE_MAX_ABSOLUE),
                }
            })
            .collect();
        Defenses {
            vie: vie.max(1.0),
            bouclier_energie: bouclier_energie.max(0.0),
            armure: armure.max(0.0),
            evasion: evasion.max(0.0),
            resistances,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resistance {
    pub type_degats: TypeDegats,
    pub non_plafonnee: f64,
    pub maximale: f64,
}

impl Resistance {
    pub fn valeur(&self) -> f64 {
        self.non_plafonnee.min(self.maximale)
    }
}

/// Défenses d'un personnage.
#[derive(Debug, Clone, PartialEq)]
pub struct Defenses {
    pub vie: f64,
    pub bouclier_energie: f64,
    pub armure: f64,
    pub evasion: f64,
    pub resistances: Vec<Resistance>,
}

impl Defenses {
    pub fn resistance(&self, t: TypeDegats) -> Option<&Resistance> {
        self.resistances.iter().find(|r| r.type_degats == t)
    }

    /// Plus gros coup encaissable de chaque type, dans l'ordre de
    /// [`TypeDegats::TOUS`].
    pub fn coups_max(&self) -> Vec<(TypeDegats, f64)> {
        let reserve = self.vie + self.bouclier_energie;
        TypeDegats::TOUS
            .iter()
            .map(|t| {
                let v = match self.resistance(*t) {
                    None => coup_physique_max(reserve, self.armure),
                    Some(r) => {
                        let pool = if *t == TypeDegats::Chaos {
                            self.vie + self.bouclier_energie / 2.0
                        } else {
                            reserve
                        };
                        pool / (1.0 - r.valeur() / 100.0)
                    }
                };
                (*t, v)
            })
            .collect()
    }

    /// EHP : moyenne des plus gros coups encaissables des cinq types.
    pub fn ehp(&self) -> f64 {
        let coups = self.coups_max();
        coups.iter().map(|(_, v)| v).sum::<f64>() / coups.len() as f64
    }

    /// Recale ces défenses (calculées par Augur) sur les stats de PoB :
    /// `avant` est le calcul d'Augur pour l'équipement actuel ; l'écart
    /// entre `self` et `avant` s'ajoute aux valeurs de PoB.
    fn etalonner(&self, avant: &Defenses, pob: &StatsPob) -> Defenses {
        let ecart = |pob: Option<f64>, apres: f64, avant: f64| match pob {
            Some(p) => (p + apres - avant).max(0.0),
            None => apres,
        };
        let resistances = self
            .resistances
            .iter()
            .zip(&avant.resistances)
            .map(|(apres, av)| {
                let t = apres.type_degats;
                match pob.resistance(t) {
                    Some((valeur, sur_plafond)) => {
                        // Au-dessus du plafond, PoB donne le maximum ; sinon
                        // on garde celui d'Augur (au moins la valeur de PoB).
                        let max_avant = if sur_plafond > 0.0 {
                            valeur
                        } else {
                            av.maximale.max(valeur)
                        };
                        Resistance {
                            type_degats: t,
                            non_plafonnee: valeur + sur_plafond + apres.non_plafonnee
                                - av.non_plafonnee,
                            maximale: (max_avant + apres.maximale - av.maximale)
                                .min(RESISTANCE_MAX_ABSOLUE),
                        }
                    }
                    None => *apres,
                }
            })
            .collect();
        Defenses {
            vie: ecart(pob.get("Life"), self.vie, avant.vie).max(1.0),
            bouclier_energie: ecart(
                pob.get("EnergyShield"),
                self.bouclier_energie,
                avant.bouclier_energie,
            ),
            armure: ecart(pob.get("Armour"), self.armure, avant.armure),
            evasion: ecart(pob.get("Evasion"), self.evasion, avant.evasion),
            resistances,
        }
    }
}

/// Stats calculées par Path of Building, exportées avec le build.
struct StatsPob<'a>(&'a BTreeMap<String, f64>);

impl StatsPob<'_> {
    fn get(&self, nom: &str) -> Option<f64> {
        self.0.get(nom).copied().filter(|v| v.is_finite())
    }

    /// Résistance affichée par PoB et excédent au-dessus du plafond.
    fn resistance(&self, t: TypeDegats) -> Option<(f64, f64)> {
        let nom = match t {
            TypeDegats::Feu => "Fire",
            TypeDegats::Froid => "Cold",
            TypeDegats::Foudre => "Lightning",
            TypeDegats::Chaos => "Chaos",
            TypeDegats::Physique => return None,
        };
        let v = self.get(&format!("{nom}Resist"))?;
        Some((v, self.get(&format!("{nom}ResistOverCap")).unwrap_or(0.0)))
    }

    fn present(&self) -> bool {
        self.get("Life").is_some()
    }
}

/// Défenses avant et après l'échange d'objet.
#[derive(Debug, Clone, PartialEq)]
pub struct DefenseComparee {
    pub avant: Defenses,
    pub apres: Defenses,
    /// L'« avant » vient des stats exportées par Path of Building.
    pub etalonnee: bool,
}

/// Compare les défenses : `bilan_avant` (équipement actuel) et `bilan_apres`
/// (objet copié à la place), étalonnées sur les stats de PoB si elles sont là.
pub fn comparer(build: &BuildPob, bilan_avant: &Bilan, bilan_apres: &Bilan) -> DefenseComparee {
    let pob = StatsPob(&build.stats);
    let avant = bilan_avant.defenses(build.niveau, PENALITE_FIN_CAMPAGNE);
    let apres = bilan_apres.defenses(build.niveau, PENALITE_FIN_CAMPAGNE);
    if pob.present() {
        DefenseComparee {
            apres: apres.etalonner(&avant, &pob),
            avant: avant.etalonner(&avant, &pob),
            etalonnee: true,
        }
    } else {
        DefenseComparee {
            avant,
            apres,
            etalonnee: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bilan(lignes: &[&str]) -> Bilan {
        let mut b = Bilan::default();
        for l in lignes {
            assert!(b.ligne(l, false), "ligne non comptée : {l}");
        }
        b
    }

    #[test]
    fn lit_les_lignes_defensives() {
        let b = bilan(&[
            "+80 to maximum Life",
            "8% increased maximum Life",
            "+10 to Strength",
            "+5 to all Attributes",
            "+30% to Fire Resistance",
            "+12% to Fire and Cold Resistances",
            "+10% to all Elemental Resistances",
            "-7% to Chaos Resistance",
            "+1% to Maximum Lightning Resistance",
            "+40 to maximum Energy Shield",
            "20% increased maximum Energy Shield",
            "15% increased Armour and Evasion Rating",
            "+28(25-30)% to Lightning Resistance",
        ]);
        assert_eq!(b.vie_plate, 80.0);
        assert_eq!(b.vie_increased, 8.0);
        assert_eq!(b.force, 15.0);
        assert_eq!(b.resistances[TypeDegats::Feu.index()], 52.0);
        assert_eq!(b.resistances[TypeDegats::Froid.index()], 22.0);
        assert_eq!(b.resistances[TypeDegats::Foudre.index()], 38.0);
        assert_eq!(b.resistances[TypeDegats::Chaos.index()], -7.0);
        assert_eq!(b.resistances_max[TypeDegats::Foudre.index()], 1.0);
        assert_eq!((b.es_plat, b.es_increased), (40.0, 20.0));
        assert_eq!((b.armure_increased, b.evasion_increased), (15.0, 15.0));
    }

    #[test]
    fn ignore_les_lignes_conditionnelles() {
        let mut b = Bilan::default();
        for l in [
            "Damage Penetrates 10% Fire Resistance",
            "Minions have 20% increased maximum Life",
            "10% increased Armour while stationary",
            "Minions have +10% to all Elemental Resistances",
        ] {
            assert!(!b.ligne(l, false), "{l}");
        }
        assert_eq!(b, Bilan::default());
    }

    #[test]
    fn lignes_locales_d_une_piece_d_armure() {
        let mut b = Bilan::default();
        let locales = DefensesLocales::depuis_texte_pob(
            "Rarity: RARE\nX\nVile Robe\nEnergy Shield: 300\nQuality: 20\nImplicits: 0\n+60 to maximum Energy Shield\n50% increased Energy Shield\n+70 to maximum Life",
        );
        assert_eq!(locales.bouclier_energie, 300.0);
        b.objet(
            locales,
            [
                "+60 to maximum Energy Shield",
                "50% increased Energy Shield",
                "+70 to maximum Life",
            ],
        );
        assert_eq!(b.locales.bouclier_energie, 300.0);
        assert_eq!((b.es_plat, b.es_increased), (0.0, 0.0));
        assert_eq!(b.vie_plate, 70.0);
    }

    #[test]
    fn vie_et_resistances() {
        let b = bilan(&[
            "+100 to maximum Life",
            "10% increased maximum Life",
            "+50% to Fire Resistance",
        ]);
        let d = b.defenses(90, PENALITE_FIN_CAMPAGNE);
        assert!((d.vie - (16.0 + 1080.0 + 100.0) * 1.1).abs() < 1e-9);
        let feu = d.resistance(TypeDegats::Feu).unwrap();
        assert_eq!(
            (feu.non_plafonnee, feu.maximale, feu.valeur()),
            (-10.0, 75.0, -10.0)
        );
        let chaos = d.resistance(TypeDegats::Chaos).unwrap();
        assert_eq!(chaos.valeur(), 0.0);
    }

    #[test]
    fn ehp_moyenne_des_coups_max() {
        let d = Defenses {
            vie: 1000.0,
            bouclier_energie: 0.0,
            armure: 0.0,
            evasion: 0.0,
            resistances: RESISTANCES
                .iter()
                .map(|t| Resistance {
                    type_degats: *t,
                    non_plafonnee: 50.0,
                    maximale: 75.0,
                })
                .collect(),
        };
        // Physique : 1000 ; quatre types à 50 % : 2000 chacun.
        assert!((d.ehp() - 9000.0 / 5.0).abs() < 1e-9);
    }
}
