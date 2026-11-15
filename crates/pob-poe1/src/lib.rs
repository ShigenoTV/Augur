//! Moteur de calcul de PoE 1 : Path of Building Community (PoE 1), sans fenêtre.
//!
//! Augur n'a pas de moteur PoE 1 à lui. Il fait tourner celui de Path of Building
//! (Lua, licence MIT, voir `paquet/LICENSE` dans l'archive) dans LuaJIT : on lui
//! passe le build importé (XML du code PoB) et le texte de l'objet copié en jeu
//! (Ctrl+C), il rend le DPS, l'EHP, la vie et les résistances avant et après.
//!
//! Le paquet `paquet/pob-poe1.tar.gz` (≈ 5 Mo, préparé par
//! `tools/pob/paquet_pob_poe1.py`) est embarqué dans l'appli et décompressé au
//! premier lancement dans le dossier de cache. Lua n'est pas partagé entre threads :
//! un thread dédié possède l'interpréteur et traite les demandes une par une.
//! Le démarrage de PoB prend quelques secondes, d'où le lancement en tâche de fond.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use mlua::{Function, Lua, LuaSerdeExt, Value};
use serde::{Deserialize, Serialize};

mod lua_utf8;

/// Archive de Path of Building embarquée dans l'appli.
static PAQUET: &[u8] = include_bytes!("../paquet/pob-poe1.tar.gz");
/// Version de Path of Building du paquet (ex. « 2.67.2+303c104 (arbre 3.30) »).
pub const VERSION: &str = include_str!("../paquet/VERSION");
static ENVELOPPE: &str = include_str!("augur_pob.lua");

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
pub enum ErreurPob {
    #[error("Path of Building n'a pas pu démarrer : {0}")]
    Demarrage(String),
    #[error("{0}")]
    Calcul(String),
    #[error("Aucun build PoE 1 chargé : importe ton code Path of Building.")]
    SansBuild,
    #[error("Le moteur de calcul PoE 1 s'est arrêté.")]
    Arrete,
}

/// Le build tel que PoB l'a compris.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoBuild {
    pub class_name: Option<String>,
    pub ascendancy: Option<String>,
    pub level: Option<u32>,
    pub main_skill: Option<String>,
    pub tree_version: Option<String>,
    #[serde(default)]
    pub full_dps: f64,
    #[serde(default)]
    pub combined_dps: f64,
    #[serde(default)]
    pub life: f64,
    #[serde(default)]
    pub ehp: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Resistance {
    pub value: f64,
    pub cap: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resistances {
    pub fire: Resistance,
    pub cold: Resistance,
    pub lightning: Resistance,
    pub chaos: Resistance,
}

/// Les chiffres que l'overlay compare.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resume {
    pub full_dps: f64,
    pub combined_dps: f64,
    pub life: f64,
    pub energy_shield: f64,
    pub mana: f64,
    /// EHP comme pour PoE 2 : moyenne des plus gros coups encaissables des cinq types de dégâts.
    pub ehp: f64,
    pub resistances: Resistances,
}

impl Resume {
    /// DPS de référence : « Full DPS » de PoB s'il est calculé, sinon le DPS combiné
    /// de la compétence principale.
    pub fn dps(&self) -> f64 {
        if self.full_dps > 0.0 {
            self.full_dps
        } else {
            self.combined_dps
        }
    }
}

/// Une ligne de l'infobulle de comparaison de PoB (« Life », « Total DPS »…).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ligne {
    pub label: String,
    pub before: f64,
    pub after: f64,
    #[serde(default)]
    pub lower_is_better: bool,
}

/// Résultat pour un emplacement (les deux anneaux et les deux mains sont essayés).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Emplacement {
    /// Nom PoB de l'emplacement (« Ring 1 », « Body Armour »…).
    pub slot: String,
    pub equipped: Option<String>,
    pub after: Resume,
    #[serde(default)]
    pub lines: Vec<Ligne>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparaison {
    pub before: Resume,
    pub slots: Vec<Emplacement>,
    pub compute_ms: u32,
}

impl Comparaison {
    /// L'emplacement où l'objet rapporte le plus (DPS d'abord, puis EHP).
    pub fn meilleur(&self) -> Option<&Emplacement> {
        self.slots.iter().max_by(|a, b| {
            let gain = |e: &Emplacement| {
                (
                    e.after.dps() - self.before.dps(),
                    e.after.ehp - self.before.ehp,
                )
            };
            gain(a)
                .partial_cmp(&gain(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

type Reponse<T> = mpsc::Sender<Result<T, ErreurPob>>;

/// Recherche d'upgrades générée par PoB pour un emplacement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RechercheUpgrade {
    /// Requête du site de trade (`query` + `sort`).
    pub requete: serde_json::Value,
    /// Stats pesées les plus fortes (texte du site de trade, « # » pour la valeur).
    pub stats: Vec<String>,
    /// Nombre de stats pesées.
    pub filtres: usize,
    pub compute_ms: u32,
}

/// Prix maximum de la recherche : montant et devise du site (« chaos », « divine »).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Budget {
    pub max: u32,
    pub devise: String,
}

enum Demande {
    Charger(String, Reponse<InfoBuild>),
    Comparer(String, Reponse<Comparaison>),
    Upgrade(String, Option<Budget>, Reponse<RechercheUpgrade>),
}

/// Poignée vers le thread qui possède Path of Building.
pub struct Moteur {
    tx: Mutex<mpsc::Sender<Demande>>,
    /// XML du build chargé, pour ne pas le recharger à chaque objet.
    charge: Arc<Mutex<Option<String>>>,
}

impl Moteur {
    /// Lance PoB en tâche de fond. `cache` reçoit le paquet décompressé.
    pub fn demarrer(cache: PathBuf) -> Moteur {
        let (tx, rx) = mpsc::channel::<Demande>();
        let charge = Arc::new(Mutex::new(None));
        std::thread::Builder::new()
            .name("pob-poe1".into())
            .spawn(move || boucle(&cache, rx))
            .expect("thread pob-poe1");
        Moteur {
            tx: Mutex::new(tx),
            charge,
        }
    }

    fn envoyer<T>(&self, fabrique: impl FnOnce(Reponse<T>) -> Demande) -> Result<T, ErreurPob> {
        let (rtx, rrx) = mpsc::channel();
        self.tx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send(fabrique(rtx))
            .map_err(|_| ErreurPob::Arrete)?;
        rrx.recv().map_err(|_| ErreurPob::Arrete)?
    }

    /// Charge le build (XML d'un code PoB PoE 1). Bloquant : jusqu'à quelques secondes.
    pub fn charger(&self, xml: &str) -> Result<InfoBuild, ErreurPob> {
        let info = self.envoyer(|r| Demande::Charger(xml.to_string(), r))?;
        *self.charge.lock().unwrap_or_else(|e| e.into_inner()) = Some(xml.to_string());
        Ok(info)
    }

    /// Le build `xml` est-il celui déjà chargé ?
    pub fn est_charge(&self, xml: &str) -> bool {
        self.charge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
            == Some(xml)
    }

    /// Compare l'objet copié en jeu à celui porté par le build chargé.
    pub fn comparer(&self, texte_objet: &str) -> Result<Comparaison, ErreurPob> {
        if self
            .charge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none()
        {
            return Err(ErreurPob::SansBuild);
        }
        self.envoyer(|r| Demande::Comparer(texte_objet.to_string(), r))
    }

    /// Recherche de trade pondérée pour l'emplacement PoB `slot` (« Ring 1 »…).
    pub fn recherche_upgrade(
        &self,
        slot: &str,
        budget: Option<Budget>,
    ) -> Result<RechercheUpgrade, ErreurPob> {
        if self
            .charge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none()
        {
            return Err(ErreurPob::SansBuild);
        }
        self.envoyer(|r| Demande::Upgrade(slot.to_string(), budget, r))
    }
}

fn boucle(cache: &Path, rx: mpsc::Receiver<Demande>) {
    let pob = demarrer_pob(cache);
    if let Err(e) = &pob {
        log_erreur(e);
    }
    for demande in rx {
        match (&pob, demande) {
            (Err(e), Demande::Charger(_, r)) => {
                let _ = r.send(Err(e.clone()));
            }
            (Err(e), Demande::Comparer(_, r)) => {
                let _ = r.send(Err(e.clone()));
            }
            (Err(e), Demande::Upgrade(_, _, r)) => {
                let _ = r.send(Err(e.clone()));
            }
            (Ok(lua), Demande::Upgrade(slot, budget, r)) => {
                let _ = r.send(upgrade(lua, &slot, budget));
            }
            (Ok(lua), Demande::Charger(xml, r)) => {
                let _ = r.send(charger(lua, &xml));
            }
            (Ok(lua), Demande::Comparer(texte, r)) => {
                let _ = r.send(comparer(lua, &texte));
            }
        }
    }
}

fn log_erreur(e: &ErreurPob) {
    eprintln!("pob-poe1 : {e}");
}

/// Empreinte courte du paquet : un nouveau paquet va dans un nouveau dossier.
fn empreinte(octets: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in octets {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Décompresse le paquet dans `cache/<empreinte>` s'il n'y est pas déjà.
pub fn deployer(cache: &Path) -> Result<PathBuf, ErreurPob> {
    let dossier = cache.join(empreinte(PAQUET));
    let temoin = dossier.join(".complet");
    if temoin.exists() {
        return Ok(dossier);
    }
    let err = |e: std::io::Error| ErreurPob::Demarrage(format!("décompression du paquet : {e}"));
    if dossier.exists() {
        std::fs::remove_dir_all(&dossier).map_err(err)?;
    }
    std::fs::create_dir_all(&dossier).map_err(err)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(PAQUET));
    archive.unpack(&dossier).map_err(err)?;
    std::fs::create_dir_all(dossier.join("user")).map_err(err)?;
    std::fs::write(&temoin, VERSION).map_err(err)?;
    // Les anciens paquets ne servent plus.
    if let Ok(entrees) = std::fs::read_dir(cache) {
        for e in entrees.flatten() {
            if e.path() != dossier && e.path().is_dir() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(dossier)
}

fn chemin_lua(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn demarrer_pob(cache: &Path) -> Result<Lua, ErreurPob> {
    let dossier = deployer(cache)?;
    let err = |e: mlua::Error| ErreurPob::Demarrage(texte_erreur(&e));
    let lua = Lua::new();
    let g = lua.globals();
    g.set("AUGUR_BASE", chemin_lua(&dossier)).map_err(err)?;
    g.set(
        "AUGUR_INFLATE",
        lua.create_function(|lua, data: mlua::LuaString| {
            let octets = data.as_bytes();
            let mut sortie = Vec::new();
            let ok = flate2::read::ZlibDecoder::new(&octets[..])
                .read_to_end(&mut sortie)
                .is_ok()
                || {
                    sortie.clear();
                    flate2::read::GzDecoder::new(&octets[..])
                        .read_to_end(&mut sortie)
                        .is_ok()
                };
            if ok {
                Ok((Some(lua.create_string(&sortie)?), None))
            } else {
                Ok((None, Some("données compressées illisibles".to_string())))
            }
        })
        .map_err(err)?,
    )
    .map_err(err)?;
    g.set(
        "AUGUR_DEFLATE",
        lua.create_function(|lua, (data, gzip): (mlua::LuaString, Option<bool>)| {
            let octets = data.as_bytes();
            let sortie = if gzip.unwrap_or(false) {
                let mut e =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(&octets).and_then(|_| e.finish())
            } else {
                let mut e =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(&octets).and_then(|_| e.finish())
            }
            .map_err(mlua::Error::external)?;
            lua.create_string(&sortie)
        })
        .map_err(err)?,
    )
    .map_err(err)?;
    lua_utf8::installer(&lua).map_err(err)?;
    lua.load(ENVELOPPE)
        .set_name("augur_pob")
        .exec()
        .map_err(err)?;
    Ok(lua)
}

/// Message lisible : la première ligne utile, sans les codes couleur de PoB (^1, ^7…).
fn texte_erreur(e: &mlua::Error) -> String {
    let brut = match e {
        mlua::Error::RuntimeError(m) => m.clone(),
        mlua::Error::CallbackError { cause, .. } => return texte_erreur(cause),
        autre => autre.to_string(),
    };
    let mut propre = String::with_capacity(brut.len());
    let mut car = brut.chars().peekable();
    while let Some(c) = car.next() {
        if c == '^' && car.peek().is_some_and(|n| n.is_ascii_hexdigit()) {
            car.next();
            continue;
        }
        propre.push(c);
    }
    propre
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "Error:" && !l.starts_with("stack traceback"))
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}

fn appel<T: serde::de::DeserializeOwned>(lua: &Lua, nom: &str, arg: &str) -> Result<T, ErreurPob> {
    let err = |e: mlua::Error| ErreurPob::Calcul(texte_erreur(&e));
    let f: Function = lua.globals().get(nom).map_err(err)?;
    let v: Value = f.call(arg).map_err(err)?;
    if let Value::Table(t) = &v
        && let Ok(Some(msg)) = t.get::<Option<String>>("error")
    {
        return Err(ErreurPob::Calcul(msg));
    }
    lua.from_value(v).map_err(err)
}

fn charger(lua: &Lua, xml: &str) -> Result<InfoBuild, ErreurPob> {
    appel(lua, "augur_load", xml)
}

fn comparer(lua: &Lua, texte: &str) -> Result<Comparaison, ErreurPob> {
    let debut = Instant::now();
    #[derive(Deserialize)]
    struct Brut {
        before: Resume,
        #[serde(default)]
        slots: Vec<Emplacement>,
    }
    let brut: Brut = appel(lua, "augur_compare", &texte.replace("\r\n", "\n"))?;
    Ok(Comparaison {
        before: brut.before,
        slots: brut.slots,
        compute_ms: debut.elapsed().as_millis().min(u128::from(u32::MAX)) as u32,
    })
}

fn upgrade(lua: &Lua, slot: &str, budget: Option<Budget>) -> Result<RechercheUpgrade, ErreurPob> {
    let debut = Instant::now();
    let err = |e: mlua::Error| ErreurPob::Calcul(texte_erreur(&e));
    let args = lua.create_table().map_err(err)?;
    args.set("slot", slot).map_err(err)?;
    if let Some(b) = budget.filter(|b| b.max > 0) {
        args.set("maxPrice", b.max).map_err(err)?;
        args.set("maxPriceType", b.devise).map_err(err)?;
    }
    let f: Function = lua.globals().get("augur_upgrade_query").map_err(err)?;
    let v: Value = f.call(args).map_err(err)?;
    if let Value::Table(t) = &v
        && let Ok(Some(msg)) = t.get::<Option<String>>("error")
    {
        return Err(ErreurPob::Calcul(msg));
    }
    #[derive(Deserialize)]
    struct Brut {
        query: serde_json::Value,
        #[serde(default)]
        stats: Vec<String>,
        #[serde(default)]
        filters: usize,
    }
    let brut: Brut = lua.from_value(v).map_err(err)?;
    Ok(RechercheUpgrade {
        requete: serde_json::json!({ "query": brut.query, "sort": { "statgroup.0": "desc" } }),
        stats: brut.stats,
        filtres: brut.filters,
        compute_ms: debut.elapsed().as_millis().min(u128::from(u32::MAX)) as u32,
    })
}

#[cfg(test)]
mod tests;
