use super::objectif::{Class, Goal};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Option choisie par le joueur à la révélation : un mod voulu d'abord, puis un mod inutile, un palier
/// trop bas d'un voulu en dernier ; au hasard sans objectif. `None` sans option.
fn choisir(
    pool: &AffixPool,
    goal: Option<&Goal>,
    options: &[AffixIdx],
    rng: &mut impl Rng,
) -> Option<AffixIdx> {
    if options.is_empty() {
        return None;
    }
    let Some(goal) = goal else {
        return Some(options[rng.random_range(0..options.len())]);
    };
    let rang = |&i: &AffixIdx| match goal.classify(pool, i) {
        Class::Wanted(_) => 0,
        Class::Other => 1,
        Class::Blocked(_) => 2,
    };
    options.iter().min_by_key(|i| rang(i)).copied()
}

pub type AffixIdx = u16; // index dans AffixPool.affixes
pub type GroupId = u16; // deux affixes du même groupe ne peuvent pas coexister

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Slot {
    Prefix,
    Suffix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rarity {
    Normal,
    Magic,
    Rare,
}

impl Rarity {
    /// (max préfixes, max suffixes) d'une base ordinaire ; une base peut les décaler (voir `AffixPool::cap`).
    pub const fn cap(self) -> (u8, u8) {
        match self {
            Rarity::Normal => (0, 0),
            Rarity::Magic => (1, 1),
            Rarity::Rare => (3, 3),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Affix {
    pub id: String,
    pub name: String,   // "Sturdy"
    pub family: String, // libellé lisible du groupe, ex. "Vie maximale"
    pub text: String,   // gabarit avec plages, ex. "+(40-49) to maximum Life"
    pub group: GroupId,
    /// Sous-famille dans le groupe d'exclusion (même `type` de mod dans le jeu, ex. « +# to Level of all
    /// Spell Skills » vs « ... Minion Skills », qui s'excluent mais ne sont pas des tiers l'un de l'autre).
    /// Les tiers sont numérotés au sein de (groupe, famille).
    #[serde(default)]
    pub family_id: u16,
    pub slot: Slot,
    pub tier: u8,     // 1 = meilleur dans sa famille
    pub req_ilvl: u8, // niveau de modificateur (ilvl requis)
    pub weight: u32,  // poids de spawn résolu pour CETTE base
    pub tags: u64,    // bitmask de tags
    /// Domaine « desecrated » : jamais tirable par une monnaie normale (Transmute, Chaos, Exalt, ...),
    /// uniquement par `CurrencyKind::Desecrate`. Empêche par construction le bug qu'on avait repéré :
    /// laisser ces mods fuiter dans le pool normal fausserait silencieusement toutes les probabilités.
    #[serde(default)]
    pub desecrated: bool,
    /// Décalage du plafond (préfixes, suffixes) tant que ce mod est sur l'objet : stats
    /// `local_maximum_prefixes_allowed_+` / `local_maximum_suffixes_allowed_+` d'un mod (ex. Potent
    /// Liquid Contempt : préfixe « +1 Suffix Modifier allowed » = (0, 1)). (0, 0) pour un mod ordinaire.
    #[serde(default)]
    pub cap_shift: (i8, i8),
    /// Autres groupes d'exclusion du mod, en plus de `group` (Alloys hybrides : Swift Alloy sur anneau
    /// « vitesse d'attaque » qui prend aussi le groupe vitesse d'incantation, Transcendent Alloy qui prend
    /// Force, Dextérité et Intelligence). Deux mods s'excluent dès qu'ils partagent un groupe (règle de
    /// l'émulateur de Craft of Exile : familles en commun). Vide pour un mod ordinaire.
    #[serde(default)]
    pub extra_groups: Vec<GroupId>,
}

impl Affix {
    /// Tous les groupes d'exclusion du mod : `group` puis `extra_groups`.
    pub fn groups(&self) -> impl Iterator<Item = GroupId> + '_ {
        std::iter::once(self.group).chain(self.extra_groups.iter().copied())
    }

    /// Les deux mods partagent-ils un groupe d'exclusion (ne peuvent pas coexister) ?
    pub fn conflicts(&self, other: &Affix) -> bool {
        self.groups().any(|g| other.groups().any(|h| h == g))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mod {
    pub idx: AffixIdx,
    pub fractured: bool,
    /// Mod « crafted » : posé par une Essence, un Alloy ou une Liquid Emotion. Depuis le patch 0.5.0, un
    /// objet n'en porte qu'un à la fois (« items can only have 1 crafted modifier at a time » ; le jeu refuse
    /// une seconde Essence : « this item already has a crafted modifier »).
    pub crafted: bool,
    /// Mod révélé d'une Désécration (os abyssal) : mod d'une liche ou mod ordinaire choisi parmi les
    /// options du Well of Souls. Un objet n'en porte qu'un ; l'Omen of Light ne retire que lui.
    pub desecrated: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ItemState {
    pub rarity: Rarity,
    pub ilvl: u8,
    mods: [Mod; 6],
    len: u8,
}

impl ItemState {
    pub fn new(rarity: Rarity, ilvl: u8) -> Self {
        Self {
            rarity,
            ilvl,
            mods: [Mod::default(); 6],
            len: 0,
        }
    }
    pub fn mods(&self) -> &[Mod] {
        &self.mods[..self.len as usize]
    }
    pub fn len(&self) -> usize {
        self.len as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn has_fractured(&self) -> bool {
        self.mods().iter().any(|m| m.fractured)
    }
    pub fn has_crafted(&self) -> bool {
        self.mods().iter().any(|m| m.crafted)
    }
    pub fn push(&mut self, m: Mod) {
        debug_assert!(self.len < 6);
        if self.len < 6 {
            self.mods[self.len as usize] = m;
            self.len += 1;
        }
    }
    pub fn remove(&mut self, pos: usize) {
        let l = self.len as usize;
        self.mods.copy_within(pos + 1..l, pos);
        self.len -= 1;
    }
    pub fn set_fractured(&mut self, pos: usize) {
        self.mods[pos].fractured = true;
    }
}

// ───────────────────────── Monnaies ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CurrencyKind {
    Transmute,
    Augment,
    Regal,
    Alchemy,
    Exalt,
    Chaos,
    Annul,
    Fracture,
    /// Essence : transforme un objet Magique en Rare en ajoutant un affixe GARANTI (`Currency.target`),
    /// pas un tirage pondéré. Retenu uniquement si l'affixe cible respecte la place de slot disponible
    /// et n'entre pas en conflit de groupe avec un mod déjà présent (sinon `NotApplicable`).
    Essence,
    /// Désécration (os abyssal) puis révélation au Well of Souls, en une action : un côté est choisi
    /// (forcé par l'Omen of Sinistral / Dextral Necromancy, sinon au hasard parmi les côtés ouverts ;
    /// objet plein : un mod retiré d'abord), puis le joueur choisit un mod parmi trois options : 1, 2
    /// ou 3 mods de liche (80 / 15 / 5 %), le reste tiré dans le pool normal de ce côté (émulateur de
    /// Craft of Exile, `getRevealPicks`). Un objet portant déjà un mod Désécré ne peut pas l'être une
    /// deuxième fois.
    Desecrate,
}

/// Une « action de craft » : monnaie (éventuellement Greater/Perfect) + Omen éventuel.
/// Les règles exactes sont pilotées par la donnée ; ce noyau n'en code que la mécanique générique.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    pub id: String,
    pub label: String,
    pub kind: CurrencyKind,
    #[serde(default)]
    pub min_mod_level: u8,
    /// Omen « Sinistral/Dextral Exaltation » : force le slot de l'affixe ajouté.
    #[serde(default)]
    pub add_slot: Option<Slot>,
    /// Omen « Sinistral/Dextral Annulment/Erasure » : restreint le retrait à un slot.
    #[serde(default)]
    pub remove_slot: Option<Slot>,
    /// Affixe garanti pour `CurrencyKind::Essence` — résolu par base (dépend du pool), donc absent
    /// pour toutes les autres monnaies et ignoré si `kind != Essence`.
    #[serde(default)]
    pub target: Option<AffixIdx>,
    /// Second affixe garanti possible (Potent Liquid Ferocity/Contempt : un préfixe OU un suffixe) : le slot
    /// est tiré à 50/50, puis le mod remplace un affixe non fracturé de ce slot. `None` : seulement `target`.
    #[serde(default)]
    pub alt_target: Option<AffixIdx>,
    /// Autres mods possibles d'une Essence ou d'un Alloy (Essence of the Infinite : Force, Dextérité ou
    /// Intelligence) : le mod posé est tiré à chances égales entre `target` et ceux-ci, parmi ceux sans
    /// conflit de groupe avec l'objet (émulateur de Craft of Exile). Vide : seulement `target`.
    #[serde(default)]
    pub more_targets: Vec<AffixIdx>,
    /// Omen « the Sovereign/Liege/Blackblooded » : une des options de la révélation est un mod de cette
    /// liche (Ulaman / Amanamu / Kurgal). Ignoré pour tout autre `CurrencyKind`.
    #[serde(default)]
    pub require_tag: Option<u64>,
    /// Omen of Light : le retrait (Annulment) ne peut cibler qu'un affixe `desecrated`.
    #[serde(default)]
    pub remove_desecrated_only: bool,
    /// Omen of Whittling : le retrait cible TOUJOURS l'affixe tenu du niveau requis le plus bas
    /// (déterministe), pas un tirage uniforme parmi les candidats.
    #[serde(default)]
    pub remove_lowest_level: bool,
    /// Omen of Abyssal Echoes : la révélation tire un second jeu d'options (jusqu'à six au choix).
    #[serde(default)]
    pub reveal_twice: bool,
    /// `false` (Essence normale/Lesser/Greater) : Magique → Rare, ajoute l'affixe garanti.
    /// `true` (Essence Perfect / Corrupted, Alloy) : objet déjà Rare, retire un mod (du côté
    /// de l'affixe garanti si ce côté est plein, sinon au hasard) PUIS ajoute l'affixe garanti — jamais les deux comportements sur la même entrée, comme dans le vrai jeu.
    #[serde(default)]
    pub requires_rare: bool,
    /// Omen of Greater Exaltation / Greater Annulment : l'Exalted ajoute deux mods (le second seulement
    /// s'il reste une place), l'Annulment en retire deux (le second seulement s'il en reste un à retirer).
    /// Règle de l'émulateur de Craft of Exile (`rollAffixes(2)`, `annul` à deux retraits).
    #[serde(default)]
    pub double: bool,
    pub unit_cost: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DrawFilter {
    pub min_mod_level: u8,
    pub force_slot: Option<Slot>,
    /// `false` (monnaies normales) : exclut les affixes `desecrated`. `true` (Désécration) : ne tire
    /// QUE parmi eux — jamais les deux pools mélangées.
    pub require_desecrated: bool,
    /// Omen « the Sovereign/Liege/Blackblooded » : restreint le tirage à un sous-pool via bitmask de tag.
    pub require_tag: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Applied,
    NotApplicable,
}

// ───────────────────────── Pool d'affixes ─────────────────────────

#[derive(Clone, Debug)]
pub struct AffixPool {
    pub affixes: Vec<Affix>,
    /// Décalage du nombre de préfixes / suffixes autorisés propre à la base (implicites
    /// `local_maximum_prefixes_allowed_+` / `local_maximum_suffixes_allowed_+`, ex. Dusk Ring : +1 / -1).
    pub cap_delta: (i8, i8),
    /// (max préfixes, max suffixes) d'un objet Rare de cette classe avant décalage : 3/3 en général,
    /// 2/2 pour un joyau (Path of Building, Item.lua : `affixLimit` 4 pour un Rare de type Jewel).
    pub rare_cap: (u8, u8),
}

impl Default for AffixPool {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl AffixPool {
    pub fn new(affixes: Vec<Affix>) -> Self {
        Self {
            affixes,
            cap_delta: (0, 0),
            rare_cap: Rarity::Rare.cap(),
        }
    }

    /// Affixes que le niveau de mod minimum d'une monnaie Greater / Perfect écarte, à cet ilvl
    /// (`true` = écarté). Règle de Craft of Exile (émulateur 0.5.5.5) : un tier sous le minimum est
    /// écarté, sauf s'il est le meilleur tier accessible de sa famille, qui reste tirable.
    pub fn sous_niveau_min(&self, ilvl: u8, min: u8) -> Vec<bool> {
        if min == 0 {
            return vec![false; self.affixes.len()];
        }
        let mut meilleur: std::collections::HashMap<(GroupId, u16, bool), u8> =
            std::collections::HashMap::new();
        for a in self.affixes.iter().filter(|a| a.req_ilvl <= ilvl) {
            let e = meilleur
                .entry((a.group, a.family_id, a.desecrated))
                .or_insert(a.req_ilvl);
            *e = (*e).max(a.req_ilvl);
        }
        self.affixes
            .iter()
            .map(|a| {
                a.req_ilvl < min
                    && meilleur.get(&(a.group, a.family_id, a.desecrated)) != Some(&a.req_ilvl)
            })
            .collect()
    }

    /// (max préfixes, max suffixes) pour cette base à cette rareté : plafond de la rareté (borné par
    /// celui d'un Rare de la classe, ex. joyau 2/2) décalé par l'implicite de la base. Un objet Normal
    /// reste sans affixe.
    pub fn cap(&self, rarity: Rarity) -> (u8, u8) {
        let (p, s) = rarity.cap();
        if rarity == Rarity::Normal {
            return (p, s);
        }
        let (p, s) = (p.min(self.rare_cap.0), s.min(self.rare_cap.1));
        let shift = |base: u8, d: i8| (base as i16 + d as i16).clamp(0, 6) as u8;
        (shift(p, self.cap_delta.0), shift(s, self.cap_delta.1))
    }

    /// Plafond effectif de CET objet : celui de la base à sa rareté, décalé par les mods qu'il porte
    /// (`Affix::cap_shift`). Un objet peut dépasser ce plafond (mod décaleur retiré après coup) : il n'a
    /// alors simplement plus de place dans ce slot.
    pub fn cap_of(&self, item: &ItemState) -> (u8, u8) {
        let (p, s) = self.cap(item.rarity);
        if item.rarity == Rarity::Normal {
            return (p, s);
        }
        let (dp, ds) = item.mods().iter().fold((0i16, 0i16), |(a, b), m| {
            let c = self.affixes[m.idx as usize].cap_shift;
            (a + c.0 as i16, b + c.1 as i16)
        });
        (
            (p as i16 + dp).clamp(0, 6) as u8,
            (s as i16 + ds).clamp(0, 6) as u8,
        )
    }

    /// Tirages d'un Orb of Alchemy : 4 mods au hasard ; avec l'Omen of Sinistral / Dextral Alchemy
    /// (`add_slot`), le nombre maximal de mods de ce côté, puis le reste des 4 de l'autre côté (émulateur
    /// de Craft of Exile, `poe2_alchemy`).
    pub fn alchemy_draws(&self, slot: Option<Slot>) -> [(Option<Slot>, u8); 2] {
        match slot {
            None => [(None, 4), (None, 0)],
            Some(s) => {
                let (p, q) = self.cap(Rarity::Rare);
                let max = match s {
                    Slot::Prefix => p,
                    Slot::Suffix => q,
                }
                .min(4);
                let autre = match s {
                    Slot::Prefix => Slot::Suffix,
                    Slot::Suffix => Slot::Prefix,
                };
                [(Some(s), max), (Some(autre), 4 - max)]
            }
        }
    }

    /// Reste-t-il une place de préfixe ou de suffixe sur cet objet (6 affixes au plus) ?
    pub fn has_room(&self, item: &ItemState) -> bool {
        let (cap_p, cap_s) = self.cap_of(item);
        item.len() < 6
            && (self.count(item, Slot::Prefix) < cap_p || self.count(item, Slot::Suffix) < cap_s)
    }

    /// Nombre total d'affixes autorisés à cette rareté (objet « plein »).
    pub fn max_mods(&self, rarity: Rarity) -> usize {
        let (p, s) = self.cap(rarity);
        (p + s).min(6) as usize
    }

    pub fn has_desecrated(&self, item: &ItemState) -> bool {
        item.mods()
            .iter()
            .any(|m| m.desecrated || self.affixes[m.idx as usize].desecrated)
    }

    pub fn count(&self, item: &ItemState, slot: Slot) -> u8 {
        item.mods()
            .iter()
            .filter(|m| self.affixes[m.idx as usize].slot == slot)
            .count() as u8
    }

    /// Règle d'éligibilité d'un affixe au prochain tirage, partagée par [`Self::draw`] et
    /// [`Self::distribution`].
    fn eligibility<'a>(
        &'a self,
        item: &'a ItemState,
        f: &'a DrawFilter,
    ) -> impl Fn(usize, &Affix) -> bool + 'a {
        let (cap_p, cap_s) = self.cap_of(item);
        let open_p = self.count(item, Slot::Prefix) < cap_p;
        let open_s = self.count(item, Slot::Suffix) < cap_s;
        let base = self.eligibility_sans_plafond(item, f);
        move |i: usize, a: &Affix| -> bool {
            let open = match a.slot {
                Slot::Prefix => open_p,
                Slot::Suffix => open_s,
            };
            open && base(i, a)
        }
    }

    /// Probabilité exacte de chaque affixe au prochain tirage (même règle que [`Self::draw`]),
    /// dans l'ordre du pool. Vide si aucun affixe n'est tirable.
    pub fn distribution(&self, item: &ItemState, f: &DrawFilter) -> Vec<(AffixIdx, f64)> {
        let eligible = self.eligibility(item, f);
        let total: u64 = self
            .affixes
            .iter()
            .enumerate()
            .filter(|&(i, a)| eligible(i, a))
            .map(|(_, a)| a.weight as u64)
            .sum();
        if total == 0 {
            return Vec::new();
        }
        self.affixes
            .iter()
            .enumerate()
            .filter(|&(i, a)| eligible(i, a))
            .map(|(i, a)| (i as AffixIdx, a.weight as f64 / total as f64))
            .collect()
    }

    /// Tirage pondéré exact parmi les affixes éligibles dans l'état courant.
    pub fn draw(&self, item: &ItemState, f: &DrawFilter, rng: &mut impl Rng) -> Option<AffixIdx> {
        let eligible = self.eligibility(item, f);

        let total: u64 = self
            .affixes
            .iter()
            .enumerate()
            .filter(|&(i, a)| eligible(i, a))
            .map(|(_, a)| a.weight as u64)
            .sum();
        if total == 0 {
            return None;
        }
        let mut roll = rng.random_range(0..total);
        for (i, a) in self.affixes.iter().enumerate() {
            if !eligible(i, a) {
                continue;
            }
            let w = a.weight as u64;
            if roll < w {
                return Some(i as AffixIdx);
            }
            roll -= w;
        }
        None
    }

    fn add_random(&self, item: &mut ItemState, f: &DrawFilter, rng: &mut impl Rng) {
        if let Some(idx) = self.draw(item, f, rng) {
            item.push(Mod {
                idx,
                fractured: false,
                crafted: false,
                desecrated: false,
            });
        }
    }

    /// Retire un affixe non fracturé (filtré par slot si Omen). `false` si aucun candidat.
    pub(crate) fn remove_random(
        &self,
        item: &mut ItemState,
        slot: Option<Slot>,
        desecrated_only: bool,
        lowest_level: bool,
        rng: &mut impl Rng,
    ) -> bool {
        let cands: Vec<usize> = item
            .mods()
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                !m.fractured
                    && slot.is_none_or(|s| self.affixes[m.idx as usize].slot == s)
                    && (!desecrated_only || m.desecrated || self.affixes[m.idx as usize].desecrated)
            })
            .map(|(i, _)| i)
            .collect();
        if cands.is_empty() {
            return false;
        }
        let pos = if lowest_level {
            // l'affixe TENU du niveau requis le plus bas ; égalité -> tirage au hasard (Craft of Exile)
            let niveau = |i: usize| self.affixes[item.mods()[i].idx as usize].req_ilvl;
            let bas = cands.iter().map(|&i| niveau(i)).min().unwrap();
            let ex: Vec<usize> = cands
                .iter()
                .copied()
                .filter(|&i| niveau(i) == bas)
                .collect();
            ex[rng.random_range(0..ex.len())]
        } else {
            cands[rng.random_range(0..cands.len())]
        };
        item.remove(pos);
        true
    }

    /// Options proposées à la révélation d'un mod Désécré de ce côté (émulateur de Craft of Exile,
    /// `getRevealPicks`) : 1, 2 ou 3 mods de liche (80 / 15 / 5 %), dont un de la liche de l'Omen s'il y en
    /// a un, puis le pool normal de ce côté jusqu'à trois options, tirages pondérés sans remise (un
    /// groupe tiré écarte ses autres paliers). Le niveau de mod minimum de l'os vaut pour les deux pools.
    pub fn reveal_options(
        &self,
        item: &ItemState,
        c: &Currency,
        cote: Slot,
        rng: &mut impl Rng,
    ) -> Vec<AffixIdx> {
        const OPTIONS: usize = 3;
        let u: f64 = rng.random();
        let liches = if u < 0.8 {
            1
        } else if u < 0.95 {
            2
        } else {
            3
        };
        let filtre = |desecre: bool, tag: Option<u64>| DrawFilter {
            min_mod_level: c.min_mod_level,
            force_slot: Some(cote),
            require_desecrated: desecre,
            require_tag: tag,
        };
        let mut pris: Vec<AffixIdx> = Vec::new();
        // Chaque tirage (Omen, liche, pool normal) part de son propre pool : une option tirée n'écarte
        // que les autres paliers de son mod dans le même tirage (`filterRollablePoolForModTiers` de
        // l'émulateur), pas les mods d'un autre tirage qui partagent son groupe (un mod de liche
        // Strength and Dexterity n'écarte pas la Dexterity du pool normal).
        let tirer = |f: DrawFilter, n: usize, pris: &mut Vec<AffixIdx>, rng: &mut _| {
            // l'objet est rendu « ouvert » de ce côté : la révélation ignore les plafonds
            let mut ouvert = *item;
            ouvert.rarity = Rarity::Rare;
            let eligible = self.eligibility_sans_plafond(&ouvert, &f);
            let debut = pris.len();
            while pris.len() < n {
                let cand: Vec<(usize, u64)> = self
                    .affixes
                    .iter()
                    .enumerate()
                    .filter(|&(i, a)| {
                        eligible(i, a)
                            && !pris.contains(&(i as AffixIdx))
                            && !pris[debut..].iter().any(|&p| {
                                let b = &self.affixes[p as usize];
                                (b.group, b.family_id, b.desecrated)
                                    == (a.group, a.family_id, a.desecrated)
                            })
                    })
                    .map(|(i, a)| (i, a.weight as u64))
                    .collect();
                let total: u64 = cand.iter().map(|c| c.1).sum();
                if total == 0 {
                    return;
                }
                let mut roll = Rng::random_range(rng, 0..total);
                for (i, w) in cand {
                    if roll < w {
                        pris.push(i as AffixIdx);
                        break;
                    }
                    roll -= w;
                }
            }
        };
        if let Some(t) = c.require_tag {
            tirer(filtre(true, Some(t)), 1, &mut pris, rng);
        }
        tirer(filtre(true, None), liches, &mut pris, rng);
        tirer(filtre(false, None), OPTIONS, &mut pris, rng);
        pris
    }

    /// Comme [`Self::eligibility`], sans la règle de place libre (côté déjà choisi par l'appelant).
    fn eligibility_sans_plafond<'a>(
        &'a self,
        item: &'a ItemState,
        f: &'a DrawFilter,
    ) -> impl Fn(usize, &Affix) -> bool + 'a {
        let mut held = [GroupId::MAX; 24];
        for (i, g) in item
            .mods()
            .iter()
            .flat_map(|m| self.affixes[m.idx as usize].groups())
            .take(held.len())
            .enumerate()
        {
            held[i] = g;
        }
        let sous_min = self.sous_niveau_min(item.ilvl, f.min_mod_level);
        move |i: usize, a: &Affix| -> bool {
            a.weight > 0
                && a.desecrated == f.require_desecrated
                && a.req_ilvl <= item.ilvl
                && !sous_min[i]
                && f.require_tag.is_none_or(|t| a.tags & t != 0)
                && f.force_slot.is_none_or(|s| s == a.slot)
                && !a.groups().any(|g| held.contains(&g))
        }
    }

    pub fn apply(&self, item: &mut ItemState, c: &Currency, rng: &mut impl Rng) -> Outcome {
        self.apply_pour(item, c, rng, None)
    }

    /// Comme [`Self::apply`] ; `goal` guide le choix du joueur à la révélation d'une Désécration (un mod
    /// voulu s'il y en a un parmi les options, sinon un mod inutile plutôt qu'un palier trop bas d'un
    /// voulu). Sans objectif, l'option est prise au hasard.
    pub fn apply_pour(
        &self,
        item: &mut ItemState,
        c: &Currency,
        rng: &mut impl Rng,
        goal: Option<&Goal>,
    ) -> Outcome {
        use CurrencyKind::*;
        let f = DrawFilter {
            min_mod_level: c.min_mod_level,
            force_slot: c.add_slot,
            ..Default::default()
        };
        let n = item.len();
        match c.kind {
            Transmute if item.rarity == Rarity::Normal => {
                item.rarity = Rarity::Magic;
                self.add_random(item, &f, rng);
            }
            Augment if item.rarity == Rarity::Magic && self.has_room(item) => {
                self.add_random(item, &f, rng)
            }
            Regal if item.rarity == Rarity::Magic => {
                item.rarity = Rarity::Rare;
                self.add_random(item, &f, rng);
            }
            // Normal ou Magique (patch 0.3.1 : « When used on Magic items the original modifiers are not retained »)
            Alchemy if item.rarity != Rarity::Rare => {
                *item = ItemState::new(Rarity::Rare, item.ilvl);
                for (slot, n) in self.alchemy_draws(c.add_slot) {
                    let f = DrawFilter {
                        force_slot: slot,
                        ..f
                    };
                    for _ in 0..n {
                        self.add_random(item, &f, rng);
                    }
                }
            }
            Exalt if item.rarity == Rarity::Rare && self.has_room(item) => {
                self.add_random(item, &f, rng);
                if c.double && self.has_room(item) {
                    self.add_random(item, &f, rng);
                }
            }
            Chaos if item.rarity == Rarity::Rare => {
                // retrait PUIS ajout : le pool du tirage est calculé après le retrait
                if !self.remove_random(
                    item,
                    c.remove_slot,
                    c.remove_desecrated_only,
                    c.remove_lowest_level,
                    rng,
                ) {
                    return Outcome::NotApplicable;
                }
                self.add_random(item, &f, rng);
            }
            Annul if item.rarity != Rarity::Normal => {
                if !self.remove_random(
                    item,
                    c.remove_slot,
                    c.remove_desecrated_only,
                    c.remove_lowest_level,
                    rng,
                ) {
                    return Outcome::NotApplicable;
                }
                if c.double {
                    self.remove_random(
                        item,
                        c.remove_slot,
                        c.remove_desecrated_only,
                        c.remove_lowest_level,
                        rng,
                    );
                }
            }
            Fracture if item.rarity == Rarity::Rare && n >= 4 && !item.has_fractured() => {
                let pos = rng.random_range(0..n);
                item.set_fractured(pos);
            }
            // un seul mod crafted par objet (0.5.0) : l'Essence est refusée tant qu'il en reste un
            Essence
                if ((item.rarity == Rarity::Magic && !c.requires_rare)
                    || (item.rarity == Rarity::Rare && c.requires_rare))
                    && !item.has_crafted() =>
            {
                let Some(target) = c.target else {
                    return Outcome::NotApplicable;
                };
                // affixe garanti sans place ou en conflit de groupe : l'objet reste tel quel
                let before = *item;
                let groupe_pris = |item: &ItemState, t: AffixIdx| {
                    let a = &self.affixes[t as usize];
                    item.mods()
                        .iter()
                        .any(|m| self.affixes[m.idx as usize].conflicts(a))
                };
                let place = |item: &ItemState, t: AffixIdx| {
                    let mut rare = *item;
                    rare.rarity = Rarity::Rare;
                    let (cap_p, cap_s) = self.cap_of(&rare);
                    match self.affixes[t as usize].slot {
                        Slot::Prefix => self.count(item, Slot::Prefix) < cap_p,
                        Slot::Suffix => self.count(item, Slot::Suffix) < cap_s,
                    }
                };
                let pick = if let Some(alt) = c.alt_target {
                    // un préfixe OU un suffixe (Potent Liquid Ferocity/Contempt) : le slot est tiré à 50/50, puis le
                    // mod remplace un affixe non fracturé de CE slot (règle donnée par Max)
                    let pick = if rng.random_bool(0.5) { alt } else { target };
                    self.remove_random(
                        item,
                        Some(self.affixes[pick as usize].slot),
                        false,
                        false,
                        rng,
                    );
                    pick
                } else {
                    // Essence ou Alloy (émulateur de Craft of Exile, `poe2_essence`) : un mod tiré à chances
                    // égales parmi les mods possibles sans conflit de groupe avec l'objet ; refusée si aucun
                    let cands: Vec<AffixIdx> = std::iter::once(target)
                        .chain(c.more_targets.iter().copied())
                        .filter(|&t| !groupe_pris(item, t))
                        .collect();
                    let un_parmi = |v: &[AffixIdx], rng: &mut _| match v.len() {
                        1 => v[0],
                        n => v[Rng::random_range(rng, 0..n)],
                    };
                    if cands.is_empty() {
                        return Outcome::NotApplicable;
                    }
                    let mut pick = un_parmi(&cands, rng);
                    if c.requires_rare {
                        // Essence Perfect / Corrupted, Alloy : le mod retiré est pris du côté du mod tiré si ce
                        // côté est plein, sinon au hasard parmi tous les mods
                        let a = &self.affixes[pick as usize];
                        let (cap_p, cap_s) = self.cap_of(item);
                        let plein = match a.slot {
                            Slot::Prefix => self.count(item, Slot::Prefix) >= cap_p,
                            Slot::Suffix => self.count(item, Slot::Suffix) >= cap_s,
                        };
                        // Omen of Sinistral / Dextral Crystallisation : retrait de ce côté seulement
                        let cote = c.remove_slot.or(plein.then_some(a.slot));
                        if !self.remove_random(item, cote, false, false, rng) {
                            return Outcome::NotApplicable;
                        }
                    }
                    // côté du mod tiré sans place : nouveau tirage parmi les mods possibles qui en ont
                    if !place(item, pick) && cands.len() > 1 {
                        let ok: Vec<AffixIdx> =
                            cands.iter().copied().filter(|&t| place(item, t)).collect();
                        if !ok.is_empty() {
                            pick = un_parmi(&ok, rng);
                        }
                    }
                    pick
                };
                if !place(item, pick) || groupe_pris(item, pick) {
                    *item = before;
                    return Outcome::NotApplicable;
                }
                item.rarity = Rarity::Rare;
                item.push(Mod {
                    idx: pick,
                    fractured: false,
                    crafted: true,
                    desecrated: false,
                });
            }
            Desecrate if item.rarity == Rarity::Rare && !self.has_desecrated(item) => {
                let before = *item;
                let (cap_p, cap_s) = self.cap_of(item);
                let ouvert = |item: &ItemState, s: Slot| match s {
                    Slot::Prefix => self.count(item, Slot::Prefix) < cap_p,
                    Slot::Suffix => self.count(item, Slot::Suffix) < cap_s,
                };
                // Omen of Sinistral / Dextral Necromancy : ce côté, vidé d'un mod s'il est plein ; sinon objet
                // plein : un mod retiré au hasard (émulateur de Craft of Exile, `poe2_desecrate`)
                if let Some(cote) = c.add_slot
                    && !ouvert(item, cote)
                    && !self.remove_random(item, Some(cote), false, false, rng)
                {
                    return Outcome::NotApplicable;
                }
                if !self.has_room(item) && !self.remove_random(item, None, false, false, rng) {
                    return Outcome::NotApplicable;
                }
                let cote = match (
                    c.add_slot,
                    ouvert(item, Slot::Prefix),
                    ouvert(item, Slot::Suffix),
                ) {
                    (Some(s), _, _) if ouvert(item, s) => s,
                    (_, true, true) if rng.random_bool(0.5) => Slot::Prefix,
                    (_, true, true) => Slot::Suffix,
                    (_, true, false) => Slot::Prefix,
                    (_, false, true) => Slot::Suffix,
                    _ => {
                        *item = before;
                        return Outcome::NotApplicable;
                    }
                };
                let mut options = self.reveal_options(item, c, cote, rng);
                if c.reveal_twice {
                    for o in self.reveal_options(item, c, cote, rng) {
                        if !options.contains(&o) {
                            options.push(o);
                        }
                    }
                }
                let Some(idx) = choisir(self, goal, &options, rng) else {
                    // aucun mod possible de ce côté : l'objet reste tel quel, jamais amputé du mod retiré
                    *item = before;
                    return Outcome::NotApplicable;
                };
                item.push(Mod {
                    idx,
                    fractured: false,
                    crafted: false,
                    desecrated: true,
                });
            }
            _ => return Outcome::NotApplicable,
        }
        Outcome::Applied
    }
}
