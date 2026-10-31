//! Cas unitaires de formule du Theorycrafter (`tests/formules/*.yaml`).
//!
//! Format : `docs/theorycraft/format-cas-golden.md`. Chaque cas
//! `type: unitaire` nomme un `calcul` ; ce module le traduit en appels au
//! **moteur** (pas à une réimplémentation des formules) et compare les sorties
//! clé par clé, sorties imbriquées comprises (`par_type.feu`). Une clé
//! attendue absente des sorties vaut 0, comme dans l'oracle Python
//! `tools/theorycraft/verifier_golden.py`.
//!
//! Les cas `type: personnage` sont ignorés tant qu'ils ne sont pas validés.
//! Un `calcul` sans équivalent dans le moteur (évasion, déflexion, bouclier
//! d'énergie, conversions) est compté « en attente », jamais en échec.
//! Comme [`super::golden`], ce module ne lit pas le disque.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

use super::defense::reduction_physique;
use super::modifier::{Modificateur, TypeModificateur as T};
use super::offense::{self, resistance_effective_cible};
use super::stats::{self, TypeDegats};
use super::trace::nombre;
use super::{Cible, Competence, Conditions, ModDb, Personnage, calculer};

pub const TAILLE_MAX_YAML: usize = 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct FichierFormules {
    #[serde(default)]
    pub mecanique: Option<String>,
    #[serde(default)]
    pub fiche: Option<String>,
    #[serde(default)]
    pub cas: Vec<CasFormule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CasFormule {
    pub id: String,
    #[serde(rename = "type")]
    pub genre: String,
    #[serde(default)]
    pub calcul: Option<String>,
    #[serde(default)]
    pub statut: Option<String>,
    #[serde(default)]
    pub confiance: Option<String>,
    #[serde(default)]
    pub entrees: Option<Mapping>,
    #[serde(default)]
    pub attendu: Option<Value>,
    #[serde(default)]
    pub tolerance_pct: Option<f64>,
}

pub fn lire(yaml: &str) -> Result<FichierFormules, String> {
    if yaml.len() > TAILLE_MAX_YAML {
        return Err(format!("fichier trop gros ({} octets)", yaml.len()));
    }
    serde_yaml_ng::from_str(yaml).map_err(|e| e.to_string())
}

/// Sortie d'un calcul : un nombre ou un dictionnaire de sorties.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Sortie {
    Nombre(f64),
    Dict(BTreeMap<String, Sortie>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Reussi,
    Echec,
    Erreur,
    /// Mécanique pas encore dans le moteur, ou gabarit de mesure non validé.
    EnAttente,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ecart {
    pub chemin: String,
    pub attendu: f64,
    pub obtenu: f64,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultatFormule {
    pub id: String,
    pub calcul: Option<String>,
    pub verdict: Verdict,
    pub ecarts: Vec<Ecart>,
    /// Raison d'une attente ou d'une erreur.
    pub message: Option<String>,
}

enum Issue {
    Attente(String),
    Erreur(String),
}

/// Accès aux entrées avec suivi des clés lues : une clé jamais lue est une
/// faute de frappe probable et fait échouer le cas.
struct Entrees<'a> {
    map: Option<&'a Mapping>,
    lues: RefCell<BTreeSet<String>>,
}

impl<'a> Entrees<'a> {
    fn brut(&self, cle: &str) -> Option<&'a Value> {
        self.lues.borrow_mut().insert(cle.to_string());
        self.map?.get(cle).filter(|v| !v.is_null())
    }

    fn nombre(&self, cle: &str, defaut: Option<f64>) -> Result<f64, Issue> {
        match self.brut(cle) {
            Some(v) => v
                .as_f64()
                .ok_or_else(|| Issue::Erreur(format!("`{cle}` doit être un nombre"))),
            None => defaut.ok_or_else(|| Issue::Erreur(format!("entrée `{cle}` manquante"))),
        }
    }

    fn liste(&self, cle: &str) -> Result<Vec<f64>, Issue> {
        match self.brut(cle) {
            None => Ok(Vec::new()),
            Some(Value::Sequence(s)) => s
                .iter()
                .map(|v| {
                    v.as_f64()
                        .ok_or_else(|| Issue::Erreur(format!("`{cle}` : nombre attendu")))
                })
                .collect(),
            Some(_) => Err(Issue::Erreur(format!("`{cle}` doit être une liste"))),
        }
    }

    fn dict(&self, cle: &str) -> Result<Vec<(String, &'a Value)>, Issue> {
        match self.brut(cle) {
            None => Ok(Vec::new()),
            Some(Value::Mapping(m)) => m
                .iter()
                .map(|(k, v)| {
                    k.as_str()
                        .map(|k| (k.to_string(), v))
                        .ok_or_else(|| Issue::Erreur(format!("`{cle}` : clé texte attendue")))
                })
                .collect(),
            Some(_) => Err(Issue::Erreur(format!("`{cle}` doit être un dictionnaire"))),
        }
    }

    fn par_type(&self, cle: &str) -> Result<Vec<(TypeDegats, f64)>, Issue> {
        self.dict(cle)?
            .into_iter()
            .map(|(k, v)| {
                let t = type_degats(&k)?;
                let x = v
                    .as_f64()
                    .ok_or_else(|| Issue::Erreur(format!("`{cle}.{k}` : nombre attendu")))?;
                Ok((t, x))
            })
            .collect()
    }

    fn plages(&self, cle: &str) -> Result<Vec<(TypeDegats, [f64; 2])>, Issue> {
        self.dict(cle)?
            .into_iter()
            .map(|(k, v)| {
                let t = type_degats(&k)?;
                let s = v.as_sequence().filter(|s| s.len() == 2);
                let p = s.and_then(|s| Some([s[0].as_f64()?, s[1].as_f64()?]));
                p.map(|p| (t, p))
                    .ok_or_else(|| Issue::Erreur(format!("`{cle}.{k}` : [min, max] attendu")))
            })
            .collect()
    }

    fn texte(&self, cle: &str) -> Result<String, Issue> {
        self.brut(cle)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| Issue::Erreur(format!("entrée `{cle}` (texte) manquante")))
    }

    fn non_lues(&self) -> Vec<String> {
        let lues = self.lues.borrow();
        self.map
            .map(|m| {
                m.keys()
                    .filter_map(Value::as_str)
                    .filter(|k| !lues.contains(*k))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn type_degats(nom: &str) -> Result<TypeDegats, Issue> {
    TypeDegats::TOUS
        .into_iter()
        .find(|t| t.nom() == nom)
        .ok_or_else(|| Issue::Erreur(format!("type de dégâts inconnu « {nom} »")))
}

fn m(stat: impl Into<String>, k: T, v: f64) -> Modificateur {
    Modificateur::new(stat, k, v, "cas de formule")
}

fn dict(paires: impl IntoIterator<Item = (&'static str, f64)>) -> Sortie {
    Sortie::Dict(
        paires
            .into_iter()
            .map(|(k, v)| (k.to_string(), Sortie::Nombre(v)))
            .collect(),
    )
}

fn competence(nom: &str) -> Competence {
    Competence {
        nom: nom.to_string(),
        genre: Default::default(),
        temps_incantation: 1.0,
        critique_base: 0.0,
        efficacite_degats_ajoutes: 100.0,
        degats: BTreeMap::new(),
    }
}

fn degats_coup_moyen(e: &Entrees) -> Result<Sortie, Issue> {
    for cle in [
        "conversion_competence",
        "gain_competence",
        "conversion_secondaire",
        "gain_secondaire",
    ] {
        if !e.dict(cle)?.is_empty() {
            return Err(Issue::Attente(format!(
                "conversions et gains de dégâts pas encore dans le moteur (`{cle}`)"
            )));
        }
    }
    let mut comp = competence("cas de formule");
    comp.efficacite_degats_ajoutes = e.nombre("efficacite_ajoutes", Some(1.0))? * 100.0;
    for (t, p) in e.plages("base")? {
        comp.degats.insert(t, p);
    }
    let mut mods = Vec::new();
    for (t, [min, max]) in e.plages("ajoutes")? {
        mods.push(m(t.stat_ajoutes_min(), T::Added, min));
        mods.push(m(t.stat_ajoutes_max(), T::Added, max));
    }
    for (etiquette, v) in e.dict("increased")? {
        let v = v
            .as_f64()
            .ok_or_else(|| Issue::Erreur(format!("increased.{etiquette} : nombre attendu")))?;
        let stat = match etiquette.as_str() {
            "sort" => stats::SPELL_DAMAGE.to_string(),
            "tous" => stats::DAMAGE.to_string(),
            "elementaire" => stats::ELEMENTAL_DAMAGE.to_string(),
            autre => type_degats(autre)?.stat_degats(),
        };
        mods.push(m(stat, T::Increased, v));
    }
    for v in e.liste("more")? {
        mods.push(m(stats::DAMAGE, T::More, v));
    }
    let o = offense::calculer(&ModDb::new(mods), &comp, &Cible::default());
    let par_type = Sortie::Dict(
        o.par_type_brut
            .iter()
            .map(|(t, v)| (t.nom().to_string(), Sortie::Nombre(*v)))
            .collect(),
    );
    Ok(Sortie::Dict(BTreeMap::from([
        ("par_type".to_string(), par_type),
        (
            "total".to_string(),
            Sortie::Nombre(o.coup_moyen_brut.valeur),
        ),
    ])))
}

fn critique_multiplicateur(e: &Entrees) -> Result<Sortie, Issue> {
    let mut comp = competence("cas de formule");
    comp.critique_base = e.nombre("crit_base", None)?;
    comp.degats.insert(TypeDegats::Feu, [1.0, 1.0]);
    let mods = vec![
        m(
            stats::CRITICAL_HIT_CHANCE,
            T::Added,
            e.nombre("crit_plat", Some(0.0))?,
        ),
        m(
            stats::CRITICAL_HIT_CHANCE,
            T::Increased,
            e.nombre("increased_crit", Some(0.0))?,
        ),
        m(
            stats::CRITICAL_DAMAGE_BONUS,
            T::Added,
            e.nombre("bonus_plat", Some(0.0))?,
        ),
        m(
            stats::CRITICAL_DAMAGE_BONUS,
            T::Increased,
            e.nombre("increased_bonus", Some(0.0))?,
        ),
    ];
    let o = offense::calculer(&ModDb::new(mods), &comp, &Cible::default());
    Ok(dict([
        ("chance_pct", o.chance_critique.valeur),
        ("bonus_pct", o.bonus_critique.valeur),
        ("multiplicateur_moyen", o.multiplicateur_critique),
    ]))
}

fn vitesse_incantation(e: &Entrees) -> Result<Sortie, Issue> {
    let mut comp = competence("cas de formule");
    comp.temps_incantation = e.nombre("temps_base", None)?;
    let mut mods = vec![m(
        stats::CAST_SPEED,
        T::Increased,
        e.nombre("increased", Some(0.0))?,
    )];
    mods.extend(
        e.liste("more")?
            .into_iter()
            .map(|v| m(stats::CAST_SPEED, T::More, v)),
    );
    let o = offense::calculer(&ModDb::new(mods), &comp, &Cible::default());
    let cadence = o.incantations_par_seconde.valeur;
    Ok(dict([("cadence", cadence), ("temps", 1.0 / cadence)]))
}

fn dps_sort(e: &Entrees) -> Result<Sortie, Issue> {
    let mut comp = competence("cas de formule");
    for (t, v) in e.par_type("coup_moyen_par_type")? {
        comp.degats.insert(t, [v, v]);
    }
    let cadence = e.nombre("cadence", None)?;
    if cadence <= 0.0 {
        return Err(Issue::Erreur("`cadence` doit être positive".into()));
    }
    comp.temps_incantation = 1.0 / cadence;
    // Le multiplicateur moyen des critiques est rejoué par le moteur avec une
    // chance de 100 % et le bonus correspondant.
    let mc = e.nombre("multiplicateur_crit", Some(1.0))?;
    let mut mods = Vec::new();
    if mc < 1.0 {
        return Err(Issue::Erreur(
            "`multiplicateur_crit` doit valoir au moins 1".into(),
        ));
    }
    if mc > 1.0 {
        comp.critique_base = 100.0;
        mods.push(m(
            stats::CRITICAL_DAMAGE_BONUS,
            T::Added,
            (mc - 1.0) * 100.0 - offense::BONUS_CRITIQUE_BASE,
        ));
    }
    let mut cible = Cible::default();
    for (t, r) in e.par_type("res_cible")? {
        cible.resistances.insert(t, r);
    }
    for (t, p) in e.par_type("penetration")? {
        mods.push(m(t.stat_penetration(), T::Added, p));
    }
    let o = offense::calculer(&ModDb::new(mods), &comp, &cible);
    Ok(dict([("dps", o.dps.valeur)]))
}

fn resistances_joueur(e: &Entrees) -> Result<Sortie, Issue> {
    let t = type_degats(&e.texte("type")?)?;
    if t == TypeDegats::Physique {
        return Err(Issue::Erreur("pas de résistance physique".into()));
    }
    let perso = Personnage {
        // Convention des fiches : pénalité positive qui se retranche.
        penalite_campagne: -e.nombre("penalite_campagne", Some(60.0))?,
        modificateurs: vec![
            m(t.stat_resistance(), T::Added, e.nombre("brute", None)?),
            m(
                t.stat_resistance_max(),
                T::Added,
                e.nombre("bonus_max", Some(0.0))?,
            ),
        ],
        ..Default::default()
    };
    let r =
        calculer(&perso, &Conditions::default()).map_err(|e| Issue::Erreur(format!("{e:?}")))?;
    let trace = r
        .trace(&format!("resistance_{}", t.nom()))
        .ok_or_else(|| Issue::Erreur("sortie de résistance absente".into()))?;
    let max = trace
        .trouver("résistance maximale")
        .map(|t| t.valeur)
        .unwrap_or(f64::NAN);
    let non_plafonnee = trace
        .trouver("résistance non plafonnée")
        .map(|t| t.valeur)
        .unwrap_or(f64::NAN);
    Ok(dict([
        ("max_effectif", max),
        ("finale", trace.valeur),
        ("surplus", (non_plafonnee - max).max(0.0)),
        ("multiplicateur_subi", 1.0 - trace.valeur / 100.0),
    ]))
}

fn resistances_cible(e: &Entrees) -> Result<Sortie, Issue> {
    let eff = resistance_effective_cible(
        e.nombre("res_cible", None)?,
        e.nombre("penetration", Some(0.0))?,
    );
    Ok(dict([
        ("effective", eff),
        ("multiplicateur", 1.0 - eff / 100.0),
    ]))
}

fn armure_reduction(e: &Entrees) -> Result<Sortie, Issue> {
    let d = e.nombre("degats", None)?;
    let r = reduction_physique(
        e.nombre("armure", None)?,
        d,
        e.nombre("reduction_additionnelle_pct", Some(0.0))?,
    );
    Ok(dict([
        ("reduction_pct", r * 100.0),
        ("degats_subis", d * (1.0 - r)),
    ]))
}

fn ehp_coup_max(e: &Entrees) -> Result<Sortie, Issue> {
    let t = type_degats(&e.texte("type")?)?;
    let mut mods = vec![
        m(stats::LIFE, T::Base, e.nombre("vie", Some(0.0))?),
        m(stats::ENERGY_SHIELD, T::Base, e.nombre("es", Some(0.0))?),
        m(stats::ARMOUR, T::Base, e.nombre("armure", Some(0.0))?),
        m(
            stats::DAMAGE_TAKEN,
            T::Increased,
            e.nombre("increased_subi", Some(0.0))?,
        ),
    ];
    mods.extend(
        e.liste("more_subi")?
            .into_iter()
            .map(|v| m(stats::DAMAGE_TAKEN, T::More, v)),
    );
    let res = e.nombre("resistance", Some(0.0))?;
    if t != TypeDegats::Physique {
        mods.push(m(t.stat_resistance(), T::Added, res));
        // La résistance est donnée finale : on relève le plafond si besoin.
        if res > 75.0 {
            mods.push(m(t.stat_resistance_max(), T::Added, res - 75.0));
        }
    }
    let perso = Personnage {
        modificateurs: mods,
        ..Default::default()
    };
    let r =
        calculer(&perso, &Conditions::default()).map_err(|e| Issue::Erreur(format!("{e:?}")))?;
    let v = r
        .valeur(&format!("coup_max_{}", t.nom()))
        .ok_or_else(|| Issue::Erreur("sortie coup_max absente".into()))?;
    Ok(dict([("coup_max", v)]))
}

fn executer_calcul(calcul: &str, e: &Entrees) -> Result<Sortie, Issue> {
    match calcul {
        "degats.coup_moyen" => degats_coup_moyen(e),
        "critique.multiplicateur" => critique_multiplicateur(e),
        "vitesse.incantation" => vitesse_incantation(e),
        "dps.sort" => dps_sort(e),
        "resistances.joueur" => resistances_joueur(e),
        "resistances.cible" => resistances_cible(e),
        "armure.reduction" => armure_reduction(e),
        "ehp.coup_max" => ehp_coup_max(e),
        "evasion.chance" => Err(Issue::Attente("évasion pas encore dans le moteur".into())),
        "deflexion.chance" => Err(Issue::Attente("déflexion pas encore dans le moteur".into())),
        "es.maximum" => Err(Issue::Attente(
            "bouclier d'énergie local des objets pas encore dans le moteur".into(),
        )),
        "es.recharge" => Err(Issue::Attente(
            "recharge du bouclier d'énergie pas encore dans le moteur".into(),
        )),
        autre => Err(Issue::Erreur(format!("calcul inconnu « {autre} »"))),
    }
}

fn comparer(
    chemin: &str,
    attendu: &Value,
    obtenu: Option<&Sortie>,
    tol_pct: f64,
    ecarts: &mut Vec<Ecart>,
) -> Result<(), String> {
    match attendu {
        Value::Mapping(map) => {
            let sous = match obtenu {
                Some(Sortie::Dict(d)) => Some(d),
                None => None,
                Some(Sortie::Nombre(_)) => return Err(format!("{chemin} : dictionnaire attendu")),
            };
            for (k, v) in map {
                let k = k
                    .as_str()
                    .ok_or_else(|| format!("{chemin} : clé texte attendue"))?;
                let c = if chemin.is_empty() {
                    k.to_string()
                } else {
                    format!("{chemin}.{k}")
                };
                comparer(&c, v, sous.and_then(|d| d.get(k)), tol_pct, ecarts)?;
            }
            Ok(())
        }
        v => {
            let a = v
                .as_f64()
                .ok_or_else(|| format!("{chemin} : nombre attendu"))?;
            let o = match obtenu {
                Some(Sortie::Nombre(o)) => *o,
                // Clé absente : 0, comme l'oracle Python.
                None => 0.0,
                Some(Sortie::Dict(_)) => {
                    return Err(format!("{chemin} : nombre attendu, dictionnaire obtenu"));
                }
            };
            let marge = (a.abs() * tol_pct / 100.0).max(1e-9);
            ecarts.push(Ecart {
                chemin: chemin.to_string(),
                attendu: a,
                obtenu: o,
                ok: (o - a).abs() <= marge,
            });
            Ok(())
        }
    }
}

pub fn executer(cas: &CasFormule) -> ResultatFormule {
    let mut r = ResultatFormule {
        id: cas.id.clone(),
        calcul: cas.calcul.clone(),
        verdict: Verdict::EnAttente,
        ecarts: Vec::new(),
        message: None,
    };
    match cas.genre.as_str() {
        "personnage" => {
            r.message = Some("gabarit de mesure en jeu".into());
            if cas.statut.as_deref() == Some("validé") && cas.attendu.is_some() {
                r.message = Some(
                    "cas personnage validé : à transposer dans tests/golden/archetype/".into(),
                );
            }
            return r;
        }
        "unitaire" => {}
        autre => {
            r.verdict = Verdict::Erreur;
            r.message = Some(format!("type de cas inconnu « {autre} »"));
            return r;
        }
    }
    let Some(calcul) = cas.calcul.as_deref() else {
        r.verdict = Verdict::Erreur;
        r.message = Some("`calcul` manquant".into());
        return r;
    };
    let Some(attendu) = cas.attendu.as_ref().filter(|a| !a.is_null()) else {
        r.verdict = Verdict::Erreur;
        r.message = Some("`attendu` manquant".into());
        return r;
    };
    let e = Entrees {
        map: cas.entrees.as_ref(),
        lues: RefCell::new(BTreeSet::new()),
    };
    match executer_calcul(calcul, &e) {
        Err(Issue::Attente(msg)) => r.message = Some(msg),
        Err(Issue::Erreur(msg)) => {
            r.verdict = Verdict::Erreur;
            r.message = Some(msg);
        }
        Ok(sortie) => {
            let non_lues = e.non_lues();
            if !non_lues.is_empty() {
                r.verdict = Verdict::Erreur;
                r.message = Some(format!("entrées non reconnues : {}", non_lues.join(", ")));
                return r;
            }
            match comparer(
                "",
                attendu,
                Some(&sortie),
                cas.tolerance_pct.unwrap_or(0.01),
                &mut r.ecarts,
            ) {
                Err(msg) => {
                    r.verdict = Verdict::Erreur;
                    r.message = Some(msg);
                }
                Ok(()) => {
                    r.verdict = if r.ecarts.iter().all(|c| c.ok) {
                        Verdict::Reussi
                    } else {
                        Verdict::Echec
                    };
                }
            }
        }
    }
    r
}

/// Rapport Markdown, au même format que celui des cas golden.
pub fn rapport_markdown(resultats: &[ResultatFormule]) -> String {
    let compte = |v: Verdict| resultats.iter().filter(|r| r.verdict == v).count();
    let mut s = String::new();
    let _ = writeln!(
        s,
        "### Cas de formule : {}/{} exécutés au vert, {} en attente\n",
        compte(Verdict::Reussi),
        resultats.len() - compte(Verdict::EnAttente),
        compte(Verdict::EnAttente)
    );
    let _ = writeln!(s, "| Cas | Calcul | Sortie | Attendu | Obtenu | |");
    let _ = writeln!(s, "|---|---|---|---:|---:|---|");
    for r in resultats {
        let calcul = r.calcul.as_deref().unwrap_or("-");
        match r.verdict {
            Verdict::EnAttente => {
                let _ = writeln!(
                    s,
                    "| `{}` | {calcul} | | | | ⏸ {} |",
                    r.id,
                    r.message.as_deref().unwrap_or("")
                );
            }
            Verdict::Erreur => {
                let _ = writeln!(
                    s,
                    "| `{}` | {calcul} | | | | ❌ {} |",
                    r.id,
                    r.message.as_deref().unwrap_or("")
                );
            }
            _ => {
                for c in &r.ecarts {
                    let _ = writeln!(
                        s,
                        "| `{}` | {calcul} | {} | {} | {} | {} |",
                        r.id,
                        c.chemin,
                        nombre(c.attendu),
                        nombre(c.obtenu),
                        if c.ok { "✅" } else { "❌" }
                    );
                }
            }
        }
    }
    s
}
