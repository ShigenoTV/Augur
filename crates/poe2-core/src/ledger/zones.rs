//! Classement des zones à partir de leur identifiant interne (`area_id` de
//! la ligne « Generating level … » de `Client.txt`).
//!
//! Règles tirées de la table `WorldAreas` du jeu (RePoE, client 0.5) :
//! les maps de l'Atlas commencent par `Map`, les hideouts contiennent
//! `Hideout`, les villes finissent par `_town`. Les règles par préfixe
//! tiennent quand GGG ajoute des maps ; seules les exceptions sont listées.

use serde::{Deserialize, Serialize};

/// Nature d'une zone pour le suivi de session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TypeZone {
    /// Map de l'Atlas ouverte avec une Waystone (y compris maps uniques,
    /// citadelles et arènes de boss de fin de jeu).
    Map,
    /// Hideout du joueur (ou d'un autre joueur).
    Hideout,
    /// Ville de la campagne ou de la fin de jeu.
    Ville,
    /// Zone de passage de la fin de jeu, hors map (Realmgate…).
    Carrefour,
    /// Tout le reste : campagne, sous-zones ouvertes depuis une map
    /// (portails de mécaniques), épreuves…
    Autre,
}

/// Identifiants qui commencent par `Map` sans être une map à faire.
const CARREFOURS: &[&str] = &["MapLeaguePortal"];

/// Villes dont l'identifiant ne finit pas par `_town`.
const VILLES: &[&str] = &["G_Endgame_Town", "CurrentTown"];

/// Classe une zone d'après son identifiant interne.
pub fn type_zone(area_id: &str) -> TypeZone {
    // Les hideouts « _Claimable » sont des maps à faire avant d'être réclamées.
    if area_id.contains("Hideout") && !area_id.ends_with("_Claimable") {
        return TypeZone::Hideout;
    }
    if VILLES.contains(&area_id) || area_id.to_ascii_lowercase().ends_with("_town") {
        return TypeZone::Ville;
    }
    if CARREFOURS.contains(&area_id) {
        return TypeZone::Carrefour;
    }
    if area_id.starts_with("Map") {
        return TypeZone::Map;
    }
    TypeZone::Autre
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classe_les_zones_connues() {
        for (id, attendu) in [
            ("MapCrypt", TypeZone::Map),
            ("MapUniqueVault", TypeZone::Map),
            ("MapUberBoss_IronCitadel", TypeZone::Map),
            ("Map_HildaCampsite", TypeZone::Map),
            ("MapHideoutFarmlands_Claimable", TypeZone::Map),
            ("HideoutBlankMountain", TypeZone::Hideout),
            ("PersonalHideout", TypeZone::Hideout),
            ("PlayerAtlasHideout", TypeZone::Hideout),
            ("G1_town", TypeZone::Ville),
            ("P1_Town", TypeZone::Ville),
            ("G_Endgame_Town", TypeZone::Ville),
            ("MapLeaguePortal", TypeZone::Carrefour),
            ("G1_2", TypeZone::Autre),
            ("", TypeZone::Autre),
        ] {
            assert_eq!(type_zone(id), attendu, "{id}");
        }
    }
}
