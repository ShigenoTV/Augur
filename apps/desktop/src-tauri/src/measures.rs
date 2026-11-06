//! Session de mesures : Augur garde tout ce qu'il lit pendant que le joueur
//! fait ses relevés en jeu (objets copiés, codes Path of Building, zones de
//! Client.txt, notes libres), puis l'exporte en un fichier à envoyer.
//!
//! Rien ne quitte le PC : l'export est écrit dans `Documents\Augur\mesures`
//! et copié dans le presse-papier seulement quand le joueur le demande.
//! Les lignes de chat de Client.txt ne sont jamais gardées.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::contracts::ParsedItem;
use crate::time::rfc3339_now;

/// Événement émis à chaque changement de la session (charge : `MeasureStatus`).
pub const EVENT_STATUS: &str = "augur://measure-session";
/// Version du format de fichier exporté.
pub const FORMAT: &str = "augur-mesures/1";

/// Façon dont l'objet a été copié en jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CopyKind {
    /// Ctrl+C.
    Simple,
    /// Ctrl+Alt+C : tiers et affixes visibles, valeurs AVANT la qualité.
    Advanced,
}

/// Un relevé de la session.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Entry {
    #[serde(rename_all = "camelCase")]
    Item {
        at: String,
        copy: CopyKind,
        /// Texte exact du presse-papier.
        raw_text: String,
        /// Ce que le parseur en a compris (absent si refusé).
        parsed: Option<ParsedItem>,
        /// Raison du refus du parseur.
        error: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    PobCode { at: String, code: String },
    #[serde(rename_all = "camelCase")]
    Zone {
        at: String,
        zone: String,
        /// Niveau de zone (niveau des monstres) lu juste avant l'entrée.
        level: Option<u32>,
        area_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Note { at: String, text: String },
}

/// Une session en cours ou terminée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub format: &'static str,
    pub augur_version: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub client_log: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub items: u32,
    pub pob_codes: u32,
    pub zones: u32,
    pub notes: u32,
}

impl Session {
    pub fn new(augur_version: String, started_at: String) -> Self {
        Self {
            format: FORMAT,
            augur_version,
            started_at,
            ended_at: None,
            client_log: None,
            entries: Vec::new(),
        }
    }

    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for e in &self.entries {
            match e {
                Entry::Item { .. } => c.items += 1,
                Entry::PobCode { .. } => c.pob_codes += 1,
                Entry::Zone { .. } => c.zones += 1,
                Entry::Note { .. } => c.notes += 1,
            }
        }
        c
    }

    /// Ajoute un relevé, sauf s'il répète exactement le précédent du même
    /// genre (même objet copié deux fois de suite, même zone rechargée).
    pub fn push(&mut self, entry: Entry) {
        let duplicate = self
            .entries
            .iter()
            .rev()
            .find(|e| same_kind(e, &entry))
            .is_some_and(|prev| match (prev, &entry) {
                (Entry::Item { raw_text: a, .. }, Entry::Item { raw_text: b, .. }) => a == b,
                (Entry::PobCode { code: a, .. }, Entry::PobCode { code: b, .. }) => a == b,
                _ => false,
            });
        if !duplicate {
            self.entries.push(entry);
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Résumé lisible, rangé à côté du JSON.
    pub fn to_markdown(&self) -> String {
        let c = self.counts();
        let mut out = String::new();
        out.push_str("# Session de mesures Augur\n\n");
        out.push_str(&format!(
            "- Augur {}\n- Début : {}\n- Fin : {}\n- Relevés : {} objet(s), {} code(s) PoB, {} zone(s), {} note(s)\n",
            self.augur_version,
            self.started_at,
            self.ended_at.as_deref().unwrap_or("en cours"),
            c.items,
            c.pob_codes,
            c.zones,
            c.notes
        ));
        if let Some(log) = &self.client_log {
            out.push_str(&format!("- Client.txt : {log}\n"));
        }
        out.push_str(
            "\nRappel : en Ctrl+Alt+C, le jeu affiche les valeurs des mods avant la qualité.\n",
        );
        for e in &self.entries {
            out.push('\n');
            match e {
                Entry::Item {
                    at,
                    copy,
                    raw_text,
                    parsed,
                    error,
                } => {
                    let name = parsed
                        .as_ref()
                        .map(|p| p.name.clone().unwrap_or_else(|| p.base_type.clone()))
                        .unwrap_or_else(|| "objet non reconnu".into());
                    let how = match copy {
                        CopyKind::Simple => "Ctrl+C",
                        CopyKind::Advanced => "Ctrl+Alt+C",
                    };
                    out.push_str(&format!("## {at} · Objet ({how}) : {name}\n\n"));
                    if let Some(err) = error {
                        out.push_str(&format!("Refusé par le parseur : {err}\n\n"));
                    }
                    out.push_str("```\n");
                    out.push_str(raw_text.trim_end());
                    out.push_str("\n```\n");
                }
                Entry::PobCode { at, code } => {
                    out.push_str(&format!(
                        "## {at} · Code Path of Building ({} caractères)\n\n```\n{code}\n```\n",
                        code.len()
                    ));
                }
                Entry::Zone {
                    at,
                    zone,
                    level,
                    area_id,
                } => {
                    let level = level.map(|l| format!(", niveau {l}")).unwrap_or_default();
                    let id = area_id
                        .as_ref()
                        .map(|a| format!(" ({a})"))
                        .unwrap_or_default();
                    out.push_str(&format!("## {at} · Zone : {zone}{level}{id}\n"));
                }
                Entry::Note { at, text } => {
                    out.push_str(&format!("## {at} · Note\n\n{text}\n"));
                }
            }
        }
        out
    }
}

fn same_kind(a: &Entry, b: &Entry) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Ctrl+Alt+C ajoute des lignes d'en-tête de mod entre accolades.
pub fn copy_kind(raw_text: &str) -> CopyKind {
    let advanced = raw_text.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("{ ") && l.ends_with('}')
    });
    if advanced {
        CopyKind::Advanced
    } else {
        CopyKind::Simple
    }
}

/// Reconnaît un code d'export Path of Building (base64 compressé, sans
/// espace) ou un lien pobb.in / pastebin.
pub fn looks_like_pob_code(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.contains(char::is_whitespace) {
        return false;
    }
    if t.starts_with("https://pobb.in/") || t.starts_with("https://pastebin.com/") {
        return true;
    }
    t.len() >= 200
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'-' | b'_'))
}

/// Nom de fichier tiré de l'horodatage RFC 3339 (`2026-11-05T15:30:12Z`).
pub fn file_stem(started_at: &str) -> String {
    let compact: String = started_at
        .trim_end_matches('Z')
        .chars()
        .map(|c| {
            if c == ':' {
                '-'
            } else if c == 'T' {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("augur-mesures-{compact}")
}

/// Fichiers écrits à l'arrêt de la session.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportInfo {
    pub json_path: String,
    pub markdown_path: String,
    pub counts: Counts,
}

/// Écrit `<stem>.json` et `<stem>.md` dans `dir`.
pub fn write_export(session: &Session, dir: &Path) -> std::io::Result<ExportInfo> {
    std::fs::create_dir_all(dir)?;
    let stem = file_stem(&session.started_at);
    let json = dir.join(format!("{stem}.json"));
    let md = dir.join(format!("{stem}.md"));
    std::fs::write(&json, session.to_json())?;
    std::fs::write(&md, session.to_markdown())?;
    Ok(ExportInfo {
        json_path: json.display().to_string(),
        markdown_path: md.display().to_string(),
        counts: session.counts(),
    })
}

/// État visible par l'interface.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasureStatus {
    pub active: bool,
    pub started_at: Option<String>,
    pub counts: Counts,
    pub last_export: Option<ExportInfo>,
}

#[derive(Default)]
struct Inner {
    session: Option<Session>,
    /// Dernière session terminée, gardée pour la copie dans le presse-papier.
    finished: Option<Session>,
    last_export: Option<ExportInfo>,
    /// Dernier « Generating level N area X » lu, rattaché à l'entrée suivante.
    pending_area: Option<(u32, String)>,
}

#[derive(Default)]
pub struct MeasureState(Mutex<Inner>);

impl MeasureState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_active(&self) -> bool {
        self.lock().session.is_some()
    }

    pub fn status(&self) -> MeasureStatus {
        let g = self.lock();
        MeasureStatus {
            active: g.session.is_some(),
            started_at: g.session.as_ref().map(|s| s.started_at.clone()),
            counts: g.session.as_ref().map(Session::counts).unwrap_or_default(),
            last_export: g.last_export.clone(),
        }
    }

    pub fn finished_json(&self) -> Option<String> {
        self.lock().finished.as_ref().map(Session::to_json)
    }

    /// Ajoute un relevé si une session est en cours ; vrai si ajouté.
    fn record(&self, entry: Entry) -> bool {
        let mut g = self.lock();
        match g.session.as_mut() {
            Some(s) => {
                s.push(entry);
                true
            }
            None => false,
        }
    }
}

fn emit_status(app: &AppHandle) {
    let status = app.state::<MeasureState>().status();
    crate::tray::set_measure_active(app, status.active);
    let _ = app.emit(EVENT_STATUS, status);
}

fn record(app: &AppHandle, entry: Entry) {
    if app.state::<MeasureState>().record(entry) {
        emit_status(app);
    }
}

pub fn start(app: &AppHandle) -> MeasureStatus {
    {
        let state = app.state::<MeasureState>();
        let mut g = state.lock();
        if g.session.is_none() {
            let mut s = Session::new(app.package_info().version.to_string(), rfc3339_now());
            s.client_log = app.state::<crate::client_log::ClientLogState>().get().path;
            g.session = Some(s);
            g.pending_area = None;
        }
    }
    emit_status(app);
    app.state::<MeasureState>().status()
}

fn export_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .document_dir()
        .or_else(|_| app.path().app_data_dir())
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("Augur")
        .join("mesures")
}

/// Arrête la session et écrit l'export. Sans session en cours, ne fait rien.
pub fn stop(app: &AppHandle) -> std::io::Result<MeasureStatus> {
    let session = {
        let state = app.state::<MeasureState>();
        let mut g = state.lock();
        g.session.take()
    };
    if let Some(mut session) = session {
        session.ended_at = Some(rfc3339_now());
        let export = write_export(&session, &export_dir(app));
        let state = app.state::<MeasureState>();
        {
            let mut g = state.lock();
            g.finished = Some(session);
            if let Ok(info) = &export {
                g.last_export = Some(info.clone());
            }
        }
        emit_status(app);
        export?;
    }
    Ok(app.state::<MeasureState>().status())
}

pub fn toggle(app: &AppHandle) {
    if app.state::<MeasureState>().is_active() {
        if let Err(err) = stop(app) {
            log::error!("export des mesures impossible : {err}");
        }
        crate::show_main_window(app);
    } else {
        start(app);
    }
}

pub fn add_note(app: &AppHandle, text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    let state = app.state::<MeasureState>();
    let added = state.record(Entry::Note {
        at: rfc3339_now(),
        text: text.to_owned(),
    });
    if added {
        emit_status(app);
    }
    added
}

/// Objet copié en jeu (lu ou refusé par le parseur).
pub fn record_item(app: &AppHandle, raw_text: &str, result: &Result<ParsedItem, String>) {
    let (parsed, error) = match result {
        Ok(item) => (Some(item.clone()), None),
        Err(e) => (None, Some(e.clone())),
    };
    record(
        app,
        Entry::Item {
            at: rfc3339_now(),
            copy: copy_kind(raw_text),
            raw_text: raw_text.to_owned(),
            parsed,
            error,
        },
    );
}

pub fn record_pob_code(app: &AppHandle, code: &str) {
    record(
        app,
        Entry::PobCode {
            at: rfc3339_now(),
            code: code.trim().to_owned(),
        },
    );
}

/// Génération de zone lue dans Client.txt : gardée pour l'entrée qui suit.
pub fn note_area_generated(app: &AppHandle, level: u32, area_id: String) {
    let state = app.state::<MeasureState>();
    let mut g = state.lock();
    if g.session.is_some() {
        g.pending_area = Some((level, area_id));
    }
}

pub fn record_zone(app: &AppHandle, zone: &str, at: String) {
    let pending = {
        let state = app.state::<MeasureState>();
        let mut g = state.lock();
        g.pending_area.take()
    };
    let (level, area_id) = match pending {
        Some((l, id)) => (Some(l), Some(id)),
        None => (None, None),
    };
    record(
        app,
        Entry::Zone {
            at,
            zone: zone.to_owned(),
            level,
            area_id,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = "Item Class: Rings\nRarity: Rare\nDire Loop\nRuby Ring\n--------\n+14% to Fire Resistance\n";
    const AVANCE: &str = "Item Class: Rings\nRarity: Rare\nDire Loop\nRuby Ring\n--------\n{ Suffix Modifier \"of the Salamander\" (Tier: 6) — Elemental, Fire, Resistance }\n+14(12-15)% to Fire Resistance\n";

    #[test]
    fn distingue_ctrl_c_et_ctrl_alt_c() {
        assert_eq!(copy_kind(SIMPLE), CopyKind::Simple);
        assert_eq!(copy_kind(AVANCE), CopyKind::Advanced);
    }

    #[test]
    fn reconnait_les_codes_pob() {
        let code = "eNrtfV1z2ziy6PP-".repeat(20);
        assert!(looks_like_pob_code(&code));
        assert!(looks_like_pob_code("https://pobb.in/AbCdEf123"));
        assert!(!looks_like_pob_code("court"));
        assert!(!looks_like_pob_code(SIMPLE));
        assert!(!looks_like_pob_code(&format!("{code} {code}")));
    }

    #[test]
    fn nomme_le_fichier_d_apres_le_debut() {
        assert_eq!(
            file_stem("2026-11-05T15:30:12Z"),
            "augur-mesures-2026-11-05_15-30-12"
        );
    }

    #[test]
    fn ignore_la_meme_copie_deux_fois_de_suite() {
        let mut s = Session::new("0.1.3".into(), "2026-11-05T15:30:12Z".into());
        let item = |t: &str| Entry::Item {
            at: "t".into(),
            copy: CopyKind::Simple,
            raw_text: t.into(),
            parsed: None,
            error: Some("x".into()),
        };
        s.push(item(SIMPLE));
        s.push(Entry::Zone {
            at: "t".into(),
            zone: "Plantation".into(),
            level: Some(66),
            area_id: None,
        });
        s.push(item(SIMPLE));
        s.push(item(AVANCE));
        assert_eq!(
            s.counts(),
            Counts {
                items: 2,
                pob_codes: 0,
                zones: 1,
                notes: 0
            }
        );
    }

    #[test]
    fn exporte_json_et_resume() {
        let mut s = Session::new("0.1.3".into(), "2026-11-05T15:30:12Z".into());
        s.push(Entry::Item {
            at: "2026-11-05T15:31:00Z".into(),
            copy: CopyKind::Advanced,
            raw_text: AVANCE.into(),
            parsed: None,
            error: Some("base inconnue".into()),
        });
        s.push(Entry::Zone {
            at: "2026-11-05T15:32:00Z".into(),
            zone: "Plantation".into(),
            level: Some(66),
            area_id: Some("MapPlantation".into()),
        });
        s.push(Entry::Note {
            at: "2026-11-05T15:33:00Z".into(),
            text: "DPS Arc : 12 345".into(),
        });
        s.ended_at = Some("2026-11-05T15:40:00Z".into());

        let dir = tempfile::tempdir().unwrap();
        let info = write_export(&s, dir.path()).unwrap();
        assert_eq!(
            info.counts,
            Counts {
                items: 1,
                pob_codes: 0,
                zones: 1,
                notes: 1
            }
        );

        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&info.json_path).unwrap()).unwrap();
        assert_eq!(json["format"], FORMAT);
        assert_eq!(json["entries"][0]["kind"], "item");
        assert_eq!(json["entries"][0]["copy"], "advanced");
        assert_eq!(json["entries"][0]["rawText"], AVANCE);
        assert_eq!(json["entries"][1]["level"], 66);
        assert_eq!(json["entries"][2]["text"], "DPS Arc : 12 345");

        let md = std::fs::read_to_string(&info.markdown_path).unwrap();
        assert!(md.contains("Objet (Ctrl+Alt+C) : objet non reconnu"));
        assert!(md.contains("Refusé par le parseur : base inconnue"));
        assert!(md.contains("Zone : Plantation, niveau 66 (MapPlantation)"));
        assert!(md.contains("DPS Arc : 12 345"));
    }
}
