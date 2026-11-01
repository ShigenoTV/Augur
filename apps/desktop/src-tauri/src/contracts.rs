//! Types du contrat application ↔ interface : source de vérité dans
//! `crates/augur-contracts` (génère aussi `@augur/contracts` pour le front).

pub use augur_contracts::*;

/// Construit une erreur de commande sans délai de réessai.
pub fn app_error(code: ErrorCode, message: impl Into<String>) -> AppError {
    AppError {
        code,
        message: message.into(),
        retry_after_ms: None,
    }
}
