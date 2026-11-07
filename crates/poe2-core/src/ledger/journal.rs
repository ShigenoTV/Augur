//! Lecture des lignes de `Client.txt`, le journal texte que le jeu écrit.
//!
//! Formats relevés sur un client PoE 2 (0.5, 31 octobre 2026) :
//!
//! ```text
//! 2026/10/31 21:35:30 2730093 2caa229f [DEBUG Client 13604] Generating level 65 area "HideoutBlankMountain" with seed 1
//! 2026/10/31 21:35:30 2730328 7fbd1225 [INFO Client 13604] [SCENE] Set Source [Alpine Plateau Hideout]
//! 2026/10/31 14:12:34 1 a [INFO Client 1] : You have entered Clearfell.
//! ```
//!
//! La ligne « Generating » précède toujours l'entrée dans la zone : elle
//! porte l'identifiant interne (indépendant de la langue), le niveau de zone
//! et la graine de l'instance. Les lignes de chat ne sont jamais gardées.

use serde::{Deserialize, Serialize};

/// Horodatage d'une ligne du journal, en secondes depuis le 1er janvier 1970
/// **en heure locale du joueur** (le journal n'indique pas le fuseau). Seules
/// les différences entre deux horodatages ont un sens.
pub type Horodatage = i64;

/// Ce qu'une ligne du journal apprend au suivi de session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EvenementJournal {
    /// Génération (ou rechargement) d'une zone.
    #[serde(rename_all = "camelCase")]
    ZoneGeneree {
        niveau: u32,
        area_id: String,
        /// Graine de l'instance : même graine et même zone, même instance.
        graine: u64,
    },
    /// Entrée dans une zone, nom affiché par le client.
    #[serde(rename_all = "camelCase")]
    ZoneEntree { nom: String },
}

/// Une ligne utile du journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LigneJournal {
    /// Absent si la ligne ne commence pas par une date lisible.
    pub a: Option<Horodatage>,
    pub evenement: EvenementJournal,
}

/// Analyse une ligne du journal ; `None` pour tout ce qui n'intéresse pas
/// le suivi (chat, écrans de chargement, lignes tronquées…). Ne panique
/// jamais, quelle que soit l'entrée.
pub fn analyser_ligne(ligne: &str) -> Option<LigneJournal> {
    let evenement = analyser_evenement(ligne)?;
    Some(LigneJournal {
        a: lire_horodatage(ligne),
        evenement,
    })
}

fn analyser_evenement(ligne: &str) -> Option<EvenementJournal> {
    if let Some((_, reste)) = ligne.split_once("] : You have entered ") {
        let nom = reste.trim().trim_end_matches('.');
        if !nom.is_empty() {
            return Some(EvenementJournal::ZoneEntree {
                nom: nom.to_owned(),
            });
        }
    }
    if let Some((_, reste)) = ligne.split_once("[SCENE] Set Source [") {
        let nom = reste.trim_end().trim_end_matches(']');
        if nom.is_empty() || nom == "(null)" || nom == "(unknown)" {
            return None;
        }
        return Some(EvenementJournal::ZoneEntree {
            nom: nom.to_owned(),
        });
    }
    if let Some((_, reste)) = ligne.split_once("] Generating level ") {
        let (niveau, reste) = reste.split_once(" area \"")?;
        let (area_id, reste) = reste.split_once('"')?;
        if area_id.is_empty() {
            return None;
        }
        let graine = reste
            .trim()
            .strip_prefix("with seed ")
            .and_then(|g| g.trim().parse().ok())
            .unwrap_or(0);
        return Some(EvenementJournal::ZoneGeneree {
            niveau: niveau.trim().parse().ok()?,
            area_id: area_id.to_owned(),
            graine,
        });
    }
    None
}

/// Lit `AAAA/MM/JJ HH:MM:SS` en début de ligne.
pub fn lire_horodatage(ligne: &str) -> Option<Horodatage> {
    let tete = ligne.get(..19)?;
    let b = tete.as_bytes();
    let sep_ok = b[4] == b'/' && b[7] == b'/' && b[10] == b' ' && b[13] == b':' && b[16] == b':';
    if !sep_ok {
        return None;
    }
    let nombre = |de: usize, a: usize| -> Option<i64> {
        let s = &tete[de..a];
        if s.bytes().all(|c| c.is_ascii_digit()) {
            s.parse().ok()
        } else {
            None
        }
    };
    let (an, mois, jour) = (nombre(0, 4)?, nombre(5, 7)?, nombre(8, 10)?);
    let (h, m, s) = (nombre(11, 13)?, nombre(14, 16)?, nombre(17, 19)?);
    if !(1..=12).contains(&mois) || !(1..=31).contains(&jour) || h > 23 || m > 59 || s > 60 {
        return None;
    }
    Some(jours_depuis_1970(an, mois, jour) * 86_400 + h * 3_600 + m * 60 + s)
}

/// Nombre de jours entre le 1970-01-01 et la date donnée (calendrier
/// grégorien proleptique, algorithme de H. Hinnant).
fn jours_depuis_1970(an: i64, mois: i64, jour: i64) -> i64 {
    let an = if mois <= 2 { an - 1 } else { an };
    let ere = an.div_euclid(400);
    let annee_ere = an - ere * 400;
    let mois_mars = (mois + 9) % 12;
    let jour_annee = (153 * mois_mars + 2) / 5 + jour - 1;
    let jour_ere = annee_ere * 365 + annee_ere / 4 - annee_ere / 100 + jour_annee;
    ere * 146_097 + jour_ere - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_la_generation_avec_graine() {
        let l = "2026/10/31 14:12:33 1 a [DEBUG Client 1] Generating level 65 area \"MapCrypt\" with seed 3735928559";
        assert_eq!(
            analyser_ligne(l),
            Some(LigneJournal {
                a: lire_horodatage(l),
                evenement: EvenementJournal::ZoneGeneree {
                    niveau: 65,
                    area_id: "MapCrypt".into(),
                    graine: 3_735_928_559,
                },
            })
        );
    }

    #[test]
    fn lit_les_entrees_de_zone() {
        for l in [
            "2026/10/31 14:12:34 1 a [INFO Client 1] [SCENE] Set Source [Clearfell]",
            "2026/10/31 14:12:34 1 a [INFO Client 1] : You have entered Clearfell.",
        ] {
            assert_eq!(
                analyser_ligne(l).map(|x| x.evenement),
                Some(EvenementJournal::ZoneEntree {
                    nom: "Clearfell".into()
                })
            );
        }
        assert_eq!(
            analyser_ligne("2026/10/31 14:12:34 1 a [INFO Client 1] [SCENE] Set Source [(null)]"),
            None
        );
    }

    #[test]
    fn horodatage_coherent() {
        assert_eq!(lire_horodatage("1970/01/01 00:00:00 x"), Some(0));
        // 2026-10-31 21:35:30 (UTC naïf) = 1 793 482 530.
        assert_eq!(
            lire_horodatage("2026/10/31 21:35:30 2730093"),
            Some(1_793_482_530)
        );
        let a = lire_horodatage("2024/02/28 23:59:59").unwrap();
        let b = lire_horodatage("2024/02/29 00:00:00").unwrap();
        assert_eq!(b - a, 1);
        assert_eq!(lire_horodatage("2026/13/01 00:00:00"), None);
        assert_eq!(lire_horodatage("pas une date du tout !"), None);
        assert_eq!(lire_horodatage("2026/10/3"), None);
    }

    #[test]
    fn ignore_le_chat_et_les_lignes_cassees() {
        for l in [
            "",
            "2026/10/31 14:12:33 1 a [INFO Client 1] @From Bob: Generating level 1 area \"x\"",
            "2026/10/31 14:12:33 1 a [INFO Client 1] #Bob: You have entered nothing",
            "2026/10/31 14:12:33 1 a [DEBUG Client 1] Generating level XX area \"MapCrypt\" with seed 1",
            "2026/10/31 14:12:33 1 a [DEBUG Client 1] Generating level 65 area \"",
            "é",
        ] {
            assert_eq!(analyser_ligne(l), None, "{l}");
        }
    }

    #[test]
    fn graine_absente_vaut_zero() {
        let l = "2026/10/31 14:12:33 1 a [DEBUG Client 1] Generating level 12 area \"G1_2\"";
        assert!(matches!(
            analyser_ligne(l).unwrap().evenement,
            EvenementJournal::ZoneGeneree { graine: 0, .. }
        ));
    }
}
