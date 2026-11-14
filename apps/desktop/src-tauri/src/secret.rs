//! Session pathofexile.com (`POESESSID`) du joueur, pour lire ses coffres de PoE 1.
//!
//! Décision de Max (14/11/2026) : voie non officielle, pour un usage personnel.
//! Règles :
//! - la valeur est gardée dans le **Gestionnaire d'identification Windows**
//!   (chiffrée par Windows pour la session de l'utilisateur), jamais dans un
//!   fichier d'Augur, jamais dans les journaux ni le diagnostic ;
//! - elle n'est envoyée qu'à `www.pathofexile.com` (voir `poe1_stash.rs`) ;
//! - « Oublier ma session » l'efface.
//!
//! Hors Windows (développement), elle ne vit qu'en mémoire.

/// Nom de l'entrée dans le Gestionnaire d'identification.
#[cfg_attr(not(windows), allow(dead_code))]
const CIBLE: &str = "Augur/PoE1/POESESSID";

/// Une valeur de session plausible : 32 caractères hexadécimaux.
pub fn valide(sid: &str) -> bool {
    sid.len() == 32 && sid.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Security::Credentials::{
        CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW,
        CredFree, CredReadW, CredWriteW,
    };
    use windows::core::{HSTRING, PWSTR};

    pub fn lire() -> Option<String> {
        let cible = HSTRING::from(super::CIBLE);
        let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY : CredReadW alloue la structure, libérée par CredFree avant de
        // sortir ; le blob est copié avant la libération.
        unsafe {
            CredReadW(&cible, CRED_TYPE_GENERIC, None, &mut cred).ok()?;
            let c = &*cred;
            let octets =
                std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize)
                    .to_vec();
            CredFree(cred as *const _);
            String::from_utf8(octets).ok()
        }
    }

    pub fn ecrire(sid: &str) -> Result<(), String> {
        let mut cible: Vec<u16> = super::CIBLE.encode_utf16().chain(Some(0)).collect();
        let mut blob = sid.as_bytes().to_vec();
        let cred = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(cible.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // SAFETY : la structure et ses tampons vivent jusqu'au retour de l'appel.
        unsafe { CredWriteW(&cred, 0) }
            .map_err(|e| format!("Session non enregistrée dans Windows ({e})."))
    }

    pub fn effacer() {
        let cible = HSTRING::from(super::CIBLE);
        // SAFETY : simple suppression par nom ; une entrée absente n'est pas une erreur.
        let _ = unsafe { CredDeleteW(&cible, CRED_TYPE_GENERIC, None) };
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::Mutex;

    static MEMOIRE: Mutex<Option<String>> = Mutex::new(None);

    pub fn lire() -> Option<String> {
        MEMOIRE.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn ecrire(sid: &str) -> Result<(), String> {
        *MEMOIRE.lock().unwrap_or_else(|e| e.into_inner()) = Some(sid.to_string());
        Ok(())
    }

    pub fn effacer() {
        *MEMOIRE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

pub use imp::{ecrire, effacer, lire};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnait_une_valeur_de_session() {
        assert!(valide("0123456789abcdef0123456789ABCDEF"));
        assert!(!valide("trop-court"));
        assert!(!valide("0123456789abcdef0123456789abcdeg"));
    }
}
