//! Suivi d'une session de farm à partir des lignes de `Client.txt`.
//!
//! Le suivi découpe le temps entre deux générations de zone et l'attribue à
//! la nature de la zone en cours (map, hideout, ville, autre). Une **partie
//! de map** regroupe tout le temps passé dans une même instance de map :
//!
//! - même identifiant et même graine = même instance, donc un retour par
//!   portail après un passage au hideout prolonge la partie au lieu d'en
//!   créer une nouvelle ;
//! - une zone « autre » ouverte depuis la map (portail de mécanique,
//!   sous-zone) compte dans la partie en cours ;
//! - une autre map ferme la partie précédente.
//!
//! Le suivi est déterministe : il ne lit jamais l'horloge, seulement les
//! horodatages du journal (et l'instant `maintenant` fourni par l'appelant
//! pour le segment en cours).

use serde::{Deserialize, Serialize};

use super::journal::{EvenementJournal, Horodatage, LigneJournal};
use super::zones::{TypeZone, type_zone};

/// Durée minimale de session pour annoncer un rythme en maps par heure.
const DUREE_MIN_RYTHME: i64 = 60;

/// Une map jouée pendant la session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartieMap {
    pub area_id: String,
    /// Nom affiché par le client (absent tant que l'entrée n'est pas lue).
    pub nom: Option<String>,
    /// Niveau de zone (niveau des monstres).
    pub niveau: u32,
    pub graine: u64,
    /// Première entrée dans l'instance.
    pub debut: Horodatage,
    /// Dernière sortie de l'instance (ou dernier instant connu si on y est).
    pub fin: Horodatage,
    /// Secondes passées dans la map et ses sous-zones, hors hideout et ville.
    pub duree_active: i64,
    /// Entrées par portail depuis l'extérieur (1 = map faite d'une traite).
    pub entrees: u32,
    /// Sous-zones ouvertes depuis la map.
    pub sous_zones: u32,
}

/// Répartition du temps de la session, en secondes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repartition {
    /// Maps et sous-zones ouvertes depuis une map.
    pub map: i64,
    pub hideout: i64,
    pub ville: i64,
    /// Campagne, Realmgate et zones non rattachées à une map.
    pub autre: i64,
}

impl Repartition {
    pub fn total(&self) -> i64 {
        self.map + self.hideout + self.ville + self.autre
    }
}

/// Bilan d'une session à un instant donné.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bilan {
    /// Première ligne utile lue (absent si la session est vide).
    pub debut: Option<Horodatage>,
    pub fin: Option<Horodatage>,
    pub temps: Repartition,
    pub maps: Vec<PartieMap>,
    /// Maps commencées par heure de session (absent sous une minute).
    pub maps_par_heure: Option<f64>,
    /// Durée active moyenne d'une map (absente sans map).
    pub duree_moyenne_map: Option<i64>,
    /// Zone en cours et sa nature.
    pub zone_courante: Option<String>,
    pub type_zone_courante: Option<TypeZone>,
}

#[derive(Debug, Clone)]
struct ZoneCourante {
    area_id: String,
    nom: Option<String>,
    type_zone: TypeZone,
    depuis: Horodatage,
    /// Partie de map à laquelle le temps de la zone est rattaché.
    partie: Option<usize>,
}

/// Suivi incrémental : on lui pousse les lignes du journal dans l'ordre.
#[derive(Debug, Clone, Default)]
pub struct SuiviSession {
    debut: Option<Horodatage>,
    dernier: Option<Horodatage>,
    zone: Option<ZoneCourante>,
    maps: Vec<PartieMap>,
    /// Dernière partie de map, reprenable tant qu'aucune autre map n'a été ouverte.
    partie_ouverte: Option<usize>,
    temps: Repartition,
}

impl SuiviSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construit le suivi à partir d'un extrait de journal complet.
    pub fn depuis_texte(texte: &str) -> Self {
        let mut suivi = Self::new();
        for ligne in texte.lines().filter_map(super::journal::analyser_ligne) {
            suivi.pousser(&ligne);
        }
        suivi
    }

    /// Intègre une ligne du journal.
    pub fn pousser(&mut self, ligne: &LigneJournal) {
        match &ligne.evenement {
            EvenementJournal::ZoneEntree { nom } => self.nommer(nom),
            EvenementJournal::ZoneGeneree {
                niveau,
                area_id,
                graine,
            } => {
                // Sans date lisible, on se cale sur la dernière ligne datée.
                let Some(t) = ligne.a.or(self.dernier) else {
                    return;
                };
                // Le journal peut reculer (changement d'heure) : on ne compte
                // jamais de durée négative.
                let t = self.dernier.map_or(t, |d| t.max(d));
                self.debut.get_or_insert(t);
                self.dernier = Some(t);
                self.entrer(t, *niveau, area_id, *graine);
            }
        }
    }

    fn nommer(&mut self, nom: &str) {
        let Some(zone) = self.zone.as_mut() else {
            return;
        };
        if zone.nom.is_none() {
            zone.nom = Some(nom.to_owned());
        }
        if zone.type_zone == TypeZone::Map
            && let Some(p) = zone.partie
            && self.maps[p].nom.is_none()
        {
            self.maps[p].nom = Some(nom.to_owned());
        }
    }

    fn entrer(&mut self, t: Horodatage, niveau: u32, area_id: &str, graine: u64) {
        let venait_d_une_map = self.zone.as_ref().is_some_and(|z| z.partie.is_some());
        if let Some(zone) = self.zone.take() {
            Self::clore_segment(&mut self.temps, &mut self.maps, &zone, t);
        }

        let type_zone = type_zone(area_id);
        let partie = match type_zone {
            TypeZone::Map => Some(self.entrer_en_map(t, niveau, area_id, graine, venait_d_une_map)),
            // Sous-zone ouverte depuis la map en cours.
            TypeZone::Autre if venait_d_une_map => self.partie_ouverte.inspect(|&p| {
                self.maps[p].sous_zones += 1;
            }),
            _ => None,
        };
        self.zone = Some(ZoneCourante {
            area_id: area_id.to_owned(),
            nom: None,
            type_zone,
            depuis: t,
            partie,
        });
    }

    fn entrer_en_map(
        &mut self,
        t: Horodatage,
        niveau: u32,
        area_id: &str,
        graine: u64,
        venait_d_une_map: bool,
    ) -> usize {
        if let Some(p) = self.partie_ouverte {
            let partie = &mut self.maps[p];
            // Graine absente (format inattendu) : même zone = même partie.
            if partie.area_id == area_id && partie.graine == graine {
                if !venait_d_une_map {
                    partie.entrees += 1;
                }
                return p;
            }
        }
        self.maps.push(PartieMap {
            area_id: area_id.to_owned(),
            nom: None,
            niveau,
            graine,
            debut: t,
            fin: t,
            duree_active: 0,
            entrees: 1,
            sous_zones: 0,
        });
        let p = self.maps.len() - 1;
        self.partie_ouverte = Some(p);
        p
    }

    fn clore_segment(
        temps: &mut Repartition,
        maps: &mut [PartieMap],
        zone: &ZoneCourante,
        t: Horodatage,
    ) {
        let duree = (t - zone.depuis).max(0);
        if let Some(p) = zone.partie {
            temps.map += duree;
            maps[p].duree_active += duree;
            maps[p].fin = maps[p].fin.max(t);
            return;
        }
        match zone.type_zone {
            TypeZone::Hideout => temps.hideout += duree,
            TypeZone::Ville => temps.ville += duree,
            // Une map a toujours une partie : ce cas ne se produit pas.
            TypeZone::Map | TypeZone::Carrefour | TypeZone::Autre => temps.autre += duree,
        }
    }

    /// Bilan de la session. `maintenant` prolonge la zone en cours jusqu'à
    /// cet instant (même échelle que le journal) ; sans lui, le bilan
    /// s'arrête à la dernière ligne lue.
    pub fn bilan(&self, maintenant: Option<Horodatage>) -> Bilan {
        let mut temps = self.temps;
        let mut maps = self.maps.clone();
        let fin = match (self.dernier, maintenant) {
            (Some(d), Some(m)) => Some(d.max(m)),
            (d, _) => d,
        };
        if let (Some(zone), Some(f)) = (&self.zone, fin) {
            Self::clore_segment(&mut temps, &mut maps, zone, f);
        }

        let total = temps.total();
        let maps_par_heure =
            (total >= DUREE_MIN_RYTHME).then(|| maps.len() as f64 * 3_600.0 / total as f64);
        let duree_moyenne_map = (!maps.is_empty())
            .then(|| maps.iter().map(|m| m.duree_active).sum::<i64>() / maps.len() as i64);

        Bilan {
            debut: self.debut,
            fin,
            temps,
            maps,
            maps_par_heure,
            duree_moyenne_map,
            zone_courante: self
                .zone
                .as_ref()
                .map(|z| z.nom.clone().unwrap_or_else(|| z.area_id.clone())),
            type_zone_courante: self.zone.as_ref().map(|z| z.type_zone),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fabrique une ligne « Generating » à `hh:mm:ss`.
    fn generer(heure: &str, niveau: u32, area: &str, graine: u64) -> String {
        format!(
            "2026/11/07 {heure} 1 a [DEBUG Client 1] Generating level {niveau} area \"{area}\" with seed {graine}"
        )
    }

    fn scene(heure: &str, nom: &str) -> String {
        format!("2026/11/07 {heure} 1 a [INFO Client 1] [SCENE] Set Source [{nom}]")
    }

    fn suivi(lignes: &[String]) -> SuiviSession {
        SuiviSession::depuis_texte(&lignes.join("\n"))
    }

    #[test]
    fn session_vide() {
        let b = SuiviSession::new().bilan(Some(1_000));
        assert_eq!(b.debut, None);
        assert_eq!(b.temps.total(), 0);
        assert!(b.maps.is_empty());
        assert_eq!(b.maps_par_heure, None);
        assert_eq!(b.duree_moyenne_map, None);
    }

    #[test]
    fn une_map_d_une_traite() {
        let s = suivi(&[
            generer("20:00:00", 65, "HideoutBlankMountain", 1),
            scene("20:00:01", "Alpine Plateau Hideout"),
            generer("20:02:00", 79, "MapCrypt", 42),
            scene("20:02:01", "Crypt"),
            generer("20:06:30", 65, "HideoutBlankMountain", 1),
        ]);
        let b = s.bilan(Some(s.bilan(None).fin.unwrap() + 30));
        assert_eq!(b.temps.hideout, 120 + 30);
        assert_eq!(b.temps.map, 270);
        assert_eq!(b.maps.len(), 1);
        let m = &b.maps[0];
        assert_eq!(m.nom.as_deref(), Some("Crypt"));
        assert_eq!(
            (m.niveau, m.graine, m.entrees, m.duree_active),
            (79, 42, 1, 270)
        );
        assert_eq!(m.fin - m.debut, 270);
        // Pas encore de ligne [SCENE] après le retour : on affiche l'identifiant.
        assert_eq!(b.zone_courante.as_deref(), Some("HideoutBlankMountain"));
        assert_eq!(b.type_zone_courante, Some(TypeZone::Hideout));
    }

    #[test]
    fn retour_par_portail_dans_la_meme_instance() {
        let b = suivi(&[
            generer("20:00:00", 79, "MapCrypt", 42),
            generer("20:03:00", 65, "HideoutBlankMountain", 1),
            // Passage au hideout pour vider l'inventaire, puis retour.
            generer("20:04:00", 79, "MapCrypt", 42),
            generer("20:06:00", 65, "HideoutBlankMountain", 1),
        ])
        .bilan(None);
        assert_eq!(b.maps.len(), 1);
        assert_eq!(b.maps[0].entrees, 2);
        assert_eq!(b.maps[0].duree_active, 300);
        assert_eq!(b.maps[0].fin - b.maps[0].debut, 360);
        assert_eq!(b.temps.hideout, 60);
    }

    #[test]
    fn meme_map_autre_graine_est_une_nouvelle_partie() {
        let b = suivi(&[
            generer("20:00:00", 79, "MapCrypt", 42),
            generer("20:03:00", 65, "HideoutBlankMountain", 1),
            generer("20:04:00", 80, "MapCrypt", 43),
            generer("20:06:00", 65, "HideoutBlankMountain", 1),
        ])
        .bilan(None);
        assert_eq!(b.maps.len(), 2);
        assert_eq!(b.maps[1].niveau, 80);
        assert_eq!(b.duree_moyenne_map, Some(150));
    }

    #[test]
    fn une_map_fermee_ne_se_reprend_pas() {
        let b = suivi(&[
            generer("20:00:00", 79, "MapCrypt", 42),
            generer("20:01:00", 79, "MapSump", 7),
            generer("20:02:00", 79, "MapCrypt", 42),
            generer("20:03:00", 65, "HideoutBlankMountain", 1),
        ])
        .bilan(None);
        assert_eq!(b.maps.len(), 3);
    }

    #[test]
    fn sous_zone_rattachee_a_la_map() {
        let b = suivi(&[
            generer("20:00:00", 79, "MapCrypt", 42),
            // Portail de mécanique ouvert depuis la map.
            generer("20:02:00", 79, "AbyssSubArea", 9),
            generer("20:03:00", 79, "MapCrypt", 42),
            generer("20:05:00", 65, "HideoutBlankMountain", 1),
        ])
        .bilan(None);
        assert_eq!(b.maps.len(), 1);
        let m = &b.maps[0];
        assert_eq!((m.entrees, m.sous_zones, m.duree_active), (1, 1, 300));
        assert_eq!(b.temps.map, 300);
        assert_eq!(b.temps.autre, 0);
    }

    #[test]
    fn la_campagne_n_est_pas_une_map() {
        let b = suivi(&[
            generer("20:00:00", 65, "HideoutBlankMountain", 1),
            generer("20:01:00", 12, "G1_2", 5),
            generer("20:05:00", 13, "G1_town", 1),
            generer("20:06:00", 65, "MapLeaguePortal", 3),
            generer("20:07:00", 65, "HideoutBlankMountain", 1),
        ])
        .bilan(None);
        assert!(b.maps.is_empty());
        assert_eq!(
            b.temps,
            Repartition {
                map: 0,
                hideout: 60,
                ville: 60,
                autre: 300,
            }
        );
        assert_eq!(b.temps.total(), 420);
        assert_eq!(b.debut.map(|d| b.fin.unwrap() - d), Some(420));
    }

    #[test]
    fn rythme_en_maps_par_heure() {
        let mut lignes = Vec::new();
        for i in 0..6u64 {
            let m = i * 10;
            lignes.push(generer(&format!("20:{m:02}:00"), 79, "MapCrypt", 100 + i));
            lignes.push(generer(
                &format!("20:{:02}:00", m + 8),
                65,
                "HideoutBlankMountain",
                1,
            ));
        }
        lignes.push(generer("21:00:00", 65, "HideoutBlankMountain", 1));
        let b = suivi(&lignes).bilan(None);
        assert_eq!(b.maps.len(), 6);
        assert_eq!(b.temps.map, 6 * 480);
        assert_eq!(b.maps_par_heure, Some(6.0));
        assert_eq!(b.duree_moyenne_map, Some(480));
    }

    #[test]
    fn horloge_qui_recule_ne_donne_pas_de_duree_negative() {
        let b = suivi(&[
            generer("20:00:00", 79, "MapCrypt", 42),
            generer("19:00:00", 65, "HideoutBlankMountain", 1),
            generer("19:00:30", 79, "MapSump", 1),
        ])
        .bilan(None);
        assert_eq!(b.maps[0].duree_active, 0);
        assert!(b.temps.total() >= 0);
        assert_eq!(b.temps.hideout, 0);
    }

    /// Extrait d'un vrai Client.txt PoE 2 (client 0.5, 2026-10-31), avec
    /// les lignes de chat et de chargement que le suivi doit ignorer.
    const VRAI_JOURNAL: &str = "\
2026/10/31 21:34:34 2674156 7fbd1225 [INFO Client 13604] [SCENE] Set Source [(unknown)]
2026/10/31 21:35:29 2729968 7fbd1225 [INFO Client 13604] [SCENE] Set Source [(null)]
2026/10/31 21:35:30 2730093 2caa229f [DEBUG Client 13604] Generating level 65 area \"HideoutBlankMountain\" with seed 1
2026/10/31 21:35:30 2730328 7fbd1225 [INFO Client 13604] [SCENE] Set Source [Alpine Plateau Hideout]
2026/10/31 21:35:32 2732734 3ef23348 [INFO Client 13604] : You have joined trade chat channel 1 Français.
2026/10/31 21:35:34 2734593 4cba6a94 [INFO Client 13604] [LOADING SCREEN] (Alpine Plateau Hideout) Duration = 4.57065 seconds";

    #[test]
    fn lit_un_vrai_journal_poe2() {
        let s = SuiviSession::depuis_texte(VRAI_JOURNAL);
        let b = s.bilan(Some(s.bilan(None).fin.unwrap() + 600));
        assert!(b.maps.is_empty());
        assert_eq!(b.temps.hideout, 600);
        assert_eq!(b.zone_courante.as_deref(), Some("Alpine Plateau Hideout"));
    }
}
