//! Path of Exile 1 : ce qu'Augur sait faire pour PoE 1 sans moteur de calcul.
//!
//! - [`ninja`] : cote poe.ninja d'un objet copié en jeu (devises, cartes, uniques) ;
//! - [`trade`] : recherche de l'objet sur le site de trade de PoE 1.
//!
//! Comme le reste de la crate : aucune entrée-sortie, les réponses de poe.ninja
//! sont fournies par l'appli.

pub mod ninja;
pub mod trade;

use crate::parser::Objet;

/// Plus long groupe de sockets liés (« B-B-R B » → 3, « B-B-B-B-B-B » → 6).
pub fn liens(objet: &Objet) -> u32 {
    objet
        .emplacements
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .map(|groupe| groupe.split('-').filter(|s| !s.is_empty()).count() as u32)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parser;

    #[test]
    fn compte_les_liens() {
        let o = |s: &str| {
            parser(&format!(
                "Rarity: Rare\nX\nY\n--------\nSockets: {s}\n--------\n+1 to Strength"
            ))
            .unwrap()
        };
        assert_eq!(liens(&o("B-B-R B")), 3);
        assert_eq!(liens(&o("B-B-B-B-B-B")), 6);
        assert_eq!(liens(&o("R G B")), 1);
    }
}
