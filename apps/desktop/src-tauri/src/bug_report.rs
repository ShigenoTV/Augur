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
/// Adresse gardée courte (Windows et certains navigateurs gèrent mal les
/// adresses de plus de 2 048 caractères) ; le rapport complet part dans le
/// presse-papiers.
const MAX_URL_LEN: usize = 1900;
const TRONQUE: &str = "\n… (tronqué : le rapport complet est dans le presse-papiers, colle-le ici)";

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
    /// Rapport complet copié dans le presse-papiers.
    pub copied: bool,
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
        if garde < 20 {
            return Err(app_error(
                ErrorCode::Internal,
                "Texte trop long : raccourcis-le et colle le reste dans le ticket.",
            ));
        }
        *plus_long = plus_long.chars().take(garde).collect::<String>() + TRONQUE;
        truncated = true;
    }
}

/// Rapport complet au format d'un ticket créé avec le modèle (mêmes intitulés
/// de sections), à coller dans le ticket si un champ est vide ou raccourci.
pub fn rapport_complet(report: &BugReport) -> String {
    let mut s = format!("[Bug] {}\n", report.titre.trim());
    let mut section = |label: &str, valeur: &str, code: bool| {
        let v = valeur.trim();
        if v.is_empty() {
            return;
        }
        s.push_str(&format!("\n### {label}\n\n"));
        if code {
            s.push_str(&format!("```text\n{v}\n```\n"));
        } else {
            s.push_str(v);
            s.push('\n');
        }
    };
    section("Où est le problème ?", &report.module, false);
    section("Ce que tu as fait", &report.etapes, false);
    section(
        "Ce que tu attendais, et ce qui s'est passé",
        &report.attendu,
        false,
    );
    section(
        "Texte de l'objet (si un objet est en cause)",
        &report.objet,
        true,
    );
    section("Diagnostic d'Augur", &report.diagnostic, true);
    s
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
        // Le modèle reste demandé même quand l'adresse est raccourcie.
        assert_eq!(q["template"], "bug-beta.yml");
        // Les champs courts restent entiers.
        assert_eq!(q["etapes"], "1. Ctrl+C sur un anneau\n2. rien");
    }

    /// Cas de Max (v0.1.32) : diagnostic de 15 lignes de journal, accents
    /// compris : l'adresse dépassait 2 048 caractères.
    #[test]
    fn diagnostic_complet_sous_la_limite_de_windows() {
        let journal = (0..15)
            .map(|i| format!("2026-11-19T08:13:{i:02}Z info suivi de d:\\steam\\steamapps\\common\\Path of Exile\\logs\\Client.txt (arbre 3.30, données ééé)"))
            .collect::<Vec<_>>()
            .join("\n");
        let r = BugReport {
            titre: "test2".into(),
            module: "Forge".into(),
            etapes: "test forge poe1".into(),
            attendu: "juste un test".into(),
            objet: "test3".into(),
            diagnostic: format!(
                "Augur : v0.1.32\nDonnées du jeu : 0.5.5.10\nPersonnage importé : Gemling Legionnaire niveau 95\n\nJournal (dernières lignes) :\n{journal}"
            ),
        };
        let (url, truncated) = issue_url(&r).unwrap();
        assert!(truncated);
        assert!(url.len() < 2048, "{} caractères", url.len());
        let q: std::collections::HashMap<_, _> = reqwest::Url::parse(&url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
        // Les champs saisis restent entiers, seul le diagnostic est raccourci.
        assert_eq!(q["template"], "bug-beta.yml");
        assert_eq!(q["etapes"], "test forge poe1");
        assert_eq!(q["attendu"], "juste un test");
        assert_eq!(q["objet"], "test3");
        assert!(q["diagnostic"].starts_with("Augur : v0.1.32"));
        assert!(q["diagnostic"].ends_with(TRONQUE));
    }

    #[test]
    fn rapport_complet_avec_les_sections_du_modele() {
        let r = BugReport {
            objet: "Rarity: Rare".into(),
            ..report()
        };
        let texte = rapport_complet(&r);
        assert!(texte.starts_with("[Bug] Overlay vide\n"));
        assert!(texte.contains("### Ce que tu as fait\n\n1. Ctrl+C sur un anneau\n2. rien\n"));
        assert!(texte.contains("### Ce que tu attendais, et ce qui s'est passé\n"));
        assert!(texte.contains(
            "### Texte de l'objet (si un objet est en cause)\n\n```text\nRarity: Rare\n```"
        ));
        assert!(texte.contains("### Diagnostic d'Augur\n\n```text\nAugur : v0.1.16\n```"));
        // Champ vide : pas de section.
        let sans_objet = rapport_complet(&report());
        assert!(!sans_objet.contains("Texte de l'objet"));
    }

    #[test]
    fn version_de_windows() {
        assert_eq!(nom_windows(19045), "Windows 10 (build 19045)");
        assert_eq!(nom_windows(26100), "Windows 11 (build 26100)");
    }
}
