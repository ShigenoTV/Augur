//! « Signaler un bug » : ouvre dans le navigateur la page de création d'un
//! ticket GitHub, préremplie avec le modèle `.github/ISSUE_TEMPLATE/bug-beta.yml`.
//!
//! Aucun jeton n'est embarqué et rien n'est envoyé par Augur : le joueur relit
//! le ticket sur GitHub et le crée lui-même. L'adresse du dépôt est fixe ici,
//! la WebView ne fournit que le texte des champs. Le diagnostic ne contient
//! aucune donnée personnelle (ni chemin, ni nom de personnage).

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::contracts::{AppError, ErrorCode, app_error};

/// Page « nouveau ticket » du dépôt, avec le modèle de bug (étiquettes `bug` et `bêta`).
const NEW_ISSUE_URL: &str = "https://github.com/ShigenoTV/Augur/issues/new";
const TEMPLATE: &str = "bug-beta.yml";
/// GitHub refuse les adresses trop longues (≈ 8 Ko) : on garde de la marge.
const MAX_URL_LEN: usize = 7000;
const TRONQUE: &str = "\n… (tronqué : colle la suite ici)";

/// Champs saisis dans la fenêtre « Signaler un bug ».
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BugReport {
    pub titre: String,
    /// Option du menu « Où est le problème ? » du modèle.
    pub module: String,
    pub etapes: String,
    pub attendu: String,
    #[serde(default)]
    pub objet: String,
    /// Diagnostic tel qu'affiché au joueur (vide s'il l'a décoché).
    #[serde(default)]
    pub diagnostic: String,
}

/// Adresse ouverte et indication de troncature (texte trop long pour l'adresse).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BugReportOpened {
    pub truncated: bool,
}

/// Diagnostic technique joint au ticket, sans donnée personnelle.
pub fn diagnostic(app: &AppHandle) -> String {
    let version = app.package_info().version.to_string();
    let donnees = poe2_core::forge::atelier::donnees()
        .ok()
        .map(|d| d.repoe_version.clone())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "indisponibles".into());
    let log = app.state::<crate::client_log::ClientLogState>().get();
    let journal = match (log.path.is_some(), log.watching) {
        (true, true) => "trouvé, suivi actif",
        (true, false) => "trouvé, suivi arrêté",
        (false, _) => "introuvable",
    };
    let settings = app.state::<crate::settings::SettingsStore>().get();
    let perso = app
        .state::<crate::character::CharacterState>()
        .summary()
        .map(|c| format!("{} niveau {}", c.class, c.level))
        .unwrap_or_else(|| "aucun".into());
    let jeu = if crate::game_window::game_is_foreground() {
        "au premier plan"
    } else {
        "non détecté"
    };
    [
        format!("Augur : v{version}"),
        format!("Données du jeu : {donnees}"),
        format!("Contrat : {}", crate::contracts::CONTRACT_VERSION),
        format!("Système : {}", system::version()),
        format!("Client.txt : {journal}"),
        format!("Jeu : {} ({jeu})", settings.game.label()),
        format!("Ligue souhaitée : {}", settings.league_for(settings.game)),
        format!("Personnage importé : {perso}"),
    ]
    .join("\n")
}

/// Construit l'adresse du ticket prérempli. Les champs les plus longs sont
/// raccourcis si l'adresse dépasse la limite de GitHub.
pub fn issue_url(report: &BugReport) -> Result<(String, bool), AppError> {
    let titre = report.titre.trim();
    if titre.is_empty() {
        return Err(app_error(ErrorCode::Internal, "Donne un titre au bug."));
    }
    let mut champs: Vec<(&str, String)> = vec![
        ("etapes", report.etapes.trim().to_string()),
        ("attendu", report.attendu.trim().to_string()),
        ("objet", report.objet.trim().to_string()),
        ("diagnostic", report.diagnostic.trim().to_string()),
    ];
    let mut truncated = false;
    loop {
        let url = build(titre, report.module.trim(), &champs)?;
        if url.len() <= MAX_URL_LEN {
            return Ok((url, truncated));
        }
        // Raccourcit le champ le plus long de 25 %.
        let (_, plus_long) = champs
            .iter_mut()
            .max_by_key(|(_, v)| v.chars().count())
            .expect("champs non vides");
        let garde = plus_long.trim_end_matches(TRONQUE).chars().count() * 3 / 4;
        if garde < 50 {
            return Err(app_error(
                ErrorCode::Internal,
                "Texte trop long : raccourcis-le et colle le reste dans le ticket.",
            ));
        }
        *plus_long = plus_long.chars().take(garde).collect::<String>() + TRONQUE;
        truncated = true;
    }
}

fn build(titre: &str, module: &str, champs: &[(&str, String)]) -> Result<String, AppError> {
    let titre = format!("[Bug] {titre}");
    let mut params: Vec<(&str, &str)> = vec![("template", TEMPLATE), ("title", &titre)];
    if !module.is_empty() {
        params.push(("module", module));
    }
    params.extend(
        champs
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| (*k, v.as_str())),
    );
    reqwest::Url::parse_with_params(NEW_ISSUE_URL, &params)
        .map(String::from)
        .map_err(|e| app_error(ErrorCode::Internal, format!("Adresse invalide ({e}).")))
}

/// Ouvre l'adresse dans le navigateur par défaut.
pub fn open_in_browser(url: &str) -> Result<(), AppError> {
    // L'adresse est toujours celle du dépôt (construite ci-dessus).
    debug_assert!(url.starts_with(NEW_ISSUE_URL));
    #[cfg(windows)]
    let mut cmd = {
        let mut c = std::process::Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.spawn().map(|_| ()).map_err(|e| {
        app_error(
            ErrorCode::Internal,
            format!(
                "Navigateur non ouvert ({e}). Signale le bug sur github.com/ShigenoTV/Augur/issues."
            ),
        )
    })
}

#[cfg(windows)]
mod system {
    use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::{PCWSTR, w};

    fn read(value: PCWSTR) -> Option<String> {
        let mut buf = [0u16; 128];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY : lecture d'une valeur texte dans un tampon local dont la
        // taille en octets est passée à l'API.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
                value,
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if status.is_err() {
            return None;
        }
        let len = (size as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    /// « Windows 11 (build 26100) » : la clé ProductName dit encore « Windows 10 » sur 11.
    pub fn version() -> String {
        match read(w!("CurrentBuildNumber")).and_then(|b| b.trim().parse::<u32>().ok()) {
            Some(build) => super::nom_windows(build),
            None => "Windows (version inconnue)".into(),
        }
    }
}

#[cfg(not(windows))]
mod system {
    pub fn version() -> String {
        std::env::consts::OS.into()
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn nom_windows(build: u32) -> String {
    let nom = if build >= 22000 {
        "Windows 11"
    } else {
        "Windows 10"
    };
    format!("{nom} (build {build})")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> BugReport {
        BugReport {
            titre: "Overlay vide".into(),
            module: "Overlay (Ctrl+C sur un objet)".into(),
            etapes: "1. Ctrl+C sur un anneau\n2. rien".into(),
            attendu: "L'overlay devait s'ouvrir & afficher le DPS".into(),
            objet: String::new(),
            diagnostic: "Augur : v0.1.16".into(),
        }
    }

    #[test]
    fn adresse_preremplie_sur_le_depot() {
        let (url, truncated) = issue_url(&report()).unwrap();
        assert!(!truncated);
        let parsed = reqwest::Url::parse(&url).unwrap();
        assert_eq!(parsed.host_str(), Some("github.com"));
        assert_eq!(parsed.path(), "/ShigenoTV/Augur/issues/new");
        let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(q["template"], "bug-beta.yml");
        assert_eq!(q["title"], "[Bug] Overlay vide");
        assert_eq!(q["module"], "Overlay (Ctrl+C sur un objet)");
        assert_eq!(q["attendu"], "L'overlay devait s'ouvrir & afficher le DPS");
        assert_eq!(q["diagnostic"], "Augur : v0.1.16");
        // Champ vide : absent de l'adresse.
        assert!(!q.contains_key("objet"));
    }

    #[test]
    fn titre_obligatoire() {
        let r = BugReport {
            titre: "  ".into(),
            ..report()
        };
        assert!(issue_url(&r).is_err());
    }

    #[test]
    fn texte_trop_long_tronque() {
        let r = BugReport {
            objet: "Rarity: Rare\n".repeat(2000),
            ..report()
        };
        let (url, truncated) = issue_url(&r).unwrap();
        assert!(truncated);
        assert!(url.len() <= MAX_URL_LEN);
        let parsed = reqwest::Url::parse(&url).unwrap();
        let q: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert!(q["objet"].ends_with(TRONQUE));
        // Les champs courts restent entiers.
        assert_eq!(q["etapes"], "1. Ctrl+C sur un anneau\n2. rien");
    }

    #[test]
    fn version_de_windows() {
        assert_eq!(nom_windows(19045), "Windows 10 (build 19045)");
        assert_eq!(nom_windows(26100), "Windows 11 (build 26100)");
    }
}
