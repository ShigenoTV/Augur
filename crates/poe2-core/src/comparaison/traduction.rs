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
    /// « +N to Level of all Elemental Spell Skills » : feu, froid, foudre.
    pub elementaires: i32,
    /// « +N to Level of all Fire Spell Skills » (et « Fire Skills »…), par
    /// type dans l'ordre de [`TypeDegats::TOUS`].
    pub par_type: [i32; 5],
    /// « +N to Level of all Minion Skills ».
    pub minions: i32,
}

impl Niveaux {
    /// Niveaux gagnés par un sort portant ces tags d'élément. Un sort à
    /// plusieurs éléments (Comet : feu et froid) profite du meilleur bonus
    /// propre à un élément, pas de leur somme.
    pub fn pour_sort(&self, elements: &[TypeDegats]) -> i32 {
        let elementaire = elements.iter().any(|t| t.est_elementaire());
        let propre = elements
            .iter()
            .map(|t| self.par_type[t.index()])
            .max()
            .unwrap_or(0);
        self.toutes + self.sorts + if elementaire { self.elementaires } else { 0 } + propre
    }

    /// Niveaux gagnés par une gemme de minions.
    pub fn pour_minion(&self) -> i32 {
        self.toutes + self.minions
    }

    pub fn ajouter(&mut self, autre: &Niveaux) {
        self.toutes += autre.toutes;
        self.minions += autre.minions;
        self.sorts += autre.sorts;
        self.elementaires += autre.elementaires;
        for (a, b) in self.par_type.iter_mut().zip(autre.par_type) {
            *a += b;
        }
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
    dot: Regex,
    support_dot: Regex,
    minion_degats: Regex,
    minion_vitesse: Regex,
    minion_critique: Regex,
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
                r"^{n}% (increased|reduced) (?:(Spell|Attack|Elemental|Lightning|Fire|Cold|Chaos|Physical) )?Damage$"
            )),
            vitesse: r(format!(r"^{n}% (increased|reduced) (Cast|Attack) Speed$")),
            critique: r(format!(
                r"^{n}% (increased|reduced) Critical Hit Chance( for Spells)?$"
            )),
            bonus_critique: r(format!(
                r"^{n}% (increased|reduced) Critical (Spell )?Damage Bonus( for Spells)?$"
            )),
            ajoutes: r(format!(
                r"^Adds {n} to {n} (Physical|Fire|Cold|Lightning|Chaos) Damage to (Spells|Attacks)$"
            )),
            gain: r(format!(
                r"^(?:Supported Skills )?Gain {n}% of (?:(Physical|Fire|Cold|Lightning|Chaos|Elemental) )?Damage as [Ee]xtra (Physical|Fire|Cold|Lightning|Chaos) Damage$"
            )),
            niveaux: r(
                r"^\+(\d+) to Level of all (Spell |Elemental Spell |Minion |(?:(Physical|Fire|Cold|Lightning|Chaos) (?:Spell )?)|)Skills$"
                    .to_string(),
            ),
            penetration: r(format!(
                r"^Damage Penetrates {n}% (Fire|Cold|Lightning|Chaos) Resistance$"
            )),
            support_degats: r(format!(
                r"^Supported (?:Spells|Skills) deal {n}% (more|less|increased|reduced) (?:(Spell|Elemental|Physical|Fire|Cold|Lightning|Chaos) )?[Dd]amage( with Hits)?( while you are above (\d+)% of your Maximum Mana)?$"
            )),
            support_stat: r(format!(
                r"^Supported (?:Spells|Skills) have {n}% (more|less|increased|reduced) (Cast Speed|Attack Speed|Critical Hit Chance|Critical Damage Bonus)$"
            )),
            dot: r(format!(
                r"^{n}% (increased|reduced) (?:(Physical|Fire|Cold|Lightning|Chaos) )?Damage over Time$"
            )),
            support_dot: r(format!(
                r"^Supported Skills deal {n}% (more|less) (?:non-Ailment )?Damage over Time$"
            )),
            minion_degats: r(format!(
                r"^(?:Minions|Minions from Supported Skills|Supported Minions) deal {n}% (increased|reduced|more|less) Damage$"
            )),
            minion_vitesse: r(format!(
                r"^Minions have {n}% (increased|reduced) (Attack and Cast|Attack|Cast) Speed$"
            )),
            minion_critique: r(format!(
                r"^Minions have {n}% (increased|reduced) Critical (Hit Chance|Damage Bonus)$"
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

/// Lignes des ailments à dégâts (ignite, poison, saignement) : chance,
/// magnitude, durée, vitesse, nombre de poisons. Les lignes conditionnelles
/// (« against Bleeding Enemies », « with Critical Hits »…) ne sont pas
/// reconnues et restent sans effet, comme avant.
fn ailment(l: &str, source: &str) -> Option<Vec<Modificateur>> {
    static M: OnceLock<[Regex; 9]> = OnceLock::new();
    let n = r"(\d+(?:\.\d+)?)";
    let m = M.get_or_init(|| {
        let r = |s: String| Regex::new(&s).unwrap();
        [
            r(format!(
                r"^(?:Supported Skills have )?{n}% (increased|reduced|more|less) Flammability Magnitude$"
            )),
            r(format!(
                r"^(?:Supported Skills have )?{n}% (increased|reduced|more|less) chance to inflict Ailments$"
            )),
            r(format!(
                r"^(?:Supported Skills have )?{n}% chance to (Poison|inflict Bleeding|Ignite)(?: on Hit)?$"
            )),
            r(format!(
                r"^{n}% (increased|reduced) (?:Ignite Magnitude|Magnitude of (Ignite|Poison|Bleeding|Ailments|Damaging Ailments) you inflict)$"
            )),
            r(format!(
                r"^{n}% (more|less) Magnitude of (Ignite|Poison|Bleeding) inflicted (?:with|by) Supported Skills$"
            )),
            r(format!(
                r"^(?:Supported Skills have )?{n}% (increased|reduced|more|less) (Poison|Bleeding|Ignite) Duration(?: on Enemies)?$"
            )),
            r(format!(
                r"^{n}% (increased|reduced|more|less) Duration of (Damaging Ailments on Enemies|Ignite, Shock and Chill on Enemies|Supported Skills and Damaging Ailments they inflict)$"
            )),
            r(format!(
                r"^(Ignites|Bleeding|Poisons?|Damaging Ailments) (?:you inflict |from Supported Skills )?deals? [Dd]amage {n}% faster$"
            )),
            r(
                r"^(?i:Supported (?:Skills|Attacks) cannot) (Ignite|Poison|cause Bleeding|inflict Bleeding|inflict Elemental Ailments|inflict Freeze, Shock or Ignite)$"
                    .to_string(),
            ),
        ]
    });
    let cle = |mot: &str| match mot {
        "Ignite" | "Ignites" => Some("ignite"),
        "Poison" | "Poisons" => Some("poison"),
        "Bleeding" | "inflict Bleeding" | "cause Bleeding" => Some("bleed"),
        _ => None,
    };
    let un = |stat: String, kind: TypeModificateur, v: f64| {
        Some(vec![Modificateur::new(stat, kind, v, source)])
    };
    if let Some(c) = m[0].captures(l) {
        return un(
            "ignite_chance".into(),
            genre(&c[2]),
            nombre(&c[1]) * signe(&c[2]),
        );
    }
    if let Some(c) = m[1].captures(l) {
        return un(
            stats::AILMENT_CHANCE.into(),
            genre(&c[2]),
            nombre(&c[1]) * signe(&c[2]),
        );
    }
    if let Some(c) = m[2].captures(l) {
        return un(
            format!("{}_chance", cle(&c[2])?),
            TypeModificateur::Added,
            nombre(&c[1]),
        );
    }
    if let Some(c) = m[3].captures(l) {
        let stat = match c.get(3).map(|x| x.as_str()) {
            None => "ignite_magnitude".to_string(),
            Some("Ailments" | "Damaging Ailments") => stats::AILMENT_MAGNITUDE.to_string(),
            Some(a) => format!("{}_magnitude", cle(a)?),
        };
        return un(stat, genre(&c[2]), nombre(&c[1]) * signe(&c[2]));
    }
    if let Some(c) = m[4].captures(l) {
        return un(
            format!("{}_magnitude", cle(&c[3])?),
            genre(&c[2]),
            nombre(&c[1]) * signe(&c[2]),
        );
    }
    if let Some(c) = m[5].captures(l) {
        return un(
            format!("{}_duration", cle(&c[3])?),
            genre(&c[2]),
            nombre(&c[1]) * signe(&c[2]),
        );
    }
    if let Some(c) = m[6].captures(l) {
        let stat = if c[3].starts_with("Ignite") {
            "ignite_duration".to_string()
        } else {
            stats::AILMENT_DURATION.to_string()
        };
        return un(stat, genre(&c[2]), nombre(&c[1]) * signe(&c[2]));
    }
    if let Some(c) = m[7].captures(l) {
        let v = nombre(&c[2]);
        let cles: Vec<&str> = match cle(&c[1]) {
            Some(k) => vec![k],
            None => vec!["ignite", "poison", "bleed"],
        };
        return Some(
            cles.into_iter()
                .map(|k| {
                    Modificateur::new(
                        format!("{k}_faster"),
                        TypeModificateur::Increased,
                        v,
                        source,
                    )
                })
                .collect(),
        );
    }
    if let Some(c) = m[8].captures(l) {
        let cles: Vec<&str> = match &c[1] {
            "inflict Elemental Ailments" | "inflict Freeze, Shock or Ignite" => vec!["ignite"],
            autre => vec![cle(autre)?],
        };
        return Some(
            cles.into_iter()
                .map(|k| {
                    Modificateur::new(
                        format!("{k}_chance"),
                        TypeModificateur::More,
                        -100.0,
                        source,
                    )
                })
                .collect(),
        );
    }
    static PILES: OnceLock<Regex> = OnceLock::new();
    let piles = PILES.get_or_init(|| {
        Regex::new(
            r"^Targets can be affected by \+(\d+) (?:of your Poisons|Poisons from Supported Skills) at the same time$",
        )
        .unwrap()
    });
    if let Some(c) = piles.captures(l) {
        return un(
            stats::POISON_STACKS.into(),
            TypeModificateur::Added,
            nombre(&c[1]),
        );
    }
    None
}

/// Traduit une ligne de mod (déjà affichée par le jeu ou par PoB).
pub fn traduire(ligne: &str, source: &str) -> Traduction {
    let l = nettoyer(ligne);
    if l.is_empty() || l.starts_with("Grants Skill:") {
        return Traduction::SansEffet;
    }
    if let Some(v) = ailment(&l, source) {
        return Traduction::Mods(v);
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
            Some("Attack") => stats::ATTACK_DAMAGE.to_string(),
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
        let stat = if &c[3] == "Attack" {
            stats::ATTACK_SPEED
        } else {
            stats::CAST_SPEED
        };
        return mods(vec![Modificateur::new(
            stat,
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
        let (smin, smax) = if &c[4] == "Attacks" {
            (t.stat_ajoutes_attaque_min(), t.stat_ajoutes_attaque_max())
        } else {
            (t.stat_ajoutes_min(), t.stat_ajoutes_max())
        };
        return mods(vec![
            Modificateur::new(smin, TypeModificateur::Added, nombre(&c[1]), source),
            Modificateur::new(smax, TypeModificateur::Added, nombre(&c[2]), source),
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
        match (&c[2], c.get(3).and_then(|t| type_degats(t.as_str()))) {
            ("", _) => niv.toutes = n,
            ("Spell ", _) => niv.sorts = n,
            ("Elemental Spell ", _) => niv.elementaires = n,
            ("Minion ", _) => niv.minions = n,
            (_, Some(t)) => niv.par_type[t.index()] = n,
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
        let stat = match (c.get(3).map(|x| x.as_str()), c.get(4).is_some()) {
            (None, true) => stats::HIT_DAMAGE.to_string(),
            (None, false) => stats::DAMAGE.to_string(),
            (Some("Spell"), _) => stats::SPELL_DAMAGE.to_string(),
            (Some("Elemental"), _) => stats::ELEMENTAL_DAMAGE.to_string(),
            (Some(t), _) => type_degats(t)
                .map(TypeDegats::stat_degats)
                .unwrap_or_default(),
        };
        let mut md = Modificateur::new(stat, genre(&c[2]), nombre(&c[1]) * signe(&c[2]), source);
        if let Some(p) = c.get(6) {
            md = md.sous_condition(ConditionModificateur::ManaMinPct(nombre(p.as_str())));
        }
        return mods(vec![md]);
    }
    if let Some(c) = m.support_stat.captures(&l) {
        let stat = match &c[3] {
            "Cast Speed" => stats::CAST_SPEED,
            "Attack Speed" => stats::ATTACK_SPEED,
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
    if let Some(c) = m.dot.captures(&l) {
        let stat = match c.get(3).and_then(|t| type_degats(t.as_str())) {
            Some(t) => t.stat_degats_dot(),
            None => stats::DAMAGE_OVER_TIME.to_string(),
        };
        return mods(vec![Modificateur::new(
            stat,
            TypeModificateur::Increased,
            nombre(&c[1]) * signe(&c[2]),
            source,
        )]);
    }
    if let Some(c) = m.support_dot.captures(&l) {
        return mods(vec![Modificateur::new(
            stats::DAMAGE_OVER_TIME,
            TypeModificateur::More,
            nombre(&c[1]) * signe(&c[2]),
            source,
        )]);
    }
    if let Some(c) = m.minion_degats.captures(&l) {
        return mods(vec![Modificateur::new(
            stats::MINION_DAMAGE,
            genre(&c[2]),
            nombre(&c[1]) * signe(&c[2]),
            source,
        )]);
    }
    if let Some(c) = m.minion_vitesse.captures(&l) {
        let v = nombre(&c[1]) * signe(&c[2]);
        let mut stats_v = Vec::new();
        if c[3].contains("Attack") {
            stats_v.push(stats::MINION_ATTACK_SPEED);
        }
        if c[3].contains("Cast") {
            stats_v.push(stats::MINION_CAST_SPEED);
        }
        return mods(
            stats_v
                .into_iter()
                .map(|s| Modificateur::new(s, TypeModificateur::Increased, v, source))
                .collect(),
        );
    }
    if let Some(c) = m.minion_critique.captures(&l) {
        let stat = if &c[3] == "Hit Chance" {
            stats::MINION_CRITICAL_HIT_CHANCE
        } else {
            stats::MINION_CRITICAL_DAMAGE_BONUS
        };
        return mods(vec![Modificateur::new(
            stat,
            TypeModificateur::Increased,
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
            Traduction::Niveaux(n) => n.pour_sort(&[TypeDegats::Foudre]),
            autre => panic!("{autre:?}"),
        };
        assert_eq!(niv("+1 to Level of all Spell Skills"), 1);
        assert_eq!(niv("{fractured}+3 to Level of all Spell Skills"), 3);
        assert_eq!(niv("+5 to Level of all Lightning Spell Skills"), 5);
        assert_eq!(niv("+1 to Level of all Skills"), 1);
        match traduire("+2 to Level of all Minion Skills", "t") {
            Traduction::Niveaux(n) => {
                assert_eq!(
                    (n.pour_minion(), n.pour_sort(&[TypeDegats::Foudre])),
                    (2, 0)
                )
            }
            autre => panic!("{autre:?}"),
        }
        let froid = match traduire("+2 to Level of all Cold Spell Skills", "t") {
            Traduction::Niveaux(n) => n,
            autre => panic!("{autre:?}"),
        };
        assert_eq!(froid.pour_sort(&[TypeDegats::Foudre]), 0);
        assert_eq!(froid.pour_sort(&[TypeDegats::Froid]), 2);
        assert_eq!(froid.pour_sort(&[TypeDegats::Feu, TypeDegats::Froid]), 2);
        assert_eq!(niv("+2 to Level of all Elemental Spell Skills"), 2);
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
    fn traduit_les_mods_d_attaque() {
        let un = |l: &str| match traduire(l, "t") {
            Traduction::Mods(v) => v,
            autre => panic!("{l} : {autre:?}"),
        };
        assert_eq!(
            un("10% increased Attack Damage")[0].stat,
            stats::ATTACK_DAMAGE
        );
        assert_eq!(
            un("12% increased Attack Speed")[0].stat,
            stats::ATTACK_SPEED
        );
        let a = un("Adds 3 to 6 Fire Damage to Attacks");
        assert_eq!(a[0].stat, TypeDegats::Feu.stat_ajoutes_attaque_min());
        let s = un("Adds 3 to 6 Fire Damage to Spells");
        assert_eq!(s[0].stat, TypeDegats::Feu.stat_ajoutes_min());
    }

    #[test]
    fn classe_le_reste() {
        for l in [
            "+119 to maximum Life",
            "+42% to Fire Resistance",
            "35% increased Movement Speed",
            "Minions have 30% increased maximum Life",
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

    #[test]
    fn traduit_les_mods_de_minions_et_de_degats_dans_le_temps() {
        let un = |l: &str| match traduire(l, "t") {
            Traduction::Mods(v) if v.len() == 1 => v[0].clone(),
            autre => panic!("{l} : {autre:?}"),
        };
        let m = un("Minions deal 20% increased Damage");
        assert_eq!((m.stat.as_str(), m.valeur), (stats::MINION_DAMAGE, 20.0));
        let m = un("Minions from Supported Skills deal 25% more Damage");
        assert_eq!(
            (m.stat.as_str(), m.kind),
            (stats::MINION_DAMAGE, TypeModificateur::More)
        );
        let m = un("Minions have 10% increased Attack Speed");
        assert_eq!(m.stat, stats::MINION_ATTACK_SPEED);
        match traduire("Minions have 8% increased Attack and Cast Speed", "t") {
            Traduction::Mods(v) => assert_eq!(v.len(), 2),
            autre => panic!("{autre:?}"),
        }
        let m = un("Minions have 30% increased Critical Hit Chance");
        assert_eq!(m.stat, stats::MINION_CRITICAL_HIT_CHANCE);
        let m = un("15% increased Damage over Time");
        assert_eq!(m.stat, stats::DAMAGE_OVER_TIME);
        let m = un("25% increased Chaos Damage over Time");
        assert_eq!(m.stat, TypeDegats::Chaos.stat_degats_dot());
        let m = un("Supported Skills deal 20% more non-Ailment Damage over Time");
        assert_eq!(
            (m.stat.as_str(), m.kind),
            (stats::DAMAGE_OVER_TIME, TypeModificateur::More)
        );
    }

    #[test]
    fn traduit_les_effets_de_supports_courants() {
        let un = |l: &str| match traduire(l, "t") {
            Traduction::Mods(v) if v.len() == 1 => v[0].clone(),
            autre => panic!("{l} : {autre:?}"),
        };
        let m = un("Supported Skills deal 25% less Damage with Hits");
        assert_eq!((m.stat.as_str(), m.valeur), (stats::HIT_DAMAGE, -25.0));
        assert_eq!(m.kind, TypeModificateur::More);
        let m = un("Supported Skills deal 30% more Physical Damage");
        assert_eq!(m.stat, TypeDegats::Physique.stat_degats());
        let m = un("Supported Skills Gain 10% of Damage as extra Chaos Damage");
        assert_eq!(m.stat, stats::stat_gain_tous(TypeDegats::Chaos));
        let m = un("Supported Skills Gain 15% of Cold Damage as Extra Chaos Damage");
        assert_eq!(
            m.stat,
            stats::stat_gain(TypeDegats::Froid, TypeDegats::Chaos)
        );
    }

    #[test]
    fn traduit_les_ailments() {
        let cas: &[(&str, &str, TypeModificateur, f64)] = &[
            (
                "39% increased Flammability Magnitude",
                "ignite_chance",
                TypeModificateur::Increased,
                39.0,
            ),
            (
                "Supported Skills have 150% more Flammability Magnitude",
                "ignite_chance",
                TypeModificateur::More,
                150.0,
            ),
            (
                "Supported Skills have 25% less Flammability Magnitude",
                "ignite_chance",
                TypeModificateur::More,
                -25.0,
            ),
            (
                "10% increased chance to inflict Ailments",
                "ailment_chance",
                TypeModificateur::Increased,
                10.0,
            ),
            (
                "Supported Skills have 100% more chance to inflict Ailments",
                "ailment_chance",
                TypeModificateur::More,
                100.0,
            ),
            (
                "13% chance to Poison on Hit",
                "poison_chance",
                TypeModificateur::Added,
                13.0,
            ),
            (
                "17% chance to inflict Bleeding on Hit",
                "bleed_chance",
                TypeModificateur::Added,
                17.0,
            ),
            (
                "Supported Skills have 50% chance to inflict Bleeding",
                "bleed_chance",
                TypeModificateur::Added,
                50.0,
            ),
            (
                "Supported Skills have 30% chance to Poison on Hit",
                "poison_chance",
                TypeModificateur::Added,
                30.0,
            ),
            (
                "17% increased Ignite Magnitude",
                "ignite_magnitude",
                TypeModificateur::Increased,
                17.0,
            ),
            (
                "18% increased Magnitude of Poison you inflict",
                "poison_magnitude",
                TypeModificateur::Increased,
                18.0,
            ),
            (
                "9% increased Magnitude of Bleeding you inflict",
                "bleed_magnitude",
                TypeModificateur::Increased,
                9.0,
            ),
            (
                "8% increased Magnitude of Ailments you inflict",
                "ailment_magnitude",
                TypeModificateur::Increased,
                8.0,
            ),
            (
                "4% increased Magnitude of Damaging Ailments you inflict",
                "ailment_magnitude",
                TypeModificateur::Increased,
                4.0,
            ),
            (
                "75% more Magnitude of Ignite inflicted with Supported Skills",
                "ignite_magnitude",
                TypeModificateur::More,
                75.0,
            ),
            (
                "100% more Magnitude of Bleeding inflicted with Supported Skills",
                "bleed_magnitude",
                TypeModificateur::More,
                100.0,
            ),
            (
                "8% increased Poison Duration",
                "poison_duration",
                TypeModificateur::Increased,
                8.0,
            ),
            (
                "6% increased Ignite Duration on Enemies",
                "ignite_duration",
                TypeModificateur::Increased,
                6.0,
            ),
            (
                "Supported Skills have 20% less Poison Duration",
                "poison_duration",
                TypeModificateur::More,
                -20.0,
            ),
            (
                "9% increased Duration of Damaging Ailments on Enemies",
                "ailment_duration",
                TypeModificateur::Increased,
                9.0,
            ),
            (
                "15% increased Duration of Ignite, Shock and Chill on Enemies",
                "ignite_duration",
                TypeModificateur::Increased,
                15.0,
            ),
            (
                "Ignites you inflict deal Damage 10% faster",
                "ignite_faster",
                TypeModificateur::Increased,
                10.0,
            ),
            (
                "Ignites from Supported Skills deal Damage 15% faster",
                "ignite_faster",
                TypeModificateur::Increased,
                15.0,
            ),
            (
                "Bleeding you inflict deals Damage 5% faster",
                "bleed_faster",
                TypeModificateur::Increased,
                5.0,
            ),
            (
                "Targets can be affected by +1 of your Poisons at the same time",
                "poison_stacks",
                TypeModificateur::Added,
                1.0,
            ),
            (
                "Targets can be affected by +1 Poisons from Supported Skills at the same time",
                "poison_stacks",
                TypeModificateur::Added,
                1.0,
            ),
            (
                "Supported Skills cannot Ignite",
                "ignite_chance",
                TypeModificateur::More,
                -100.0,
            ),
            (
                "Supported Attacks cannot cause Bleeding",
                "bleed_chance",
                TypeModificateur::More,
                -100.0,
            ),
            (
                "Supported Skills Cannot inflict Freeze, Shock or Ignite",
                "ignite_chance",
                TypeModificateur::More,
                -100.0,
            ),
        ];
        for (ligne, stat, kind, v) in cas {
            let m = un(ligne);
            assert_eq!(
                (m.stat.as_str(), m.kind, m.valeur),
                (*stat, *kind, *v),
                "{ligne}"
            );
        }
        match traduire("Damaging Ailments deal damage 10% faster", "t") {
            Traduction::Mods(v) => assert_eq!(v.len(), 3),
            autre => panic!("{autre:?}"),
        }
        // Conditionnelles : toujours sans effet.
        assert_eq!(
            traduire(
                "15% increased Magnitude of Bleeding you inflict with Critical Hits",
                "t"
            ),
            Traduction::SansEffet
        );
    }
}
