//! Base de modificateurs et évaluation générique d'une stat.

use super::modifier::{Modificateur, TypeModificateur};
use super::trace::{Contribution, Trace, nombre};

/// L'ensemble des modificateurs actifs d'un personnage dans une configuration.
#[derive(Debug, Clone, Default)]
pub struct ModDb {
    mods: Vec<Modificateur>,
}

/// Somme (ou produit pour `more`) d'un type de modificateur sur plusieurs stats.
#[derive(Debug, Clone)]
pub struct Agregat {
    pub valeur: f64,
    pub contributions: Vec<Contribution>,
}

impl ModDb {
    pub fn new(mods: impl IntoIterator<Item = Modificateur>) -> Self {
        Self {
            mods: mods.into_iter().collect(),
        }
    }

    pub fn ajouter(&mut self, m: Modificateur) {
        self.mods.push(m);
    }

    pub fn mods(&self) -> &[Modificateur] {
        &self.mods
    }

    fn filtrer<'a>(
        &'a self,
        kind: TypeModificateur,
        stats: &'a [&'a str],
    ) -> impl Iterator<Item = &'a Modificateur> + 'a {
        self.mods
            .iter()
            .filter(move |m| m.kind == kind && stats.iter().any(|s| *s == m.stat))
    }

    fn contributions(&self, kind: TypeModificateur, stats: &[&str]) -> Vec<Contribution> {
        self.filtrer(kind, stats)
            .map(|m| Contribution {
                source: m.source.clone(),
                stat: m.stat.clone(),
                kind: m.kind,
                valeur: m.valeur,
            })
            .collect()
    }

    /// Somme des modificateurs `kind` (base, added, increased) sur les stats.
    pub fn somme(&self, kind: TypeModificateur, stats: &[&str]) -> Agregat {
        let contributions = self.contributions(kind, stats);
        let valeur = contributions.iter().map(|c| c.valeur).sum();
        Agregat {
            valeur,
            contributions,
        }
    }

    /// Produit des `more`/`less` sur les stats : Π (1 + v/100).
    pub fn produit_more(&self, stats: &[&str]) -> Agregat {
        let contributions = self.contributions(TypeModificateur::More, stats);
        let valeur = contributions
            .iter()
            .map(|c| 1.0 + c.valeur / 100.0)
            .product();
        Agregat {
            valeur,
            contributions,
        }
    }

    /// Dernier `override` déclaré sur les stats, s'il existe.
    pub fn override_(&self, stats: &[&str]) -> Option<Contribution> {
        self.contributions(TypeModificateur::Override, stats).pop()
    }

    /// Trace du facteur « increased » : 1 + Σ/100.
    pub fn trace_increased(&self, libelle: &str, stats: &[&str]) -> Trace {
        let inc = self.somme(TypeModificateur::Increased, stats);
        let mut t = Trace::feuille(libelle, 1.0 + inc.valeur / 100.0)
            .avec_formule(format!("1 + {}% / 100", nombre(inc.valeur)));
        t.contributions = inc.contributions;
        t
    }

    /// Trace du facteur « more » : Π (1 + v/100).
    pub fn trace_more(&self, libelle: &str, stats: &[&str]) -> Trace {
        let more = self.produit_more(stats);
        let detail = if more.contributions.is_empty() {
            "aucun".to_string()
        } else {
            more.contributions
                .iter()
                .map(|c| format!("(1 + {}%)", nombre(c.valeur)))
                .collect::<Vec<_>>()
                .join(" × ")
        };
        let mut t = Trace::feuille(libelle, more.valeur).avec_formule(detail);
        t.contributions = more.contributions;
        t
    }

    /// Évaluation générique : `(base_externe + Σbase + Σadded) × (1 + Σinc/100) × Π more`,
    /// remplacée par un `override` s'il existe.
    ///
    /// `base_externe` porte une base qui ne vient pas d'un modificateur
    /// (statistique de la gemme, valeur par défaut du jeu).
    pub fn evaluer(
        &self,
        libelle: &str,
        stats: &[&str],
        base_externe: Option<(f64, &str)>,
    ) -> Trace {
        if let Some(o) = self.override_(stats) {
            let mut t = Trace::feuille(libelle, o.valeur).avec_formule("override");
            t.contributions = vec![o];
            return t;
        }
        let base = self.somme(TypeModificateur::Base, stats);
        let added = self.somme(TypeModificateur::Added, stats);
        let externe = base_externe.map(|(v, _)| v).unwrap_or(0.0);
        let total_base = externe + base.valeur + added.valeur;

        let mut t_base = Trace::feuille("base", total_base);
        if let Some((v, src)) = base_externe {
            t_base.enfants.push(Trace::feuille(src, v));
        }
        t_base.contributions = base.contributions;
        t_base.contributions.extend(added.contributions);

        let t_inc = self.trace_increased("increased", stats);
        let t_more = self.trace_more("more", stats);
        let valeur = total_base * t_inc.valeur * t_more.valeur;
        Trace::feuille(libelle, valeur)
            .avec_formule(format!(
                "{} × {} × {}",
                nombre(total_base),
                nombre(t_inc.valeur),
                nombre(t_more.valeur)
            ))
            .avec_enfants([t_base, t_inc, t_more])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TypeModificateur::*;

    fn m(stat: &str, k: TypeModificateur, v: f64) -> Modificateur {
        Modificateur::new(stat, k, v, "test")
    }

    #[test]
    fn increased_additifs_more_multiplicatifs() {
        let db = ModDb::new([
            m("life", Base, 100.0),
            m("life", Added, 50.0),
            m("life", Increased, 30.0),
            m("life", Increased, -10.0),
            m("life", More, 20.0),
            m("life", More, -50.0),
        ]);
        let t = db.evaluer("vie", &["life"], None);
        // 150 × 1.2 × 1.2 × 0.5
        assert!((t.valeur - 108.0).abs() < 1e-9);
    }

    #[test]
    fn override_remplace() {
        let db = ModDb::new([m("life", Base, 100.0), m("life", Override, 1.0)]);
        assert_eq!(db.evaluer("vie", &["life"], None).valeur, 1.0);
    }

    #[test]
    fn stats_non_ciblees_ignorees() {
        let db = ModDb::new([m("mana", Base, 100.0)]);
        assert_eq!(db.evaluer("vie", &["life"], None).valeur, 0.0);
    }
}
