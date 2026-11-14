//! Coffres de PoE 1 : relevé des onglets choisis et valeur au prix poe.ninja.
//!
//! Voie non officielle validée par Max le 14/11/2026 (usage personnel) : les
//! anciens points d'accès du site (`character-window/get-stash-items`) avec la
//! session `POESESSID` du joueur. Garde-fous :
//! - la session est lue dans le Gestionnaire d'identification Windows
//!   ([`crate::secret`]) et n'est envoyée qu'à `www.pathofexile.com`, sans suivre
//!   de redirection ;
//! - lecture seule, un onglet à la fois, une pause de [`PAUSE`] entre deux
//!   requêtes, au plus [`MAX_ONGLETS`] onglets suivis ;
//! - arrêt au premier refus (403, 429) avec un message clair ;
//! - un relevé ne garde que des noms d'objets, des quantités et des valeurs
//!   (`coffres-poe1.json`), jamais la session.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use poe2_core::poe1::coffre::{self, Ecart, Onglet, Releve};
use poe2_core::poe1::ninja;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::contracts::{AppError, ErrorCode, app_error};
use crate::game::Game;
use crate::settings::SettingsStore;

/// Événement émis après chaque relevé (charge : [`StashStatus`]).
pub const EVENT_CHANGED: &str = "augur://poe1-stash";

/// Pause entre deux requêtes au site (la limite du site tourne autour de
/// 30 requêtes par minute pour les coffres).
const PAUSE: Duration = Duration::from_millis(2500);
/// Onglets suivis au plus.
pub const MAX_ONGLETS: usize = 20;
/// Seule adresse à qui la session est envoyée.
const SITE: &str = "https://www.pathofexile.com/character-window/get-stash-items";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Fichier {
    precedent: Option<Releve>,
    dernier: Option<Releve>,
}

#[derive(Default)]
pub struct Poe1Stash {
    releves: Mutex<Fichier>,
    occupe: AtomicBool,
}

/// État affiché par la carte « Coffres PoE 1 ».
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashStatus {
    pub account: Option<String>,
    pub has_session: bool,
    pub league: String,
    pub tabs: Vec<u32>,
    pub busy: bool,
    pub last: Option<Releve>,
    pub previous: Option<Releve>,
    /// Gain du dernier relevé sur le précédent, en Chaos.
    pub gain: Option<f64>,
    /// Objets qui ont le plus bougé entre les deux relevés.
    pub changes: Vec<Ecart>,
}

fn fichier(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("coffres-poe1.json"))
}

impl Poe1Stash {
    pub fn load(app: &AppHandle) -> Self {
        let f = fichier(app)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            releves: Mutex::new(f),
            occupe: AtomicBool::new(false),
        }
    }

    fn enregistrer(&self, app: &AppHandle, f: &Fichier) {
        if let Some(p) = fichier(app) {
            let ecrit = p
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| {
                    std::fs::write(
                        &p,
                        serde_json::to_vec_pretty(f).map_err(std::io::Error::other)?,
                    )
                });
            if let Err(e) = ecrit {
                log::warn!("relevé des coffres non enregistré : {e}");
            }
        }
    }
}

pub fn status(app: &AppHandle) -> StashStatus {
    let settings = app.state::<SettingsStore>().get();
    let state = app.state::<Poe1Stash>();
    let f = state
        .releves
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let (gain, changes) = match (&f.precedent, &f.dernier) {
        (Some(a), Some(b)) if a.ligue == b.ligue => {
            let (g, mut e) = coffre::difference(a, b);
            e.truncate(12);
            (Some(g), e)
        }
        _ => (None, Vec::new()),
    };
    StashStatus {
        account: settings.poe1.account.clone(),
        has_session: crate::secret::lire().is_some(),
        league: settings.league_for(Game::Poe1).to_string(),
        tabs: settings.poe1.stash_tabs.clone(),
        busy: state.occupe.load(Ordering::Relaxed),
        last: f.dernier,
        previous: f.precedent,
        gain,
        changes,
    }
}

fn erreur(message: impl Into<String>) -> AppError {
    app_error(ErrorCode::Internal, message.into())
}

/// Client du site : pas de redirection suivie (la session ne part nulle part ailleurs).
fn client_site(version: &str) -> Result<reqwest::Client, AppError> {
    // Même fournisseur de chiffrement que le client poe.ninja.
    let _ = crate::forge_prices::client(version)?;
    reqwest::Client::builder()
        .user_agent(format!(
            "Augur/{version} (outil personnel, lecture des coffres)"
        ))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| erreur(format!("Client HTTP indisponible ({e}).")))
}

async fn lire_onglet(
    client: &reqwest::Client,
    sid: &str,
    compte: &str,
    ligue: &str,
    index: u32,
    avec_liste: bool,
) -> Result<Value, AppError> {
    let enc = crate::forge_prices::encode;
    let url = format!(
        "{SITE}?league={}&realm=pc&accountName={}&tabs={}&tabIndex={index}",
        enc(ligue),
        enc(compte),
        u8::from(avec_liste)
    );
    let resp = client
        .get(&url)
        .header("Cookie", format!("POESESSID={sid}"))
        .send()
        .await
        .map_err(|e| {
            app_error(
                ErrorCode::Network,
                format!("pathofexile.com injoignable ({e})."),
            )
        })?;
    let code = resp.status().as_u16();
    match code {
        200 => {}
        429 => {
            let attente = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("quelques");
            return Err(app_error(
                ErrorCode::Network,
                format!("pathofexile.com limite les lectures : réessaie dans {attente} secondes."),
            ));
        }
        301..=303 | 307 | 308 | 401 | 403 => {
            return Err(erreur(
                "pathofexile.com refuse la lecture : vérifie le nom de compte (avec son #1234) et la ligue, \
                 ou reconnecte-toi sur le site et recopie une nouvelle valeur de POESESSID.",
            ));
        }
        404 => {
            return Err(erreur(format!(
                "Ligue « {ligue} » ou onglet {index} introuvable."
            )));
        }
        _ => {
            return Err(app_error(
                ErrorCode::Network,
                format!("pathofexile.com a répondu {code}."),
            ));
        }
    }
    let texte = resp
        .text()
        .await
        .map_err(|e| app_error(ErrorCode::Network, format!("Réponse illisible ({e}).")))?;
    let doc: Value = serde_json::from_str(&texte).map_err(|_| {
        erreur("Réponse inattendue de pathofexile.com (pas des données de coffre).")
    })?;
    if let Some(m) = doc["error"]["message"].as_str() {
        return Err(erreur(format!("pathofexile.com : {m}")));
    }
    Ok(doc)
}

fn identifiants(app: &AppHandle) -> Result<(String, String, String), AppError> {
    let s = app.state::<SettingsStore>().get();
    let compte = s
        .poe1
        .account
        .clone()
        .filter(|c| !c.trim().is_empty())
        .ok_or_else(|| erreur("Indique d'abord ton nom de compte pathofexile.com."))?;
    let sid = crate::secret::lire()
        .ok_or_else(|| erreur("Indique d'abord ta session pathofexile.com (POESESSID)."))?;
    Ok((compte, sid, s.league_for(Game::Poe1).to_string()))
}

/// Enregistre le compte (et la session si elle est donnée), puis vérifie l'accès
/// en lisant la liste des onglets.
pub async fn connecter(
    app: &AppHandle,
    compte: String,
    sid: Option<String>,
) -> Result<Vec<Onglet>, AppError> {
    let compte = compte.trim().to_string();
    if compte.is_empty() || !compte.contains('#') {
        return Err(erreur(
            "Nom de compte attendu au format « Nom#1234 » (visible sur ta page de compte).",
        ));
    }
    if let Some(sid) = sid.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !crate::secret::valide(sid) {
            return Err(erreur(
                "Ce n'est pas une valeur de POESESSID (32 caractères, chiffres et lettres a à f).",
            ));
        }
        crate::secret::ecrire(sid).map_err(erreur)?;
    }
    app.state::<SettingsStore>()
        .update(app, |s| s.poe1.account = Some(compte))
        .map_err(|e| erreur(format!("Réglage non enregistré ({e}).")))?;
    onglets(app).await
}

/// Oublie la session (le nom de compte et les relevés restent).
pub fn oublier(app: &AppHandle) {
    crate::secret::effacer();
    let _ = app.emit(EVENT_CHANGED, status(app));
}

/// Liste des onglets du compte, dans la ligue PoE 1 choisie (une requête).
pub async fn onglets(app: &AppHandle) -> Result<Vec<Onglet>, AppError> {
    let (compte, sid, ligue) = identifiants(app)?;
    let client = client_site(&app.package_info().version.to_string())?;
    let doc = lire_onglet(&client, &sid, &compte, &ligue, 0, true).await?;
    Ok(coffre::onglets(&doc))
}

/// Onglets suivis (au plus [`MAX_ONGLETS`]).
pub fn choisir(app: &AppHandle, mut index: Vec<u32>) -> Result<StashStatus, AppError> {
    index.sort_unstable();
    index.dedup();
    if index.len() > MAX_ONGLETS {
        return Err(erreur(format!(
            "Choisis au plus {MAX_ONGLETS} onglets : chaque onglet est une lecture sur le site."
        )));
    }
    app.state::<SettingsStore>()
        .update(app, |s| s.poe1.stash_tabs = index)
        .map_err(|e| erreur(format!("Réglage non enregistré ({e}).")))?;
    Ok(status(app))
}

struct Libere<'a>(&'a AtomicBool);

impl Drop for Libere<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// Relève les onglets suivis et leur valeur, et garde le relevé précédent.
pub async fn relever(app: &AppHandle) -> Result<StashStatus, AppError> {
    let (compte, sid, ligue) = identifiants(app)?;
    let tabs = app.state::<SettingsStore>().get().poe1.stash_tabs;
    if tabs.is_empty() {
        return Err(erreur("Choisis d'abord les onglets à suivre."));
    }
    let state = app.state::<Poe1Stash>();
    if state.occupe.swap(true, Ordering::Relaxed) {
        return Err(erreur("Un relevé est déjà en cours."));
    }
    let _libere = Libere(&state.occupe);
    let _ = app.emit(EVENT_CHANGED, status(app));

    let version = app.package_info().version.to_string();
    let client = client_site(&version)?;
    // La liste des onglets vient avec le premier onglet lu (noms à jour).
    let mut lus: Vec<(Onglet, Vec<coffre::ObjetCoffre>)> = Vec::new();
    let mut noms: Vec<Onglet> = Vec::new();
    for (n, &index) in tabs.iter().enumerate() {
        if n > 0 {
            tokio::time::sleep(PAUSE).await;
        }
        let doc = lire_onglet(&client, &sid, &compte, &ligue, index, n == 0).await?;
        if n == 0 {
            noms = coffre::onglets(&doc);
        }
        let onglet = noms
            .iter()
            .find(|o| o.index == index)
            .cloned()
            .unwrap_or(Onglet {
                index,
                nom: format!("Onglet {index}"),
                genre: String::new(),
            });
        lus.push((onglet, coffre::objets(&doc)));
    }

    // Pages poe.ninja nécessaires (gardées 30 min par l'overlay de PoE 1).
    let ninja_client = crate::forge_prices::client(&version)?;
    let cache = app.state::<crate::poe1_items::Poe1Ninja>();
    let mut pages = std::collections::HashMap::new();
    let devises = ninja::Page::Echange("Currency");
    let mut a_lire = coffre::pages_necessaires(lus.iter().flat_map(|(_, o)| o));
    if !a_lire.contains(&devises) {
        a_lire.insert(0, devises);
    }
    for p in a_lire {
        match cache.page(&ninja_client, &ligue, p).await {
            Ok(doc) => {
                pages.insert(p, doc);
            }
            Err(e) => log::warn!("poe.ninja, page {} : {}", p.chemin(), e.message),
        }
    }
    let cours = pages.get(&devises).and_then(ninja::cours_divine);
    let releve = coffre::evaluer(
        crate::time::rfc3339_now(),
        ligue,
        &lus,
        &|p| pages.get(&p).cloned(),
        cours,
    );

    let f = {
        let mut g = state.releves.lock().unwrap_or_else(|e| e.into_inner());
        g.precedent = g.dernier.take();
        g.dernier = Some(releve);
        g.clone()
    };
    state.enregistrer(app, &f);
    drop(_libere);
    let s = status(app);
    let _ = app.emit(EVENT_CHANGED, &s);
    Ok(s)
}
