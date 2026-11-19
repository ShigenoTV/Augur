//! Mise à jour de l'application depuis l'application.
//!
//! - Vérification au démarrage puis toutes les 6 heures, et à la demande.
//! - Le joueur décide : rien ne s'installe sans un clic sur « Installer et
//!   redémarrer », donc jamais en pleine partie à son insu.
//! - Les paquets sont signés (minisign) et vérifiés par le plugin updater
//!   avant installation ; un paquet mal signé est refusé.
//! - Deux canaux servis par GitHub Releases : stable (release « latest ») et
//!   bêta (manifeste attaché à la pré-release fixe `canal-beta`, que
//!   « Promouvoir une release » met à jour). Réglage « Rejoindre la bêta ».
//!
//! Chaîne de publication : docs/devops/release-windows.md (chantier DevOps).

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::contracts::{AppError, ErrorCode, app_error};
use crate::settings::SettingsStore;
use crate::time::rfc3339_from_unix;

/// Manifeste de mise à jour du canal stable, servi par les releases GitHub
/// tant que le studio n'a pas de domaine (docs/devops/release-windows.md).
/// Seule constante à changer le jour où un domaine est choisi.
pub const STABLE_ENDPOINT: &str =
    "https://github.com/ShigenoTV/Augur/releases/latest/download/latest.json";

/// Manifeste du canal bêta : fichier `latest.json` de la pré-release fixe
/// `canal-beta`. Une pré-release n'est jamais « latest » sur GitHub, donc le
/// canal stable ne la voit pas. Chaque promotion (bêta ou stable) y dépose le
/// manifeste de la version la plus récente : les testeurs reçoivent aussi les
/// versions stables.
pub const BETA_ENDPOINT: &str =
    "https://github.com/ShigenoTV/Augur/releases/download/canal-beta/latest.json";

/// Option « Rejoindre la bêta » proposée dans Réglages.
pub const BETA_AVAILABLE: bool = true;

pub const EVENT_AVAILABLE: &str = "augur://update-available";
pub const EVENT_PROGRESS: &str = "augur://update-progress";
/// Demande de vérification venue du menu de la zone de notification.
pub const EVENT_CHECK_REQUESTED: &str = "augur://update-check-requested";

const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Channel {
    Stable,
    Beta,
}

impl Channel {
    pub fn from_beta(beta: bool) -> Self {
        if beta { Self::Beta } else { Self::Stable }
    }
}

/// Manifestes interrogés, dans l'ordre : le plugin passe au suivant si le
/// précédent ne répond pas. En bêta, le stable sert de secours (canal bêta pas
/// encore créé, GitHub indisponible pour ce fichier…).
pub fn endpoints(channel: Channel) -> &'static [&'static str] {
    match channel {
        Channel::Stable => &[STABLE_ENDPOINT],
        Channel::Beta => &[BETA_ENDPOINT, STABLE_ENDPOINT],
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub channel: Channel,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Mise à jour trouvée, en attente de la décision du joueur.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

fn updater_error(err: impl std::fmt::Display) -> AppError {
    log::warn!("vérification des mises à jour impossible : {err}");
    app_error(
        ErrorCode::Network,
        format!("Impossible de vérifier les mises à jour pour le moment ({err})."),
    )
}

pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, AppError> {
    let channel =
        Channel::from_beta(BETA_AVAILABLE && app.state::<SettingsStore>().get().beta_channel);
    let urls = endpoints(channel)
        .iter()
        .map(|u| Url::parse(u))
        .collect::<Result<Vec<_>, _>>()
        .map_err(updater_error)?;
    let update = app
        .updater_builder()
        .endpoints(urls)
        .map_err(updater_error)?
        .build()
        .map_err(updater_error)?
        .check()
        .await
        .map_err(updater_error)?;

    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
        published_at: u.date.map(|d| rfc3339_from_unix(d.unix_timestamp())),
        channel,
    });
    *app.state::<PendingUpdate>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = update;
    Ok(info)
}

pub async fn install(app: &AppHandle) -> Result<(), AppError> {
    let update = app
        .state::<PendingUpdate>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or_else(|| app_error(ErrorCode::Internal, "Aucune mise à jour en attente."))?;

    let mut downloaded = 0u64;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit(EVENT_PROGRESS, UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|err| {
            app_error(
                ErrorCode::Internal,
                format!("La mise à jour a échoué ({err})."),
            )
        })?;

    // Sous Windows, l'installeur ferme Augur lui-même (mode « passive »).
    app.restart();
}

/// Vérification au démarrage puis périodique ; la fenêtre principale
/// affiche un bandeau si une version est disponible.
pub fn spawn_periodic_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio_sleep(FIRST_CHECK_DELAY).await;
        loop {
            match check(&app).await {
                Ok(Some(info)) => {
                    let _ = app.emit(EVENT_AVAILABLE, info);
                }
                Ok(None) => {}
                Err(err) => log::info!("vérification des mises à jour : {}", err.message),
            }
            tokio_sleep(CHECK_INTERVAL).await;
        }
    });
}

async fn tokio_sleep(d: Duration) {
    // Pas de dépendance directe à tokio : une attente bloquante dans le pool dédié.
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construit_les_endpoints_par_canal() {
        assert_eq!(
            endpoints(Channel::Stable),
            ["https://github.com/ShigenoTV/Augur/releases/latest/download/latest.json"]
        );
        // Bêta d'abord, stable en secours.
        assert_eq!(
            endpoints(Channel::Beta),
            [
                "https://github.com/ShigenoTV/Augur/releases/download/canal-beta/latest.json",
                STABLE_ENDPOINT
            ]
        );
        for u in endpoints(Channel::Beta) {
            assert!(Url::parse(u).is_ok());
        }
    }
}
