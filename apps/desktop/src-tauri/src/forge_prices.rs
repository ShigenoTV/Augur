//! Table de prix de Forge (`forge-prix.json` dans le dossier de configuration Augur).
//!
//! Prix en Exalted Orbs : corrections du joueur, sinon dernier relevé poe.ninja, sinon valeurs
//! indicatives (`poe2_core::forge::prix`). Le fichier est écrit uniquement par le cœur Rust ; un
//! fichier abîmé ou ancien est réparé au chargement.
//!
//! poe.ninja (décision de Max du 07/11/2026) : relevé déclenché par le joueur, une requête pour la
//! liste des ligues (à l'ouverture de Réglages) et une par catégorie, User-Agent identifiant Augur,
//! jamais plus d'une actualisation toutes les 5 minutes (10 après un échec).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use poe2_core::forge::ninja::{self, ReleveMarche};
use poe2_core::forge::prix::TablePrix;
use tauri::{AppHandle, Manager};

use crate::contracts::{AppError, ErrorCode, app_error};

#[derive(Default)]
pub struct ForgePrices {
    current: Mutex<TablePrix>,
    /// Date (unix) du dernier échec réseau.
    last_error_at: Mutex<Option<u64>>,
    /// Une actualisation est en cours (bouton pressé deux fois).
    fetching: AtomicBool,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("forge-prix.json"))
}

fn read(bytes: &[u8]) -> TablePrix {
    serde_json::from_slice::<TablePrix>(bytes)
        .map(TablePrix::reparer)
        .unwrap_or_default()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Encode une valeur de paramètre d'URL (nom de ligue avec espaces).
pub(crate) fn encode(v: &str) -> String {
    v.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn network(message: String) -> AppError {
    app_error(ErrorCode::Network, message)
}

pub(crate) async fn get_json(
    client: &reqwest::Client,
    url: &str,
) -> Result<serde_json::Value, AppError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| network(format!("Connexion à poe.ninja impossible ({e}).")))?;
    let status = resp.status();
    if status.as_u16() == 429 {
        return Err(network(
            "poe.ninja limite les requêtes : réessaie dans quelques minutes.".into(),
        ));
    }
    if !status.is_success() {
        return Err(network(format!("poe.ninja a répondu {status}.")));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| network(format!("Réponse poe.ninja illisible ({e}).")))?;
    serde_json::from_str(&body).map_err(|e| network(format!("Réponse poe.ninja inattendue ({e}).")))
}

pub(crate) fn client(version: &str) -> Result<reqwest::Client, AppError> {
    // reqwest est compilé sans fournisseur de chiffrement (« rustls-no-provider », imposé par
    // tauri-plugin-updater) : on installe ring, comme le fait le plugin.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder()
        .user_agent(format!(
            "Augur/{version} (outil personnel de craft PoE 2 ; actualisation manuelle, 5 min au moins)"
        ))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| network(format!("Client HTTP indisponible ({e}).")))
}

/// Liste des ligues de poe.ninja d'un jeu (une requête, à l'ouverture de Réglages).
pub async fn leagues(version: &str, game: crate::game::Game) -> Result<Vec<String>, AppError> {
    let client = client(version)?;
    let api = match game {
        crate::game::Game::Poe2 => ninja::API,
        crate::game::Game::Poe1 => ninja::API_POE1,
    };
    let doc = get_json(&client, &format!("{api}/leagues")).await?;
    let list = ninja::ligues(&doc);
    if list.is_empty() {
        return Err(network("poe.ninja n'a renvoyé aucune ligue.".into()));
    }
    Ok(list)
}

/// Télécharge un relevé poe.ninja (ligue courante si `ligue` est vide).
async fn fetch(
    version: &str,
    ligue: &str,
    precedent: Option<&ReleveMarche>,
) -> Result<ReleveMarche, AppError> {
    let client = client(version)?;
    let ligue = match ligue.trim() {
        "" => ninja::premiere_ligue(&get_json(&client, &format!("{}/leagues", ninja::API)).await?)
            .map_err(|e| network(e.to_string()))?,
        l => l.to_string(),
    };
    let mut reponses = Vec::new();
    for categorie in ninja::CATEGORIES {
        let url = format!(
            "{}/exchange/current/overview?league={}&type={categorie}",
            ninja::API,
            encode(&ligue)
        );
        reponses.push((categorie, get_json(&client, &url).await?));
    }
    ninja::releve(&ligue, now_unix(), &reponses, precedent).map_err(|e| network(e.to_string()))
}

impl ForgePrices {
    pub fn load(app: &AppHandle) -> Self {
        let table = file(app)
            .and_then(|p| std::fs::read(p).ok())
            .map(|b| read(&b))
            .unwrap_or_default();
        Self {
            current: Mutex::new(table),
            ..Default::default()
        }
    }

    pub fn get(&self) -> TablePrix {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn save(app: &AppHandle, table: &TablePrix) -> Result<(), AppError> {
        let Some(path) = file(app) else {
            return Ok(());
        };
        let write = || -> std::io::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let json = serde_json::to_vec_pretty(table).map_err(std::io::Error::other)?;
            std::fs::write(&path, json)
        };
        write().map_err(|e| app_error(ErrorCode::Internal, format!("Prix non enregistrés ({e}).")))
    }

    fn change(
        &self,
        app: &AppHandle,
        f: impl FnOnce(&mut TablePrix) -> Result<(), AppError>,
    ) -> Result<TablePrix, AppError> {
        let mut guard = self.current.lock().unwrap_or_else(|e| e.into_inner());
        let mut table = guard.clone();
        f(&mut table)?;
        Self::save(app, &table)?;
        *guard = table.clone();
        Ok(table)
    }

    /// Corrige des prix ; refuse toute la saisie si une valeur est invalide.
    pub fn set(
        &self,
        app: &AppHandle,
        saisie: &BTreeMap<String, f64>,
    ) -> Result<TablePrix, AppError> {
        self.change(app, |t| {
            t.saisir(saisie)
                .map_err(|e| app_error(ErrorCode::Unsupported, e.to_string()))
        })
    }

    /// Oublie les corrections du joueur (le relevé poe.ninja reste).
    pub fn reset(&self, app: &AppHandle) -> Result<TablePrix, AppError> {
        self.change(app, |t| {
            t.effacer_corrections();
            Ok(())
        })
    }

    /// Actualise les prix depuis poe.ninja, dans la limite d'une fois toutes les 5 minutes.
    pub async fn refresh(&self, app: &AppHandle, ligue: &str) -> Result<TablePrix, AppError> {
        let precedent = self.get().marche;
        let meme_ligue =
            ligue.trim().is_empty() || precedent.as_ref().is_some_and(|m| m.ligue == ligue.trim());
        let dernier = precedent
            .as_ref()
            .filter(|_| meme_ligue)
            .map(|m| m.releve_a);
        let erreur = *self.last_error_at.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(attente) = ninja::actualisation_permise(now_unix(), dernier, erreur) {
            return Err(AppError {
                code: ErrorCode::RateLimited,
                message: format!(
                    "Prix déjà actualisés récemment : nouvel essai possible dans {} min.",
                    attente.div_ceil(60)
                ),
                retry_after_ms: Some(u32::try_from(attente * 1000).unwrap_or(u32::MAX)),
            });
        }
        if self.fetching.swap(true, Ordering::SeqCst) {
            return Err(app_error(
                ErrorCode::RateLimited,
                "Actualisation des prix déjà en cours.",
            ));
        }
        let version = app.package_info().version.to_string();
        let res = fetch(&version, ligue, precedent.as_ref()).await;
        self.fetching.store(false, Ordering::SeqCst);
        match res {
            Ok(releve) => {
                *self.last_error_at.lock().unwrap_or_else(|e| e.into_inner()) = None;
                self.change(app, |t| {
                    t.appliquer_marche(releve);
                    Ok(())
                })
            }
            Err(e) => {
                *self.last_error_at.lock().unwrap_or_else(|e| e.into_inner()) = Some(now_unix());
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relit_un_fichier_abime() {
        assert_eq!(read(b"pas du json"), TablePrix::indicative());
        let t = read(br#"{"corrections":{"exalt":3}}"#);
        assert_eq!(t.prix["exalt"], 3.0);
        assert_eq!(t.prix.len(), TablePrix::indicative().prix.len());
    }

    #[test]
    fn encode_les_noms_de_ligue() {
        assert_eq!(encode("Forbidden Rites"), "Forbidden%20Rites");
        assert_eq!(encode("HC Runes of Aldur"), "HC%20Runes%20of%20Aldur");
        assert_eq!(encode("a&b=c"), "a%26b%3Dc");
    }
}
