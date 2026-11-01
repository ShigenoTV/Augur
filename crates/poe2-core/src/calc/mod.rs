//! Moteur de calcul d'Augur.
//!
//! Entrée : un [`Personnage`] (compétence + modificateurs typés avec leur
//! source) et des [`Conditions`] (cible, coup subi). Sortie : des
//! [`Resultats`], où chaque valeur nommée porte sa [`Trace`] de calcul.
//!
//! Archétype couvert au sprint 1 : **sort à dégâts directs** (dégâts de
//! gemme et ajoutés, increased/more par type, critique, vitesse
//! d'incantation, résistances et pénétration de la cible), plus les
//! mitigations défensives de base. Les conversions de dégâts, les attaques,
//! les ailments et les dégâts dans le temps ne sont pas encore couverts :
//! un cas golden qui en dépend doit être marqué « non supporté ».

pub mod defense;
pub mod formules;
pub mod golden;
pub mod moddb;
pub mod modifier;
pub mod offense;
pub mod stats;
pub mod trace;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use moddb::ModDb;
pub use modifier::{Modificateur, TypeModificateur};
pub use stats::TypeDegats;
pub use trace::{Contribution, Trace};

/// Genre de compétence. Seuls les sorts sont couverts au sprint 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GenreCompetence {
    #[default]
    Sort,
}

fn cent() -> f64 {
    100.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Competence {
    pub nom: String,
    #[serde(rename = "type", default)]
    pub genre: GenreCompetence,
    /// Temps d'incantation de base, en secondes.
    pub temps_incantation: f64,
    /// Chance de critique de base, en pourcentage.
    pub critique_base: f64,
    /// Efficacité des dégâts ajoutés, en pourcentage (100 par défaut).
    #[serde(default = "cent")]
    pub efficacite_degats_ajoutes: f64,
    /// Dégâts de base de la gemme par type : `[min, max]`.
    #[serde(default)]
    pub degats: BTreeMap<TypeDegats, [f64; 2]>,
    /// Conversion propre à la compétence (étape 1), en % : `{ physique: { feu: 80 } }`.
    #[serde(default)]
    pub conversion: BTreeMap<TypeDegats, BTreeMap<TypeDegats, f64>>,
    /// Gains propres à la compétence (étape 1), en % ; source `degats` = total
    /// de tous les types : `{ degats: { feu: 50 } }`.
    #[serde(default)]
    pub gain: BTreeMap<String, BTreeMap<TypeDegats, f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Personnage {
    #[serde(default)]
    pub niveau: Option<u32>,
    #[serde(default)]
    pub classe: Option<String>,
    #[serde(default)]
    pub competence: Option<Competence>,
    #[serde(default)]
    pub modificateurs: Vec<Modificateur>,
    /// Pénalité de résistances élémentaires due à la progression dans la
    /// campagne, en points (par exemple −60). Explicite pour rester testable.
    #[serde(default)]
    pub penalite_campagne: f64,
    /// Objets portant des défenses locales (ES, armure, évasion, déflexion).
    #[serde(default)]
    pub objets: Vec<defense::ObjetDefense>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Cible {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub resistances: BTreeMap<TypeDegats, f64>,
    #[serde(default)]
    pub armure: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Conditions {
    #[serde(default)]
    pub cible: Cible,
    /// Dégâts physiques bruts d'un coup subi, pour la réduction de l'armure.
    #[serde(default)]
    pub coup_subi: Option<f64>,
    /// Précision de l'attaquant, pour l'évasion et la déflexion.
    #[serde(default)]
    pub precision_attaquant: Option<f64>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ErreurCalcul {
    #[error("stat inconnue « {stat} » (source : {source_mod}) : vérifier la table des stats")]
    StatInconnue { stat: String, source_mod: String },
    #[error("valeur non finie pour {0}")]
    ValeurNonFinie(String),
    #[error("le temps d'incantation doit être strictement positif (reçu {0})")]
    TempsIncantation(f64),
    #[error("source de gain inconnue « {0} » : un type de dégâts ou « degats »")]
    SourceGain(String),
    #[error("dégâts {0} : le minimum dépasse le maximum ou est négatif")]
    PlageDegats(&'static str),
}

/// Les valeurs calculées, chacune avec sa trace, indexées par nom.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Resultats {
    pub valeurs: BTreeMap<String, Trace>,
}

impl Resultats {
    pub fn valeur(&self, nom: &str) -> Option<f64> {
        self.valeurs.get(nom).map(|t| t.valeur)
    }

    pub fn trace(&self, nom: &str) -> Option<&Trace> {
        self.valeurs.get(nom)
    }

    /// Noms des sorties disponibles, pour l'éditeur de cas golden.
    pub fn noms(&self) -> impl Iterator<Item = &str> {
        self.valeurs.keys().map(String::as_str)
    }
}

/// Vérifie le personnage avant calcul : aucune stat inconnue ni valeur aberrante.
pub fn valider(perso: &Personnage) -> Result<(), Vec<ErreurCalcul>> {
    let mut erreurs = Vec::new();
    for m in &perso.modificateurs {
        if !stats::est_connue(&m.stat) {
            erreurs.push(ErreurCalcul::StatInconnue {
                stat: m.stat.clone(),
                source_mod: m.source.clone(),
            });
        }
        if !m.valeur.is_finite() {
            erreurs.push(ErreurCalcul::ValeurNonFinie(format!(
                "{} ({})",
                m.stat, m.source
            )));
        }
    }
    for o in &perso.objets {
        let v = [
            o.energy_shield,
            o.armure,
            o.evasion,
            o.deflexion,
            o.increased_local,
            o.qualite,
        ];
        if v.iter().any(|x| !x.is_finite()) {
            erreurs.push(ErreurCalcul::ValeurNonFinie(
                o.nom.clone().unwrap_or_else(|| "objet".into()),
            ));
        }
    }
    if !perso.penalite_campagne.is_finite() {
        erreurs.push(ErreurCalcul::ValeurNonFinie(
            "penalite_campagne".to_string(),
        ));
    }
    if let Some(c) = &perso.competence {
        if !(c.temps_incantation > 0.0 && c.temps_incantation.is_finite()) {
            erreurs.push(ErreurCalcul::TempsIncantation(c.temps_incantation));
        }
        for v in [c.critique_base, c.efficacite_degats_ajoutes] {
            if !v.is_finite() {
                erreurs.push(ErreurCalcul::ValeurNonFinie(c.nom.clone()));
            }
        }
        for source in c.gain.keys() {
            if source != "degats" && !TypeDegats::TOUS.iter().any(|t| t.nom() == source) {
                erreurs.push(ErreurCalcul::SourceGain(source.clone()));
            }
        }
        let pourcentages = c
            .conversion
            .values()
            .chain(c.gain.values())
            .flat_map(|m| m.values());
        if pourcentages.into_iter().any(|v| !v.is_finite()) {
            erreurs.push(ErreurCalcul::ValeurNonFinie(format!(
                "{} (conversion)",
                c.nom
            )));
        }
        for (t, [min, max]) in &c.degats {
            if !(min.is_finite() && max.is_finite()) || *min < 0.0 || min > max {
                erreurs.push(ErreurCalcul::PlageDegats(t.nom()));
            }
        }
    }
    if erreurs.is_empty() {
        Ok(())
    } else {
        Err(erreurs)
    }
}

/// Calcule toutes les sorties disponibles pour ce personnage.
pub fn calculer(perso: &Personnage, cond: &Conditions) -> Result<Resultats, Vec<ErreurCalcul>> {
    valider(perso)?;
    let db = ModDb::new(perso.modificateurs.iter().cloned());
    let mut r = Resultats::default();
    let mut ins = |nom: &str, t: Trace| {
        r.valeurs.insert(nom.to_string(), t);
    };

    if let Some(comp) = &perso.competence {
        let o = offense::calculer(&db, comp, &cond.cible);
        ins("dps", o.dps);
        ins("coup_moyen", o.coup_moyen);
        ins("coup_moyen_brut", o.coup_moyen_brut);
        ins("chance_critique", o.chance_critique);
        ins("bonus_degats_critiques", o.bonus_critique);
        ins("incantations_par_seconde", o.incantations_par_seconde);
    }

    let d = defense::calculer(
        &db,
        &perso.objets,
        cond.coup_subi,
        cond.precision_attaquant,
        perso.penalite_campagne,
    );
    ins("evasion", d.evasion);
    ins("deflexion", d.deflexion);
    ins("delai_recharge_es", d.delai_recharge_es);
    ins("recharge_es", d.recharge_es);
    if let Some(tr) = d.chance_eviter {
        ins("chance_eviter", tr);
    }
    if let Some(tr) = d.chance_deflexion {
        ins("chance_deflexion", tr);
    }
    ins("vie", d.vie);
    ins("bouclier_energie", d.bouclier_energie);
    ins("armure", d.armure);
    for (t, tr) in d.resistances {
        ins(&format!("resistance_{}", t.nom()), tr);
    }
    if let Some(tr) = d.reduction_armure {
        ins("reduction_armure", tr);
    }
    for (t, tr) in d.coups_max {
        ins(&format!("coup_max_{}", t.nom()), tr);
    }
    Ok(r)
}

/// Écart d'une sortie quand on ajoute des modificateurs (le « delta » de
/// l'overlay : que m'apporte cet objet ?).
pub fn delta(
    perso: &Personnage,
    cond: &Conditions,
    ajouts: &[Modificateur],
    sortie: &str,
) -> Result<Option<(f64, f64)>, Vec<ErreurCalcul>> {
    let avant = calculer(perso, cond)?;
    let mut apres_perso = perso.clone();
    apres_perso.modificateurs.extend_from_slice(ajouts);
    let apres = calculer(&apres_perso, cond)?;
    Ok(match (avant.valeur(sortie), apres.valeur(sortie)) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    })
}
