//! Traduction d'une ligne de mod (objet, passif, support) en modificateurs
//! typés du moteur.
//!
//! Seules les lignes qui comptent pour le DPS d'un sort de foudre (Lightning
//! Bolt, Arc) sont traduites. Une ligne qui parle de dégâts, de critique ou
//! de vitesse d'incantation mais qu'aucun motif ne reconnaît (condition, mot
//! inconnu) est rendue comme [`Traduction::NonTraduite`] : le calcul reste
//! possible mais devient « approximatif ». Le reste (vie, résistances,
//! attaques, minions…) est [`Traduction::SansEffet`].

use std::sync::OnceLock;

use regex::Regex;

use crate::calc::stats::{self, TypeDegats};
use crate::calc::{ConditionModificateur, Modificateur, TypeModificateur};

/// Niveaux de gemme ajoutés par un mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Niveaux {
    /// « +N to Level of all Skills ».
    pub toutes: i32,
    /// « +N to Level of all Spell Skills ».
    pub sorts: i32,
    /// « +N to Level of all Lightning Spell Skills » (et « Lightning Skills »).
    pub foudre: i32,
}

impl Niveaux {
    /// Niveaux gagnés par un sort de foudre.
    pub fn pour_sort_de_foudre(&self) -> i32 {
        self.toutes + self.sorts + self.foudre
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Traduction {
    Mods(Vec<Modificateur>),
    Niveaux(Niveaux),
    /// Ligne pertinente pour le DPS, mais pas comprise.
    NonTraduite,
    /// Ligne sans effet sur le DPS d'un sort de foudre.
    SansEffet,
}

/// Nettoie une ligne : balises de Path of Building (`{desecrated}`,
/// `{range:0.5}`…), plages du Ctrl+Alt+C (`28(25-30)`), suffixes
/// `(implicit)`, `(rune)`, `(augmented)`…
pub fn nettoyer(ligne: &str) -> String {
    static BALISES: OnceLock<Regex> = OnceLock::new();
    static PLAGES: OnceLock<Regex> = OnceLock::new();
    static SUFFIXES: OnceLock<Regex> = OnceLock::new();
    let balises = BALISES.get_or_init(|| Regex::new(r"\{[^}]*\}").unwrap());
    let plages =
        PLAGES.get_or_init(|| Regex::new(r"(\d)\(-?\d+(?:\.\d+)?-\d+(?:\.\d+)?\)").unwrap());
    let suffixes = SUFFIXES.get_or_init(|| {
        Regex::new(
            r"\s*\((?:implicit|enchant|rune|crafted|augmented|desecrated|fractured|unscalable value|scalable value)\)\s*$",
        )
        .unwrap()
    });
    let l = balises.replace_all(ligne, "");
    let l = plages.replace_all(&l, "$1");
    let mut l = l.trim().to_string();
    while let Some(m) = suffixes.find(&l) {
        l.truncate(m.start());
    }
    l.trim().to_string()
}

fn type_degats(mot: &str) -> Option<TypeDegats> {
    Some(match mot {
        "Physical" => TypeDegats::Physique,
        "Fire" => TypeDegats::Feu,
        "Cold" => TypeDegats::Froid,
        "Lightning" => TypeDegats::Foudre,
        "Chaos" => TypeDegats::Chaos,
        _ => return None,
    })
}

fn nombre(s: &str) -> f64 {
    s.parse().unwrap_or(0.0)
}

fn signe(mot: &str) -> f64 {
    match mot {
        "reduced" | "less" => -1.0,
        _ => 1.0,
    }
}

fn genre(mot: &str) -> TypeModificateur {
    match mot {
        "more" | "less" => TypeModificateur::More,
        _ => TypeModificateur::Increased,
    }
}

struct Motifs {
    degats: Regex,
    vitesse: Regex,
    critique: Regex,
    bonus_critique: Regex,
    ajoutes: Regex,
    gain: Regex,
    niveaux: Regex,
    penetration: Regex,
    support_degats: Regex,
    support_stat: Regex,
    pertinent: Regex,
    hors_sujet: Regex,
}

fn motifs() -> &'static Motifs {
    static M: OnceLock<Motifs> = OnceLock::new();
    M.get_or_init(|| {
        let n = r"(\d+(?:\.\d+)?)";
        let r = |s: String| Regex::new(&s).unwrap();
        Motifs {
            degats: r(format!(
                r"^{n}% (increased|reduced) (?:(Spell|Elemental|Lightning|Fire|Cold|Chaos|Physical) )?Damage$"
            )),
            vitesse: r(format!(r"^{n}% (increased|reduced) Cast Speed$")),
            critique: r(format!(
                r"^{n}% (increased|reduced) Critical Hit Chance( for Spells)?$"
            )),
            bonus_critique: r(format!(
                r"^{n}% (increased|reduced) Critical (Spell )?Damage Bonus( for Spells)?$"
            )),
            ajoutes: r(format!(
                r"^Adds {n} to {n} (Physical|Fire|Cold|Lightning|Chaos) Damage to Spells$"
            )),
            gain: r(format!(
                r"^Gain {n}% of (?:(Physical|Fire|Cold|Lightning|Chaos|Elemental) )?Damage as Extra (Physical|Fire|Cold|Lightning|Chaos) Damage$"
            )),
            niveaux: r(
                r"^\+(\d+) to Level of all (Spell |Lightning Spell |Lightning |Elemental Spell |)Skills$"
                    .to_string(),
            ),
            penetration: r(format!(
                r"^Damage Penetrates {n}% (Fire|Cold|Lightning|Chaos) Resistance$"
            )),
            support_degats: r(format!(
                r"^Supported (?:Spells|Skills) deal {n}% (more|less|increased|reduced) (?:(Spell|Elemental|Lightning) )?Damage( while you are above (\d+)% of your Maximum Mana)?$"
            )),
            support_stat: r(format!(
                r"^Supported (?:Spells|Skills) have {n}% (more|less|increased|reduced) (Cast Speed|Critical Hit Chance|Critical Damage Bonus)$"
            )),
            pertinent: r(
                r"(?i)damage|critical|cast speed|level of all|spell".to_string(),
            ),
            hors_sujet: r(
                r"(?i)minion|attack|damage taken|taken as|recoup|reflect|ailment|bleed|poison|ignite|leech|block|stun|accuracy|companion|\btraps?\b|\bmines?\b|projectile speed|with (?:bows|crossbows|maces|swords|axes|daggers|claws|spears|quarterstaves|flails)|melee|thorns|charm|flask"
                    .to_string(),
            ),
        }
    })
}

/// Traduit une ligne de mod (déjà affichée par le jeu ou par PoB).
pub fn traduire(ligne: &str, source: &str) -> Traduction {
    let l = nettoyer(ligne);
    if l.is_empty() || l.starts_with("Grants Skill:") {
        return Traduction::SansEffet;
    }
    // Effets qui recopient ou ajoutent d'autres mods : impossibles à chiffrer ici.
    if l.starts_with("Reflects ") || l.starts_with("Allocates ") {
        return Traduction::NonTraduite;
    }
    let m = motifs();
    let mods = |v: Vec<Modificateur>| Traduction::Mods(v);

    if let Some(c) = m.degats.captures(&l) {
        let v = nombre(&c[1]) * signe(&c[2]);
        let stat = match c.get(3).map(|x| x.as_str()) {
            None => stats::DAMAGE.to_string(),
            Some("Spell") => stats::SPELL_DAMAGE.to_string(),
            Some("Elemental") => stats::ELEMENTAL_DAMAGE.to_string(),
            Some(t) => type_degats(t)
                .map(TypeDegats::stat_degats)
                .unwrap_or_default(),
        };
        return mods(vec![Modificateur::new(
            stat,
            TypeModificateur::Increased,
            v,
            source,
        )]);
    }
    if let Some(c) = m.vitesse.captures(&l) {
        return mods(vec![Modificateur::new(
            stats::CAST_SPEED,
            TypeModificateur::Increased,
            nombre(&c[1]) * signe(&c[2]),
            source,
        )]);
    }
    if let Some(c) = m.critique.captures(&l) {
        let stat = if c.get(3).is_some() {
            stats::SPELL_CRITICAL_HIT_CHANCE
        } else {
            stats::CRITICAL_HIT_CHANCE
        };
        return mods(vec![Modificateur::new(
            stat,
            TypeModificateur::Increased,
            nombre(&c[1]) * signe(&c[2]),
            source,
        )]);
    }
    if let Some(c) = m.bonus_critique.captures(&l) {
        let stat = if c.get(3).is_some() || c.get(4).is_some() {
            stats::SPELL_CRITICAL_DAMAGE_BONUS
        } else {
            stats::CRITICAL_DAMAGE_BONUS
        };
        return mods(vec![Modificateur::new(
            stat,
            TypeModificateur::Increased,
            nombre(&c[1]) * signe(&c[2]),
            source,
        )]);
    }
    if let Some(c) = m.ajoutes.captures(&l) {
        let Some(t) = type_degats(&c[3]) else {
            return Traduction::NonTraduite;
        };
        return mods(vec![
            Modificateur::new(
                t.stat_ajoutes_min(),
                TypeModificateur::Added,
                nombre(&c[1]),
                source,
            ),
            Modificateur::new(
                t.stat_ajoutes_max(),
                TypeModificateur::Added,
                nombre(&c[2]),
                source,
            ),
        ]);
    }
    if let Some(c) = m.gain.captures(&l) {
        let Some(cible) = type_degats(&c[3]) else {
            return Traduction::NonTraduite;
        };
        let stat = match c.get(2).map(|x| x.as_str()) {
            None => stats::stat_gain_tous(cible),
            // « Elemental Damage as Extra » : pas de stat de source composite.
            Some("Elemental") => return Traduction::NonTraduite,
            Some(s) => match type_degats(s) {
                Some(src) if src != cible => stats::stat_gain(src, cible),
                _ => return Traduction::NonTraduite,
            },
        };
        return mods(vec![Modificateur::new(
            stat,
            TypeModificateur::Added,
            nombre(&c[1]),
            source,
        )]);
    }
    if let Some(c) = m.niveaux.captures(&l) {
        let n: i32 = c[1].parse().unwrap_or(0);
        let mut niv = Niveaux::default();
        match &c[2] {
            "" => niv.toutes = n,
            "Spell " => niv.sorts = n,
            "Lightning Spell " | "Lightning " => niv.foudre = n,
            // Sorts élémentaires : la foudre en fait partie.
            "Elemental Spell " => niv.foudre = n,
            _ => return Traduction::NonTraduite,
        }
        return Traduction::Niveaux(niv);
    }
    if let Some(c) = m.penetration.captures(&l) {
        let Some(t) = type_degats(&c[2]) else {
            return Traduction::NonTraduite;
        };
        return mods(vec![Modificateur::new(
            t.stat_penetration(),
            TypeModificateur::Added,
            nombre(&c[1]),
            source,
        )]);
    }
    if let Some(c) = m.support_degats.captures(&l) {
        let stat = match c.get(3).map(|x| x.as_str()) {
            None => stats::DAMAGE.to_string(),
            Some("Spell") => stats::SPELL_DAMAGE.to_string(),
            Some("Elemental") => stats::ELEMENTAL_DAMAGE.to_string(),
            Some(t) => type_degats(t)
                .map(TypeDegats::stat_degats)
                .unwrap_or_default(),
        };
        let mut md = Modificateur::new(stat, genre(&c[2]), nombre(&c[1]) * signe(&c[2]), source);
        if let Some(p) = c.get(5) {
            md = md.sous_condition(ConditionModificateur::ManaMinPct(nombre(p.as_str())));
        }
        return mods(vec![md]);
    }
    if let Some(c) = m.support_stat.captures(&l) {
        let stat = match &c[3] {
            "Cast Speed" => stats::CAST_SPEED,
            "Critical Hit Chance" => stats::CRITICAL_HIT_CHANCE,
            _ => stats::CRITICAL_DAMAGE_BONUS,
        };
        return mods(vec![Modificateur::new(
            stat,
            genre(&c[2]),
            nombre(&c[1]) * signe(&c[2]),
            source,
        )]);
    }

    if !l.chars().any(|c| c.is_ascii_digit()) && !l.starts_with("Supported") {
        // Mots-clés sans valeur : effets d'uniques (« Reflects opposite Ring »),
        // en-têtes. Ceux qui parlent de dégâts restent signalés.
        return if m.pertinent.is_match(&l) && !m.hors_sujet.is_match(&l) {
            Traduction::NonTraduite
        } else {
            Traduction::SansEffet
        };
    }
    if m.pertinent.is_match(&l) && !m.hors_sujet.is_match(&l) {
        Traduction::NonTraduite
    } else {
        Traduction::SansEffet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn un(ligne: &str) -> Modificateur {
        match traduire(ligne, "test") {
            Traduction::Mods(v) if v.len() == 1 => v.into_iter().next().unwrap(),
            autre => panic!("{ligne} : {autre:?}"),
        }
    }

    #[test]
    fn nettoie_les_balises() {
        assert_eq!(
            nettoyer("{desecrated}28% increased Cast Speed"),
            "28% increased Cast Speed"
        );
        assert_eq!(
            nettoyer("{range:0.5}{crafted}104(90-110)% increased Lightning Damage (augmented)"),
            "104% increased Lightning Damage"
        );
        assert_eq!(
            nettoyer("+1 to Level of all Spell Skills (rune)"),
            "+1 to Level of all Spell Skills"
        );
    }

    #[test]
    fn traduit_les_degats() {
        let m = un("104% increased Lightning Damage");
        assert_eq!(
            (m.stat.as_str(), m.kind, m.valeur),
            ("lightning_damage", TypeModificateur::Increased, 104.0)
        );
        assert_eq!(un("66% increased Spell Damage").stat, "spell_damage");
        assert_eq!(
            un("10% increased Elemental Damage").stat,
            "elemental_damage"
        );
        assert_eq!(un("20% increased Damage").stat, "damage");
        assert_eq!(un("15% reduced Fire Damage").valeur, -15.0);
    }

    #[test]
    fn traduit_vitesse_et_critique() {
        assert_eq!(un("28% increased Cast Speed").stat, "cast_speed");
        assert_eq!(
            un("52% increased Critical Hit Chance for Spells").stat,
            "spell_critical_hit_chance"
        );
        assert_eq!(
            un("38% increased Critical Hit Chance").stat,
            "critical_hit_chance"
        );
        assert_eq!(
            un("15% increased Critical Damage Bonus").stat,
            "critical_damage_bonus"
        );
        assert_eq!(
            un("30% increased Critical Spell Damage Bonus").stat,
            "spell_critical_damage_bonus"
        );
    }

    #[test]
    fn traduit_ajoutes_gain_penetration() {
        match traduire("Adds 5 to 98 Lightning Damage to Spells", "t") {
            Traduction::Mods(v) => {
                assert_eq!(v[0].stat, "added_lightning_damage_min");
                assert_eq!((v[0].valeur, v[1].valeur), (5.0, 98.0));
            }
            autre => panic!("{autre:?}"),
        }
        assert_eq!(
            un("Gain 29% of Damage as Extra Lightning Damage").stat,
            "gain_damage_as_extra_lightning"
        );
        assert_eq!(
            un("Gain 20% of Lightning Damage as Extra Cold Damage").stat,
            "gain_lightning_damage_as_extra_cold"
        );
        assert_eq!(
            un("Damage Penetrates 6% Lightning Resistance").stat,
            "lightning_penetration"
        );
    }

    #[test]
    fn traduit_les_niveaux() {
        let niv = |l: &str| match traduire(l, "t") {
            Traduction::Niveaux(n) => n.pour_sort_de_foudre(),
            autre => panic!("{autre:?}"),
        };
        assert_eq!(niv("+1 to Level of all Spell Skills"), 1);
        assert_eq!(niv("{fractured}+3 to Level of all Spell Skills"), 3);
        assert_eq!(niv("+5 to Level of all Lightning Spell Skills"), 5);
        assert_eq!(niv("+1 to Level of all Skills"), 1);
        assert_eq!(
            traduire("+2 to Level of all Minion Skills", "t"),
            Traduction::SansEffet
        );
        assert_eq!(
            traduire("+2 to Level of all Cold Spell Skills", "t"),
            Traduction::NonTraduite
        );
    }

    #[test]
    fn traduit_les_supports() {
        let z = un(
            "Supported Spells deal 30% more Damage while you are above 90% of your Maximum Mana",
        );
        assert_eq!(
            (z.stat.as_str(), z.kind, z.valeur),
            ("damage", TypeModificateur::More, 30.0)
        );
        assert_eq!(z.condition, Some(ConditionModificateur::ManaMinPct(90.0)));
        let p = un("Supported Skills have 30% less Critical Damage Bonus");
        assert_eq!(
            (p.stat.as_str(), p.kind, p.valeur),
            ("critical_damage_bonus", TypeModificateur::More, -30.0)
        );
        assert_eq!(
            un("Supported Skills have 30% less Cast Speed").valeur,
            -30.0
        );
    }

    #[test]
    fn classe_le_reste() {
        for l in [
            "+119 to maximum Life",
            "+42% to Fire Resistance",
            "35% increased Movement Speed",
            "Minions deal 10% increased Damage",
            "10% increased Attack Damage",
            "Rarity: RARE",
            "Item Level: 82",
            "Grants Skill: Level 18 Spellslinger",
        ] {
            assert_eq!(traduire(l, "t"), Traduction::SansEffet, "{l}");
        }
        for l in [
            "Triggered Spells deal 16% increased Spell Damage",
            "10% increased Projectile Damage",
            "+2% to Critical Hit Chance",
            "Supported Spells Echo 1 time",
            "Reflects opposite Ring",
            "Allocates Fast Metabolism",
        ] {
            assert_eq!(traduire(l, "t"), Traduction::NonTraduite, "{l}");
        }
    }
}
