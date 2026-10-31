//! Mise à jour de l'application depuis l'application.
//!
//! - Vérification au démarrage puis toutes les 6 heures, et à la demande.
//! - Le joueur décide : rien ne s'installe sans un clic sur « Installer et
//!   redémarrer », donc jamais en pleine partie à son insu.
//! - Les paquets sont signés (minisign) et vérifiés par le plugin updater
//!   avant installation ; un paquet mal signé est refusé.
//! - Canal stable seulement en 0.1 (mises à jour servies par GitHub
//!   Releases) ; le réglage « Rejoindre la bêta » existe mais reste masqué.
//!
//! Chaîne de publication : docs/devops/release-windows.md (chantier DevOps).

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::contracts::{AppError, ErrorCode};
use crate::settings::SettingsStore;
use crate::time::rfc3339_from_unix;

/// Manifeste de mise à jour du canal stable, servi par les releases GitHub
/// tant que le studio n'a pas de domaine (docs/devops/release-windows.md).
/// Seule constante à changer le jour où un domaine est choisi.
pub const STABLE_ENDPOINT: &str =
    "https://github.com/ShigenoTV/Augur/releases/latest/download/latest.json";

/// Le canal bêta n'existe pas tant que les mises à jour passent par GitHub
/// Releases : l'option est masquée dans l'interface et pointe sur le stable.
pub const BETA_AVAILABLE: bool = false;

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

pub fn endpoint(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => STABLE_ENDPOINT,
        // Pas de canal bêta servi pour l'instant : même manifeste que le stable.
        Channel::Beta => STABLE_ENDPOINT,
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
    AppError::new(
        ErrorCode::Network,
        format!("Impossible de vérifier les mises à jour pour le moment ({err})."),
    )
}

pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, AppError> {
    let channel =
        Channel::from_beta(BETA_AVAILABLE && app.state::<SettingsStore>().get().beta_channel);
    let url = Url::parse(endpoint(channel)).map_err(updater_error)?;
    let update = app
        .updater_builder()
        .endpoints(vec![url])
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
        .ok_or_else(|| AppError::new(ErrorCode::Internal, "Aucune mise à jour en attente."))?;

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
            AppError::new(
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
            endpoint(Channel::Stable),
            "https://github.com/ShigenoTV/Augur/releases/latest/download/latest.json"
        );
        // Tant que le canal bêta n'est pas servi, il retombe sur le stable.
        assert_eq!(endpoint(Channel::Beta), endpoint(Channel::Stable));
        assert!(Url::parse(STABLE_ENDPOINT).is_ok());
    }
}
