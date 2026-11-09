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
//!
//! Deuxième archétype (Arc, fiche 09) : nombre de chaînes (base du niveau,
//! qualité, `number_of_chains`), infusion consommée par la compétence,
//! modificateurs conditionnels (Zenith II : mana au-dessus de 90 %) et
//! cadence affichée par le panneau du jeu (arrondie à 2 décimales).

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
pub use modifier::{ConditionModificateur, Modificateur, TypeModificateur};
pub use stats::TypeDegats;
pub use trace::{Contribution, Trace};

/// Genre de compétence. Seuls les sorts sont couverts au sprint 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GenreCompetence {
    #[default]
    Sort,
    /// Attaque : dégâts de base déjà tirés de l'arme (`degats`), cadence
    /// tirée du temps d'attaque (`temps_incantation`), stats d'attaque.
    Attaque,
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
    /// Temps d'incantation de base, en secondes (pour une attaque : temps
    /// d'attaque de l'arme, mods locaux et vitesse de la gemme compris).
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
    /// Chaînes de base du niveau de la gemme (RePoE `number_of_chains`).
    /// Sans valeur, la compétence ne chaîne pas et `nombre_chaines` n'est pas calculé.
    #[serde(default)]
    pub chaines_base: Option<f64>,
    /// Qualité de la gemme, en pourcentage.
    #[serde(default)]
    pub qualite: f64,
    /// Chaînes gagnées par point de qualité (Arc : 0,1, arrondi à l'inférieur).
    #[serde(default)]
    pub chaines_par_qualite: f64,
    /// Infusion consommée par la compétence si possible (Arc : foudre).
    #[serde(default)]
    pub infusion: Option<Infusion>,
}

/// Effet d'une infusion consommée (Arc : « 200% more damage » et « Chain 6
/// additional times when Lightning Infused », RePoE `ArcPlayer`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Infusion {
    /// Élément de l'infusion consommée.
    pub element: TypeDegats,
    /// Dégâts en plus (more), en pourcentage, sur tous les types.
    #[serde(default)]
    pub degats_more: f64,
    /// Chaînes supplémentaires.
    #[serde(default)]
    pub chaines: f64,
}

/// Prise en compte de l'infusion d'une compétence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModeInfusion {
    /// Comme le panneau d'information du jeu (I-17, mesuré sur le build de
    /// Max) : les dégâts supposent l'infusion consommée, mais les chaînes
    /// affichées n'incluent pas ses chaînes supplémentaires.
    #[default]
    Panneau,
    /// Infusion réellement consommée : dégâts et chaînes.
    Active,
    /// Aucune infusion disponible.
    Absente,
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
    /// Mana actuel en pourcentage du maximum, pour les modificateurs
    /// conditionnels. Sans valeur : 100 (mana plein, comme le panneau).
    #[serde(default)]
    pub mana_pct: Option<f64>,
    /// Prise en compte de l'infusion de la compétence.
    #[serde(default)]
    pub infusion: ModeInfusion,
}

impl Conditions {
    /// Mana courant retenu, en pourcentage du maximum.
    pub fn mana(&self) -> f64 {
        self.mana_pct.unwrap_or(100.0)
    }

    /// Le modificateur est-il actif dans ces conditions ?
    pub fn active(&self, m: &Modificateur) -> bool {
        match m.condition {
            None => true,
            // « above 90% of your Maximum Mana » : la borne exacte est supposée incluse.
            Some(ConditionModificateur::ManaMinPct(p)) => self.mana() >= p,
        }
    }
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
        let mut nombres = vec![
            c.critique_base,
            c.efficacite_degats_ajoutes,
            c.qualite,
            c.chaines_par_qualite,
        ];
        nombres.extend(c.chaines_base);
        if let Some(i) = &c.infusion {
            nombres.extend([i.degats_more, i.chaines]);
        }
        for v in nombres {
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
    for m in &perso.modificateurs {
        if let Some(ConditionModificateur::ManaMinPct(p)) = m.condition {
            if !p.is_finite() {
                erreurs.push(ErreurCalcul::ValeurNonFinie(format!(
                    "{} (condition)",
                    m.source
                )));
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
    if let Some(m) = cond.mana_pct {
        if !m.is_finite() {
            return Err(vec![ErreurCalcul::ValeurNonFinie("mana_pct".to_string())]);
        }
    }
    // Les modificateurs dont la condition n'est pas remplie sont écartés.
    let mut db = ModDb::new(
        perso
            .modificateurs
            .iter()
            .filter(|m| cond.active(m))
            .cloned(),
    );
    if let Some(comp) = &perso.competence {
        ajouter_mods_competence(&mut db, comp, cond.infusion);
    }
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
        if let Some(base) = comp.chaines_base {
            ins("nombre_chaines", nombre_chaines(&db, base));
        }
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

/// Modificateurs propres à la compétence : qualité (chaînes) et infusion
/// consommée. Ils passent par la base de modificateurs pour apparaître dans
/// la trace avec leur source.
fn ajouter_mods_competence(db: &mut ModDb, comp: &Competence, mode: ModeInfusion) {
    let chaines_qualite = (comp.qualite * comp.chaines_par_qualite + 1e-9).floor();
    if comp.chaines_base.is_some() && chaines_qualite != 0.0 {
        db.ajouter(Modificateur::new(
            stats::NUMBER_OF_CHAINS,
            TypeModificateur::Added,
            chaines_qualite,
            format!(
                "Qualité de la gemme : plancher({} % × {})",
                trace::nombre(comp.qualite),
                trace::nombre(comp.chaines_par_qualite)
            ),
        ));
    }
    let Some(inf) = &comp.infusion else {
        return;
    };
    let nom = format!("Infusion de {} consommée ({})", inf.element.nom(), comp.nom);
    if matches!(mode, ModeInfusion::Panneau | ModeInfusion::Active) && inf.degats_more != 0.0 {
        db.ajouter(Modificateur::new(
            stats::DAMAGE,
            TypeModificateur::More,
            inf.degats_more,
            nom.clone(),
        ));
    }
    if mode == ModeInfusion::Active && inf.chaines != 0.0 && comp.chaines_base.is_some() {
        db.ajouter(Modificateur::new(
            stats::NUMBER_OF_CHAINS,
            TypeModificateur::Added,
            inf.chaines,
            nom,
        ));
    }
}

/// Nombre de chaînes : (base du niveau + ajouts) × increased × more,
/// arrondi à l'inférieur (fiche 09, mesuré : 11 + 3 = 14, × 2 = 28).
fn nombre_chaines(db: &ModDb, base: f64) -> Trace {
    let brut = db.evaluer(
        "chaînes (avant arrondi)",
        &[stats::NUMBER_OF_CHAINS],
        Some((base, "chaînes de base du niveau de la gemme")),
    );
    let v = (brut.valeur + 1e-9).floor().max(0.0);
    Trace::feuille("nombre de chaînes", v)
        .avec_formule(format!("plancher({})", trace::nombre(brut.valeur)))
        .avec_enfant(brut)
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
