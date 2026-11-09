//! Gains par map : butin saisi par le joueur, valorisé aux prix du marché.
//!
//! `Client.txt` ne note pas le butin ramassé et l'API des réserves n'est pas ouverte pour PoE 2 :
//! le joueur compte lui-même, après chaque map, les monnaies ramassées ([`OBJETS`]), ajoute la
//! valeur du reste (« autre butin », en Exalted) et la dépense de la map (Waystone, tablettes).
//! Décision de Max du 09/11/2026 : compteurs rapides par map, prix poe.ninja de Forge.
//!
//! Tout est en Exalted Orbs. Le module ne fait aucune entrée-sortie : l'appli lui passe les
//! quantités et la table de prix (identifiant -> prix d'une unité en Exalted).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::session::Bilan;

/// Monnaies proposées en compteurs, avec leur nom en jeu. Les identifiants sont ceux de la table de
/// prix de Forge (relevé poe.ninja, sinon valeurs indicatives).
pub const OBJETS: [(&str, &str); 12] = [
    ("divine", "Divine Orb"),
    ("annul", "Orb of Annulment"),
    ("chaos", "Chaos Orb"),
    ("exalt", "Exalted Orb"),
    ("regal", "Regal Orb"),
    ("alchemy", "Orb of Alchemy"),
    ("augment", "Orb of Augmentation"),
    ("transmute", "Orb of Transmutation"),
    ("exalt_greater", "Greater Exalted Orb"),
    ("chaos_greater", "Greater Chaos Orb"),
    ("regal_greater", "Greater Regal Orb"),
    ("fracture", "Fracturing Orb"),
];

/// Quantité maximale d'une monnaie dans une map (borne de saisie).
pub const QUANTITE_MAX: u32 = 100_000;
/// Montant maximal saisi en Exalted (autre butin, dépense).
pub const MONTANT_MAX: f64 = 10_000_000.0;
/// Durée minimale de session pour annoncer un gain par heure (comme le rythme).
const DUREE_MIN_PAR_HEURE: i64 = 60;

fn est_objet(id: &str) -> bool {
    OBJETS.iter().any(|(k, _)| *k == id)
}

fn montant(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, MONTANT_MAX)
    } else {
        0.0
    }
}

/// Butin d'une map, saisi par le joueur.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Butin {
    /// Monnaies ramassées (identifiant de [`OBJETS`] -> quantité).
    pub quantites: BTreeMap<String, u32>,
    /// Valeur du reste du butin (objets, fragments…), en Exalted.
    pub autre: f64,
    /// Dépense de la map (Waystone, tablettes…), en Exalted.
    pub depense: f64,
}

impl Butin {
    /// Nettoie la saisie : identifiants inconnus et quantités nulles écartés, montants bornés.
    pub fn normalise(mut self) -> Self {
        self.quantites.retain(|id, q| est_objet(id) && *q > 0);
        for q in self.quantites.values_mut() {
            *q = (*q).min(QUANTITE_MAX);
        }
        self.autre = montant(self.autre);
        self.depense = montant(self.depense);
        self
    }

    pub fn est_vide(&self) -> bool {
        self.quantites.is_empty() && self.autre == 0.0 && self.depense == 0.0
    }

    /// Valeur du butin aux prix donnés.
    pub fn valeur(&self, prix: &BTreeMap<String, f64>) -> ValeurButin {
        let mut brut = self.autre;
        let mut sans_prix = Vec::new();
        for (id, q) in &self.quantites {
            match prix.get(id).filter(|p| p.is_finite() && **p >= 0.0) {
                Some(p) => brut += p * f64::from(*q),
                None => sans_prix.push(id.clone()),
            }
        }
        ValeurButin {
            brut,
            depense: self.depense,
            net: brut - self.depense,
            sans_prix,
        }
    }
}

/// Valeur d'un butin, en Exalted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValeurButin {
    pub brut: f64,
    pub depense: f64,
    /// Brut moins la dépense.
    pub net: f64,
    /// Monnaies saisies sans prix connu (comptées pour zéro).
    pub sans_prix: Vec<String>,
}

/// Butins de la session, par position de la map dans [`Bilan::maps`] (la liste ne fait que
/// s'allonger pendant une session).
pub type ButinsSession = BTreeMap<usize, Butin>;

/// Gains d'une session, en Exalted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BilanGains {
    /// Valeur de chaque map, dans l'ordre de [`Bilan::maps`] (absente sans saisie).
    pub par_map: Vec<Option<ValeurButin>>,
    pub brut: f64,
    pub depense: f64,
    pub net: f64,
    /// Maps dont le butin a été saisi.
    pub maps_saisies: u32,
    /// Gain net moyen d'une map saisie (absent sans saisie).
    pub net_par_map: Option<f64>,
    /// Gain net par heure de session (absent sous une minute ou sans saisie).
    pub net_par_heure: Option<f64>,
}

/// Valorise les butins d'une session. Un butin rattaché à une map absente du bilan est ignoré.
pub fn bilan_gains(
    bilan: &Bilan,
    butins: &ButinsSession,
    prix: &BTreeMap<String, f64>,
) -> BilanGains {
    let mut g = BilanGains {
        par_map: vec![None; bilan.maps.len()],
        ..BilanGains::default()
    };
    for (i, butin) in butins {
        if *i >= bilan.maps.len() || butin.est_vide() {
            continue;
        }
        let v = butin.valeur(prix);
        g.brut += v.brut;
        g.depense += v.depense;
        g.net += v.net;
        g.maps_saisies += 1;
        g.par_map[*i] = Some(v);
    }
    if g.maps_saisies > 0 {
        g.net_par_map = Some(g.net / f64::from(g.maps_saisies));
        let total = bilan.temps.total();
        g.net_par_heure = (total >= DUREE_MIN_PAR_HEURE).then(|| g.net * 3_600.0 / total as f64);
    }
    g
}

/// Prix des [`OBJETS`] pris dans une table plus large (instantané gardé avec la session).
pub fn prix_objets(prix: &BTreeMap<String, f64>) -> BTreeMap<String, f64> {
    OBJETS
        .iter()
        .filter_map(|(id, _)| prix.get(*id).map(|p| ((*id).to_string(), *p)))
        .filter(|(_, p)| p.is_finite() && *p >= 0.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::SuiviSession;

    fn prix() -> BTreeMap<String, f64> {
        [
            ("divine", 600.0),
            ("exalt", 1.0),
            ("chaos", 50.0),
            ("forge_inconnu", 3.0),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_string(), *v))
        .collect()
    }

    fn butin(q: &[(&str, u32)], autre: f64, depense: f64) -> Butin {
        Butin {
            quantites: q.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
            autre,
            depense,
        }
    }

    fn bilan() -> Bilan {
        // Deux maps, session de 10 minutes.
        let journal = "\
2026/11/07 20:00:00 1 a [DEBUG Client 1] Generating level 79 area \"MapCrypt\" with seed 42
2026/11/07 20:05:00 1 a [DEBUG Client 1] Generating level 65 area \"HideoutBlankMountain\" with seed 1
2026/11/07 20:06:00 1 a [DEBUG Client 1] Generating level 80 area \"MapSump\" with seed 7
2026/11/07 20:10:00 1 a [DEBUG Client 1] Generating level 65 area \"HideoutBlankMountain\" with seed 1";
        SuiviSession::depuis_texte(journal).bilan(None)
    }

    #[test]
    fn valorise_une_map() {
        let v = butin(&[("divine", 1), ("exalt", 12), ("chaos", 2)], 30.0, 20.0).valeur(&prix());
        assert_eq!(v.brut, 600.0 + 12.0 + 100.0 + 30.0);
        assert_eq!(v.depense, 20.0);
        assert_eq!(v.net, 722.0);
        assert!(v.sans_prix.is_empty());
    }

    #[test]
    fn monnaie_sans_prix_comptee_pour_zero() {
        let v = butin(&[("annul", 2), ("exalt", 3)], 0.0, 0.0).valeur(&prix());
        assert_eq!(v.brut, 3.0);
        assert_eq!(v.sans_prix, vec!["annul".to_string()]);
    }

    #[test]
    fn nettoie_la_saisie() {
        let b = butin(
            &[("exalt", 0), ("inconnu", 4), ("chaos", 2_000_000)],
            f64::NAN,
            -5.0,
        )
        .normalise();
        assert_eq!(b.quantites.len(), 1);
        assert_eq!(b.quantites["chaos"], QUANTITE_MAX);
        assert_eq!((b.autre, b.depense), (0.0, 0.0));
        assert!(butin(&[("exalt", 0)], 0.0, 0.0).normalise().est_vide());
    }

    #[test]
    fn bilan_de_session() {
        let b = bilan();
        let butins: ButinsSession = [
            (0, butin(&[("exalt", 10)], 0.0, 4.0)),
            (1, butin(&[("chaos", 1)], 0.0, 4.0)),
            (5, butin(&[("divine", 1)], 0.0, 0.0)), // map absente : ignorée
        ]
        .into_iter()
        .collect();
        let g = bilan_gains(&b, &butins, &prix());
        assert_eq!(g.par_map.len(), 2);
        assert_eq!(g.par_map[0].as_ref().unwrap().net, 6.0);
        assert_eq!(g.par_map[1].as_ref().unwrap().net, 46.0);
        assert_eq!((g.brut, g.depense, g.net), (60.0, 8.0, 52.0));
        assert_eq!(g.maps_saisies, 2);
        assert_eq!(g.net_par_map, Some(26.0));
        // 52 ex en 10 minutes.
        assert_eq!(g.net_par_heure, Some(312.0));
    }

    #[test]
    fn sans_saisie_pas_de_moyenne() {
        let g = bilan_gains(&bilan(), &ButinsSession::new(), &prix());
        assert_eq!(g.par_map, vec![None, None]);
        assert_eq!((g.net_par_map, g.net_par_heure), (None, None));
    }

    #[test]
    fn instantane_des_prix() {
        let p = prix_objets(&prix());
        assert_eq!(p.len(), 3);
        assert!(!p.contains_key("forge_inconnu"));
    }

    #[test]
    fn les_objets_ont_un_prix_forge() {
        let table = crate::forge::prix::TablePrix::indicative();
        for (id, _) in OBJETS {
            assert!(
                table.prix.contains_key(id),
                "{id} absent de la table de Forge"
            );
        }
    }
}
