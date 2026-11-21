//! Cote des uniques et des rares de PoE 2 par le site de trade, sur le
//! raccourci du joueur (Alt+P par défaut).
//!
//! Décision de Max du 18/11/2026 : comme Awakened PoE Trade et Exiled
//! Exchange 2, la recherche part du PC du joueur, une fois par appui, et suit
//! les limites que GGG renvoie (`X-Rate-Limit-*`, via `augur-ratelimit`). Au
//! premier refus (429), Augur s'arrête et affiche le temps d'attente donné par
//! GGG. Une cote = deux requêtes : `search` puis un `fetch` des
//! [`cote::ANNONCES_MAX`] annonces les moins chères.

use std::time::Duration;

use augur_ratelimit::{BudgetManager, LimiterConfig, Priority, parse_policy_from_pairs};
use poe2_core::cote;
use poe2_core::parser;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::contracts::{AppError, ErrorCode, ParsedItem, PriceEstimate, PriceSource, app_error};
use crate::game::Game;

const API: &str = "https://www.pathofexile.com/api/trade2";
const SITE: &str = "https://www.pathofexile.com/trade2/search/poe2";

/// Attente maximale d'un créneau libre : au-delà, on rend la main au joueur.
const ATTENTE_MAX: Duration = Duration::from_secs(8);

/// Budget des requêtes au site de trade, partagé par toute l'appli.
pub struct TradeBudget(BudgetManager);

impl Default for TradeBudget {
    fn default() -> Self {
        Self(BudgetManager::new(LimiterConfig::default()))
    }
}

/// Résultat d'une cote trade, pour l'overlay.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradePriceCheck {
    /// Absent quand aucune annonce comparable n'est en ligne.
    pub price: Option<PriceEstimate>,
    /// Annonces en ligne trouvées par la recherche.
    pub total: u32,
    /// Adresse des résultats sur le site de trade (« voir les annonces »).
    pub url: String,
    /// Mods que le site de trade ne sait pas chercher (recherche plus large).
    pub ignored: Vec<String>,
}

fn erreur(code: ErrorCode, message: impl Into<String>) -> AppError {
    app_error(code, message.into())
}

fn client(version: &str) -> Result<reqwest::Client, AppError> {
    // Même fournisseur de chiffrement que le client poe.ninja.
    let _ = crate::forge_prices::client(version)?;
    reqwest::Client::builder()
        .user_agent(format!(
            "Augur/{version} (outil personnel PoE 2 ; cote à la demande du joueur)"
        ))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| {
            erreur(
                ErrorCode::Network,
                format!("Client HTTP indisponible ({e})."),
            )
        })
}

/// Envoie une requête au site de trade sous le contrôle du budget.
async fn envoyer(
    budget: &BudgetManager,
    point: &str,
    requete: reqwest::RequestBuilder,
) -> Result<Value, AppError> {
    let permis = tokio::time::timeout(ATTENTE_MAX, budget.acquire(point, Priority::Interactive))
        .await
        .map_err(|_| {
            erreur(
                ErrorCode::RateLimited,
                "Limite du site de trade : réessaie dans quelques secondes.",
            )
        })?;
    let resp = match requete.send().await {
        Ok(r) => r,
        Err(e) => {
            drop(permis);
            return Err(erreur(
                ErrorCode::Network,
                format!("Site de trade injoignable ({e})."),
            ));
        }
    };
    let status = resp.status().as_u16();
    let paires: Vec<(String, String)> = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
        .collect();
    let politique = parse_policy_from_pairs(paires.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .ok()
        .flatten();
    permis.complete(politique.as_ref(), status);
    match status {
        200 => {}
        429 => {
            let secondes = paires
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
                .and_then(|(_, v)| v.trim().parse::<u32>().ok())
                .unwrap_or(60);
            let mut e = erreur(
                ErrorCode::RateLimited,
                format!("Le site de trade demande une pause : réessaie dans {secondes} s."),
            );
            e.retry_after_ms = Some(secondes.saturating_mul(1000));
            return Err(e);
        }
        400 => {
            return Err(erreur(
                ErrorCode::Unsupported,
                "Le site de trade n'a pas compris la recherche de cet objet.",
            ));
        }
        401 | 403 => {
            return Err(erreur(
                ErrorCode::Network,
                format!("Le site de trade a refusé la requête ({status})."),
            ));
        }
        _ => {
            return Err(erreur(
                ErrorCode::Network,
                format!("Le site de trade a répondu {status}."),
            ));
        }
    }
    let texte = resp
        .text()
        .await
        .map_err(|e| erreur(ErrorCode::Network, format!("Réponse illisible ({e}).")))?;
    serde_json::from_str(&texte)
        .map_err(|_| erreur(ErrorCode::Network, "Réponse inattendue du site de trade."))
}

/// Cote trade de l'objet (ligue PoE 2 des Réglages).
pub async fn coter(app: &AppHandle, item: &ParsedItem) -> Result<TradePriceCheck, AppError> {
    let objet = parser::parser(&item.raw_text)
        .map_err(|e| erreur(ErrorCode::ParseFailed, e.to_string()))?;
    let recherche = cote::recherche_trade(&objet).ok_or_else(|| {
        erreur(
            ErrorCode::Unsupported,
            "Le site de trade ne sait pas comparer cet objet (unique ou rare identifié seulement).",
        )
    })?;
    let ligue = app
        .state::<crate::settings::SettingsStore>()
        .get()
        .league_for(Game::Poe2)
        .to_string();
    let version = app.package_info().version.to_string();
    let http = client(&version)?;
    let budget = &app.state::<TradeBudget>().0;
    let enc = crate::forge_prices::encode;

    let recu = envoyer(
        budget,
        "trade2-search",
        http.post(format!("{API}/search/poe2/{}", enc(&ligue)))
            .header("Content-Type", "application/json")
            .body(recherche.requete.to_string()),
    )
    .await?;
    let id = recu["id"]
        .as_str()
        .ok_or_else(|| erreur(ErrorCode::Network, "Réponse de recherche sans identifiant."))?
        .to_string();
    let url = format!("{SITE}/{}/{}", enc(&ligue), enc(&id));
    let total = recu["total"].as_u64().unwrap_or(0).min(u64::from(u32::MAX)) as u32;
    let annonces: Vec<&str> = recu["result"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(cote::ANNONCES_MAX)
        .collect();
    let mut check = TradePriceCheck {
        price: None,
        total,
        url,
        ignored: recherche.ignorees,
    };
    if annonces.is_empty() {
        return Ok(check);
    }
    let lues = envoyer(
        budget,
        "trade2-fetch",
        http.get(format!(
            "{API}/fetch/{}?query={}&realm=poe2",
            annonces.join(","),
            enc(&id)
        )),
    )
    .await?;

    // Cours des devises : page poe.ninja des monnaies (gardée 30 min).
    let devises = crate::poe2_price::page_devises(app).await?;
    let taux = cote::taux_devises(&devises);
    let prix = cote::prix_annonces(&lues, &taux);
    let divine = taux.get("divine").copied();
    check.price = cote::fourchette(&prix).map(|(bas, mediane, haut)| {
        let (devise, t) = match divine {
            Some(d) if mediane >= d => ("divine", d),
            _ => ("exalted", 1.0),
        };
        PriceEstimate {
            currency: devise.into(),
            low: cote::arrondi(bas / t),
            median: cote::arrondi(mediane / t),
            high: cote::arrondi(haut / t),
            sample_size: prix.len() as u32,
            source: PriceSource::Trade,
            league: ligue.clone(),
            observed_at: crate::time::rfc3339_now(),
            stale: false,
            quantity: None,
            unit_price: None,
        }
    });
    Ok(check)
}

/// Ouvre les résultats d'une cote dans le navigateur (adresse du site de trade seulement).
pub fn ouvrir(url: &str) -> Result<(), AppError> {
    if !url.starts_with(SITE) {
        return Err(erreur(ErrorCode::Internal, "Adresse refusée."));
    }
    crate::upgrade_search::ouvrir(url)
}
