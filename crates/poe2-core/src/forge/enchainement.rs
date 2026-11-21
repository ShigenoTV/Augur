//! Essais enchaînés sur l'objet en cours de craft : à chaque Ctrl+Alt+C, l'overlay compare l'objet
//! à celui de la copie précédente et dit si l'essai a fait avancer le plan (« ça a marché ») ou non
//! (« raté »), avec le coût restant, le nombre d'essais de la série et la dépense estimée.
//!
//! La série est gardée en mémoire seulement : un nouveau plan suivi, l'arrêt du suivi ou un objet
//! fini la remettent à zéro. Commun à la Forge PoE 2 et à la Forge PoE 1.

use serde::Serialize;

/// Ce qu'on retient d'une copie pour la comparer à la suivante.
#[derive(Clone, Debug, PartialEq)]
pub struct Repere {
    /// Texte copié en jeu : deux copies identiques sont le même objet, sans essai entre les deux.
    pub empreinte: String,
    /// Coût moyen restant jusqu'à l'objet fini, s'il est connu.
    pub reste: Option<f64>,
    /// Nombre de mods voulus déjà sur l'objet.
    pub voulus: u8,
    /// Tous les mods voulus sont là.
    pub fini: bool,
    /// Le conseil était de repartir d'une base neuve : la copie suivante ouvre une nouvelle série.
    pub abandon: bool,
    /// Coût d'un essai de l'action conseillée (compté dans la dépense au prochain objet différent).
    pub cout_essai: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// L'essai a rapproché l'objet du but.
    Reussi,
    /// L'essai n'a rien apporté, ou a éloigné l'objet du but.
    Rate,
    /// Tous les mods voulus sont là.
    Atteint,
    /// Même objet que la copie précédente : aucun essai entre les deux.
    MemeObjet,
}

/// Bilan affiché en tête du panneau Forge.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Enchainement {
    pub verdict: Verdict,
    /// Essais comptés depuis le début de la série.
    pub essais: u32,
    /// Dépense estimée de la série (coût unitaire de chaque action conseillée puis tentée).
    pub depense: f64,
    /// Coût moyen restant après cet essai.
    pub reste: Option<f64>,
    /// Coût moyen restant avant cet essai.
    pub reste_avant: Option<f64>,
}

/// Série d'essais en cours sur le plan suivi.
#[derive(Clone, Debug, Default)]
pub struct Serie {
    dernier: Option<Repere>,
    essais: u32,
    depense: f64,
}

/// Écart relatif sous lequel deux coûts restants sont tenus pour égaux.
const TOLERANCE: f64 = 0.005;

impl Serie {
    /// Remet la série à zéro (nouveau plan suivi, suivi arrêté).
    pub fn vider(&mut self) {
        *self = Self::default();
    }

    /// Enregistre la copie `r` et la compare à la précédente. `None` : première copie de la série
    /// (rien à comparer).
    pub fn suivre(&mut self, r: Repere) -> Option<Enchainement> {
        let Some(avant) = self.dernier.take() else {
            self.dernier = Some(r);
            return None;
        };
        if avant.empreinte == r.empreinte {
            let e = Enchainement {
                verdict: if r.fini {
                    Verdict::Atteint
                } else {
                    Verdict::MemeObjet
                },
                essais: self.essais,
                depense: self.depense,
                reste: r.reste,
                reste_avant: r.reste,
            };
            self.dernier = Some(r);
            return Some(e);
        }
        // Base neuve après un abandon conseillé, ou nouvel objet après un objet fini : nouvelle
        // série.
        if avant.abandon || avant.fini {
            *self = Self {
                dernier: Some(r),
                ..Self::default()
            };
            return None;
        }
        self.essais += 1;
        self.depense += avant.cout_essai.unwrap_or(0.0);
        let verdict = if r.fini {
            Verdict::Atteint
        } else {
            match (avant.reste, r.reste) {
                (Some(a), Some(b)) if b < a * (1.0 - TOLERANCE) => Verdict::Reussi,
                (Some(_), Some(_)) => Verdict::Rate,
                _ if r.voulus > avant.voulus => Verdict::Reussi,
                _ => Verdict::Rate,
            }
        };
        let e = Enchainement {
            verdict,
            essais: self.essais,
            depense: self.depense,
            reste: r.reste,
            reste_avant: avant.reste,
        };
        self.dernier = Some(r);
        Some(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(empreinte: &str, reste: Option<f64>, voulus: u8) -> Repere {
        Repere {
            empreinte: empreinte.into(),
            reste,
            voulus,
            fini: false,
            abandon: false,
            cout_essai: Some(2.0),
        }
    }

    #[test]
    fn premiere_copie_sans_bilan() {
        let mut s = Serie::default();
        assert_eq!(s.suivre(rep("a", Some(100.0), 0)), None);
    }

    #[test]
    fn essai_reussi_puis_rate() {
        let mut s = Serie::default();
        s.suivre(rep("a", Some(100.0), 0));
        let e = s.suivre(rep("b", Some(60.0), 1)).unwrap();
        assert_eq!(e.verdict, Verdict::Reussi);
        assert_eq!((e.essais, e.depense), (1, 2.0));
        assert_eq!((e.reste_avant, e.reste), (Some(100.0), Some(60.0)));
        let e = s.suivre(rep("c", Some(80.0), 1)).unwrap();
        assert_eq!(e.verdict, Verdict::Rate);
        assert_eq!((e.essais, e.depense), (2, 4.0));
    }

    #[test]
    fn meme_etat_mais_autre_objet_est_rate() {
        // Spam d'une monnaie qui laisse l'objet au même point du plan : essai raté.
        let mut s = Serie::default();
        s.suivre(rep("a", Some(50.0), 1));
        let e = s.suivre(rep("b", Some(50.0), 1)).unwrap();
        assert_eq!(e.verdict, Verdict::Rate);
        assert_eq!(e.essais, 1);
    }

    #[test]
    fn meme_objet_ne_compte_pas() {
        let mut s = Serie::default();
        s.suivre(rep("a", Some(50.0), 1));
        s.suivre(rep("b", Some(40.0), 2));
        let e = s.suivre(rep("b", Some(40.0), 2)).unwrap();
        assert_eq!(e.verdict, Verdict::MemeObjet);
        assert_eq!((e.essais, e.depense), (1, 2.0));
    }

    #[test]
    fn objectif_atteint_puis_nouvelle_serie() {
        let mut s = Serie::default();
        s.suivre(rep("a", Some(10.0), 2));
        let fini = Repere {
            fini: true,
            reste: Some(0.0),
            ..rep("b", Some(0.0), 3)
        };
        assert_eq!(s.suivre(fini).unwrap().verdict, Verdict::Atteint);
        assert_eq!(s.suivre(rep("c", Some(100.0), 0)), None);
        assert_eq!(s.suivre(rep("d", Some(90.0), 0)).unwrap().essais, 1);
    }

    #[test]
    fn base_neuve_apres_abandon() {
        let mut s = Serie::default();
        s.suivre(Repere {
            abandon: true,
            ..rep("a", Some(300.0), 0)
        });
        assert_eq!(s.suivre(rep("b", Some(100.0), 0)), None);
    }

    #[test]
    fn sans_cout_restant_compte_les_voulus() {
        let mut s = Serie::default();
        s.suivre(rep("a", None, 1));
        assert_eq!(
            s.suivre(rep("b", None, 2)).unwrap().verdict,
            Verdict::Reussi
        );
        assert_eq!(s.suivre(rep("c", None, 2)).unwrap().verdict, Verdict::Rate);
    }

    #[test]
    fn vider_repart_de_zero() {
        let mut s = Serie::default();
        s.suivre(rep("a", Some(100.0), 0));
        s.suivre(rep("b", Some(90.0), 0));
        s.vider();
        assert_eq!(s.suivre(rep("c", Some(100.0), 0)), None);
    }
}
