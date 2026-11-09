use super::moteur::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAX_WANTED: usize = 6;

/// « Ce groupe d'affixes, au tier `max_tier` ou mieux ».
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WantedAffix {
    pub group: GroupId,
    /// sous-famille visée dans le groupe (voir `Affix::family_id`) ; les autres familles du groupe bloquent
    #[serde(default)]
    pub family: u16,
    pub max_tier: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Affixe accepté pour le k-ième voulu.
    Wanted(usize),
    /// Même groupe que le k-ième voulu mais tier insuffisant : occupe le groupe (bloque).
    Blocked(usize),
    Other,
}

#[derive(Clone, Debug)]
pub struct Goal {
    pub wanted: Vec<WantedAffix>,
    pub slots: Vec<Slot>,
    by_group: HashMap<GroupId, usize>,
    accepted: Vec<Vec<bool>>, // accepted[k][affix_idx]
}

impl Goal {
    pub fn new(pool: &AffixPool, wanted: &[WantedAffix]) -> Result<Self, String> {
        Self::with_extra(pool, wanted, (0, 0))
    }

    /// `extra` : places de préfixe / suffixe qu'un mod décaleur garanti peut ouvrir en plus du plafond de
    /// la base (Potent Liquid Contempt : « +1 Suffix/Prefix Modifier allowed »). Le solveur dira ensuite
    /// si l'objectif est réellement atteignable.
    pub fn with_extra(
        pool: &AffixPool,
        wanted: &[WantedAffix],
        extra: (u8, u8),
    ) -> Result<Self, String> {
        if wanted.is_empty() || wanted.len() > MAX_WANTED {
            return Err(format!(
                "l'objectif doit contenir 1 à {MAX_WANTED} affixes (reçu {})",
                wanted.len()
            ));
        }
        // Chaque voulu occupe une « zone » : tous les groupes d'exclusion de son mod (un seul pour un
        // mod ordinaire ; plusieurs pour un Alloy hybride, ex. Transcendent Alloy : Force, Dextérité et
        // Intelligence). Tout mod qui touche la zone sans être accepté la bloque.
        let mut by_group = HashMap::new();
        let mut slots = Vec::new();
        for (k, w) in wanted.iter().enumerate() {
            let a = pool
                .affixes
                .iter()
                .find(|a| a.group == w.group && a.family_id == w.family)
                .ok_or_else(|| format!("groupe {} absent du pool de cette base", w.group))?;
            for g in a.groups() {
                if by_group.insert(g, k).is_some() {
                    return Err(
                        "deux affixes voulus s'excluent (même groupe d'exclusion) : objectif impossible"
                            .into(),
                    );
                }
            }
            slots.push(a.slot);
        }
        let p = slots.iter().filter(|s| **s == Slot::Prefix).count();
        let s = slots.len() - p;
        let (cap_p, cap_s) = pool.cap(Rarity::Rare);
        let (cap_p, cap_s) = ((cap_p + extra.0).min(6), (cap_s + extra.1).min(6));
        if p > cap_p as usize || s > cap_s as usize {
            return Err(format!(
                "plus de {cap_p} préfixes ou {cap_s} suffixes voulus sur cette base : objectif impossible"
            ));
        }
        let accepted = wanted
            .iter()
            .map(|w| {
                pool.affixes
                    .iter()
                    .map(|a| a.group == w.group && a.family_id == w.family && a.tier <= w.max_tier)
                    .collect()
            })
            .collect();
        Ok(Self {
            wanted: wanted.to_vec(),
            slots,
            by_group,
            accepted,
        })
    }

    pub fn len(&self) -> usize {
        self.wanted.len()
    }
    pub fn is_empty(&self) -> bool {
        self.wanted.is_empty()
    }

    #[inline]
    pub fn classify(&self, pool: &AffixPool, idx: AffixIdx) -> Class {
        let a = &pool.affixes[idx as usize];
        let zone = if a.extra_groups.is_empty() {
            self.by_group.get(&a.group).copied()
        } else {
            a.groups().find_map(|g| self.by_group.get(&g).copied())
        };
        match zone {
            None => Class::Other,
            Some(k) if self.accepted[k][idx as usize] => Class::Wanted(k),
            Some(k) => Class::Blocked(k),
        }
    }

    /// Zones des voulus que le mod touche (par un de ses groupes d'exclusion), sans doublon. Plus d'une
    /// seulement pour un mod à plusieurs groupes (ex. Transcendent Alloy face à Force ET Dextérité voulues).
    pub fn zones(&self, pool: &AffixPool, idx: AffixIdx) -> Vec<usize> {
        let mut v: Vec<usize> = Vec::new();
        for g in pool.affixes[idx as usize].groups() {
            if let Some(&k) = self.by_group.get(&g)
                && !v.contains(&k)
            {
                v.push(k);
            }
        }
        v
    }

    pub fn group_index(&self, g: GroupId) -> Option<usize> {
        self.by_group.get(&g).copied()
    }

    #[inline]
    pub fn is_met(&self, item: &ItemState) -> bool {
        self.accepted
            .iter()
            .all(|s| item.mods().iter().any(|m| s[m.idx as usize]))
    }
}
