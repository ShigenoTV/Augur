//! Vocabulaire des stats internes du moteur.
//!
//! Les identifiants sont en anglais, comme les `stat_id` du jeu, pour que les
//! tables de correspondance du Theorycrafter restent lisibles des deux côtés.
//! Toute stat utilisée dans un cas golden doit figurer dans [`stats_connues`] :
//! une faute de frappe est une erreur explicite, jamais une stat ignorée.

use serde::{Deserialize, Serialize};

/// Les types de dégâts de PoE 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeDegats {
    Physique,
    Feu,
    Froid,
    Foudre,
    Chaos,
}

impl TypeDegats {
    pub const TOUS: [TypeDegats; 5] = [
        TypeDegats::Physique,
        TypeDegats::Feu,
        TypeDegats::Froid,
        TypeDegats::Foudre,
        TypeDegats::Chaos,
    ];

    /// Nom français, utilisé dans les cas golden et les traces.
    pub fn nom(self) -> &'static str {
        match self {
            TypeDegats::Physique => "physique",
            TypeDegats::Feu => "feu",
            TypeDegats::Froid => "froid",
            TypeDegats::Foudre => "foudre",
            TypeDegats::Chaos => "chaos",
        }
    }

    /// Fragment anglais utilisé dans les identifiants de stats.
    pub fn cle(self) -> &'static str {
        match self {
            TypeDegats::Physique => "physical",
            TypeDegats::Feu => "fire",
            TypeDegats::Froid => "cold",
            TypeDegats::Foudre => "lightning",
            TypeDegats::Chaos => "chaos",
        }
    }

    pub fn est_elementaire(self) -> bool {
        matches!(
            self,
            TypeDegats::Feu | TypeDegats::Froid | TypeDegats::Foudre
        )
    }

    /// Stat de dégâts propre au type (`fire_damage`…).
    pub fn stat_degats(self) -> String {
        format!("{}_damage", self.cle())
    }

    pub fn stat_ajoutes_min(self) -> String {
        format!("added_{}_damage_min", self.cle())
    }

    pub fn stat_ajoutes_max(self) -> String {
        format!("added_{}_damage_max", self.cle())
    }

    pub fn stat_penetration(self) -> String {
        format!("{}_penetration", self.cle())
    }

    pub fn stat_resistance(self) -> String {
        format!("{}_resistance", self.cle())
    }

    pub fn stat_resistance_max(self) -> String {
        format!("maximum_{}_resistance", self.cle())
    }
}

// Stats génériques.
pub const DAMAGE: &str = "damage";
pub const SPELL_DAMAGE: &str = "spell_damage";
pub const ELEMENTAL_DAMAGE: &str = "elemental_damage";
pub const CAST_SPEED: &str = "cast_speed";
pub const CRITICAL_HIT_CHANCE: &str = "critical_hit_chance";
pub const CRITICAL_DAMAGE_BONUS: &str = "critical_damage_bonus";
pub const SPELL_CRITICAL_HIT_CHANCE: &str = "spell_critical_hit_chance";
pub const SPELL_CRITICAL_DAMAGE_BONUS: &str = "spell_critical_damage_bonus";
pub const LIFE: &str = "life";
pub const ENERGY_SHIELD: &str = "energy_shield";
pub const ARMOUR: &str = "armour";
pub const ELEMENTAL_RESISTANCE: &str = "elemental_resistance";
pub const ALL_MAXIMUM_ELEMENTAL_RESISTANCE: &str = "maximum_elemental_resistance";
/// Réduction des dégâts physiques additionnelle, en points, ajoutée à celle
/// de l'armure avant le plafond de 90 %.
pub const PHYSICAL_DAMAGE_REDUCTION: &str = "physical_damage_reduction";
/// Dégâts subis (increased / more), tous types confondus.
pub const DAMAGE_TAKEN: &str = "damage_taken";

/// Liste exhaustive des stats que le moteur sait lire.
pub fn stats_connues() -> Vec<String> {
    let mut v: Vec<String> = [
        DAMAGE,
        SPELL_DAMAGE,
        ELEMENTAL_DAMAGE,
        CAST_SPEED,
        CRITICAL_HIT_CHANCE,
        CRITICAL_DAMAGE_BONUS,
        SPELL_CRITICAL_HIT_CHANCE,
        SPELL_CRITICAL_DAMAGE_BONUS,
        LIFE,
        ENERGY_SHIELD,
        ARMOUR,
        ELEMENTAL_RESISTANCE,
        ALL_MAXIMUM_ELEMENTAL_RESISTANCE,
        PHYSICAL_DAMAGE_REDUCTION,
        DAMAGE_TAKEN,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for t in TypeDegats::TOUS {
        v.push(t.stat_degats());
        v.push(t.stat_ajoutes_min());
        v.push(t.stat_ajoutes_max());
        v.push(t.stat_penetration());
        v.push(t.stat_resistance());
        v.push(t.stat_resistance_max());
    }
    v
}

pub fn est_connue(stat: &str) -> bool {
    stats_connues().iter().any(|s| s == stat)
}
