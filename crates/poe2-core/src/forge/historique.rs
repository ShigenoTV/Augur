//! Historique des plans de Forge (demande de Max du 08/11/2026) : chaque plan calculé y entre, le
//! joueur le rouvre (nouveau calcul aux prix du moment) ou le supprime. L'historique retient aussi le
//! plan suivi par l'overlay, recalculé au démarrage de l'appli.
//!
//! Aucune entrée-sortie : l'appli lit et écrit le fichier (`forge-historique.json`).

use super::atelier::DemandePlan;
use serde::{Deserialize, Serialize};

/// Nombre d'entrées gardées ; les plus anciennes partent d'abord.
pub const MAX_ENTREES: usize = 50;

/// Un plan calculé.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntreeHistorique {
    /// Identifiant unique (date du calcul en millisecondes, ou suivante si déjà prise).
    pub id: u64,
    /// Date (unix, secondes) du dernier calcul.
    pub calcule_a: u64,
    pub demande: DemandePlan,
    pub base_nom: String,
    /// Affixes visés, lisibles.
    pub labels: Vec<String>,
    /// Coût espéré au moment du calcul, en Exalted.
    pub cout: f64,
}

/// Historique enregistré.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Historique {
    /// Du plus récent au plus ancien.
    #[serde(default)]
    pub entrees: Vec<EntreeHistorique>,
    /// Entrée suivie par l'overlay.
    #[serde(default)]
    pub suivi: Option<u64>,
}

impl Historique {
    /// Ajoute un plan calculé et le marque suivi. La même demande recalculée remplace son entrée
    /// (même identifiant), remontée en tête. Rend l'identifiant de l'entrée.
    pub fn ajouter(
        &mut self,
        maintenant_ms: u64,
        demande: DemandePlan,
        base_nom: String,
        labels: Vec<String>,
        cout: f64,
    ) -> u64 {
        let id = match self.entrees.iter().position(|e| e.demande == demande) {
            Some(i) => self.entrees.remove(i).id,
            None => {
                let max = self.entrees.iter().map(|e| e.id).max().unwrap_or(0);
                maintenant_ms.max(max + 1)
            }
        };
        self.entrees.insert(
            0,
            EntreeHistorique {
                id,
                calcule_a: maintenant_ms / 1000,
                demande,
                base_nom,
                labels,
                cout,
            },
        );
        self.entrees.truncate(MAX_ENTREES);
        self.suivi = Some(id);
        id
    }

    /// Supprime une entrée (et le suivi si c'était elle). `false` si elle n'existe pas.
    pub fn supprimer(&mut self, id: u64) -> bool {
        let avant = self.entrees.len();
        self.entrees.retain(|e| e.id != id);
        if self.suivi == Some(id) {
            self.suivi = None;
        }
        self.entrees.len() != avant
    }

    pub fn arreter_suivi(&mut self) {
        self.suivi = None;
    }

    pub fn entree(&self, id: u64) -> Option<&EntreeHistorique> {
        self.entrees.iter().find(|e| e.id == id)
    }

    /// Demande du plan suivi, à recalculer au démarrage.
    pub fn demande_suivie(&self) -> Option<&DemandePlan> {
        self.suivi
            .and_then(|id| self.entree(id))
            .map(|e| &e.demande)
    }

    /// Relit un fichier ancien ou modifié à la main : suivi orphelin oublié, doublons et surplus
    /// écartés.
    pub fn reparer(mut self) -> Self {
        let mut vus = std::collections::BTreeSet::new();
        self.entrees.retain(|e| vus.insert(e.id));
        self.entrees.truncate(MAX_ENTREES);
        if self.suivi.is_some_and(|id| self.entree(id).is_none()) {
            self.suivi = None;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::objectif::WantedAffix;

    fn demande(base: &str) -> DemandePlan {
        DemandePlan {
            base_id: base.into(),
            ilvl: 80,
            voulus: vec![WantedAffix {
                group: 1,
                family: 0,
                max_tier: 2,
            }],
            exclues: vec![],
        }
    }

    #[test]
    fn ajouter_remplacer_supprimer() {
        let mut h = Historique::default();
        let a = h.ajouter(5_000, demande("A"), "A".into(), vec![], 10.0);
        let b = h.ajouter(5_000, demande("B"), "B".into(), vec![], 20.0);
        assert_ne!(a, b, "identifiants uniques même à la même milliseconde");
        assert_eq!(h.suivi, Some(b));
        // même demande recalculée : même entrée, remontée en tête, nouveau coût
        let a2 = h.ajouter(9_000, demande("A"), "A".into(), vec![], 12.0);
        assert_eq!(a2, a);
        assert_eq!(h.entrees.len(), 2);
        assert_eq!((h.entrees[0].id, h.entrees[0].cout), (a, 12.0));
        assert_eq!(h.entrees[0].calcule_a, 9);
        assert_eq!(h.demande_suivie(), Some(&demande("A")));

        assert!(h.supprimer(a));
        assert!(!h.supprimer(a));
        assert_eq!(h.suivi, None, "le plan suivi supprimé n'est plus suivi");
        assert_eq!(h.entrees.len(), 1);
    }

    #[test]
    fn taille_bornee_et_fichier_repare() {
        let mut h = Historique::default();
        for i in 0..(MAX_ENTREES as u64 + 5) {
            h.ajouter(i, demande(&i.to_string()), String::new(), vec![], 1.0);
        }
        assert_eq!(h.entrees.len(), MAX_ENTREES);
        assert_eq!(h.entrees[0].demande.base_id, (MAX_ENTREES + 4).to_string());

        let json = r#"{"entrees":[
            {"id":1,"calculeA":1,"demande":{"baseId":"A","ilvl":80,"voulus":[]},"baseNom":"A","labels":[],"cout":1},
            {"id":1,"calculeA":2,"demande":{"baseId":"B","ilvl":80,"voulus":[]},"baseNom":"B","labels":[],"cout":2}
        ],"suivi":7}"#;
        let h: Historique = serde_json::from_str(json).unwrap();
        let h = h.reparer();
        assert_eq!(h.entrees.len(), 1);
        assert_eq!(h.suivi, None);
    }
}
