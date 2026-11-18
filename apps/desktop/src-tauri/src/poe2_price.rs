//! Prix estimé d'un objet de PoE 2 dans l'overlay (cote poe.ninja).
//!
//! Seuls les objets empilables sont cotés (monnaies, essences, Omens, runes,
//! gemmes brutes…) : poe.ninja PoE 2 ne cote ni les uniques ni les rares, et
//! Augur n'interroge pas le site de trade (décision de Max du 18/11/2026).
//! Une page poe.ninja est téléchargée à la demande et gardée 30 minutes :
//! relire plusieurs objets de la même famille ne refait aucune requête.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use poe2_core::cote;
use poe2_core::forge::ninja::API;
use poe2_core::parser::{self, Objet};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::contracts::{AppError, ErrorCode, ParsedItem, PriceEstimate, PriceSource, app_error};
use crate::game::Game;

/// Durée de vie d'une page poe.ninja en mémoire.
const DUREE_CACHE: Duration = Duration::from_secs(30 * 60);

/// Une page téléchargée : moment du relevé (pour l'âge affiché) et contenu.
type Page = (Instant, String, Value);

#[derive(Default)]
pub struct Poe2Ninja {
    pages: Mutex<HashMap<(String, &'static str), Page>>,
}

impl Poe2Ninja {
    fn en_cache(&self, ligue: &str, categorie: &'static str) -> Option<(String, Value)> {
        let pages = self.pages.lock().unwrap_or_else(|e| e.into_inner());
        pages
            .get(&(ligue.to_string(), categorie))
            .filter(|(at, _, _)| at.elapsed() < DUREE_CACHE)
            .map(|(_, date, v)| (date.clone(), v.clone()))
    }

    /// Page d'échange d'une catégorie, avec la date de son relevé (RFC 3339).
    async fn page(
        &self,
        client: &reqwest::Client,
        ligue: &str,
        categorie: &'static str,
    ) -> Result<(String, Value), AppError> {
        if let Some(p) = self.en_cache(ligue, categorie) {
            return Ok(p);
        }
        let url = format!(
            "{API}/exchange/current/overview?league={}&type={categorie}",
            crate::forge_prices::encode(ligue)
        );
        let doc = crate::forge_prices::get_json(client, &url).await?;
        let date = crate::time::rfc3339_now();
        let mut pages = self.pages.lock().unwrap_or_else(|e| e.into_inner());
        pages.retain(|_, (at, _, _)| at.elapsed() < DUREE_CACHE);
        pages.insert(
            (ligue.to_string(), categorie),
            (Instant::now(), date.clone(), doc.clone()),
        );
        Ok((date, doc))
    }
}

/// Page poe.ninja des monnaies de la ligue PoE 2 (cours des devises du trade).
pub async fn page_devises(app: &AppHandle) -> Result<Value, AppError> {
    let ligue = app
        .state::<crate::settings::SettingsStore>()
        .get()
        .league_for(Game::Poe2)
        .to_string();
    let client = crate::forge_prices::client(&app.package_info().version.to_string())?;
    let (_, doc) = app
        .state::<Poe2Ninja>()
        .page(&client, &ligue, "Currency")
        .await?;
    Ok(doc)
}

/// Prix en Exalted, ou en Divine dès qu'il atteint une Divine.
fn estimation(c: &cote::Cote, divine: Option<f64>, ligue: &str, date: String) -> PriceEstimate {
    let (devise, taux) = match divine {
        Some(d) if c.total >= d => ("divine", d),
        _ => ("exalted", 1.0),
    };
    let total = cote::arrondi(c.total / taux);
    let pile = c.quantite > 1;
    PriceEstimate {
        currency: devise.into(),
        low: total,
        median: total,
        high: total,
        sample_size: 0,
        source: PriceSource::CurrencyExchange,
        league: ligue.to_string(),
        observed_at: date,
        stale: false,
        quantity: pile.then_some(c.quantite),
        unit_price: pile.then(|| cote::arrondi(c.unite / taux)),
    }
}

fn non_cote(message: impl Into<String>) -> AppError {
    app_error(ErrorCode::Unsupported, message.into())
}

/// Cote poe.ninja de l'objet copié (ligue PoE 2 des Réglages).
pub async fn estimer(app: &AppHandle, item: &ParsedItem) -> Result<PriceEstimate, AppError> {
    let objet: Objet = parser::parser(&item.raw_text)
        .map_err(|e| app_error(ErrorCode::ParseFailed, e.to_string()))?;
    let pages = cote::pages(&objet);
    if pages.is_empty() {
        return Err(non_cote(
            "Pas de cote automatique pour ce genre d'objet : compare sur le site de trade.",
        ));
    }
    let ligue = app
        .state::<crate::settings::SettingsStore>()
        .get()
        .league_for(Game::Poe2)
        .to_string();
    let client = crate::forge_prices::client(&app.package_info().version.to_string())?;
    let ninja = app.state::<Poe2Ninja>();
    for categorie in pages {
        let (date, doc) = ninja.page(&client, &ligue, categorie).await?;
        let Some(c) = cote::coter(&doc, &objet) else {
            continue;
        };
        // Cours de la Divine : page des devises (souvent déjà en mémoire).
        let divine = match categorie {
            "Currency" => cote::exalts_par_divine(&doc),
            _ => ninja
                .page(&client, &ligue, "Currency")
                .await
                .ok()
                .and_then(|(_, d)| cote::exalts_par_divine(&d)),
        };
        return Ok(estimation(&c, divine, &ligue, date));
    }
    Err(non_cote(format!("Pas de cote pour cet objet en {ligue}.")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(unite: f64, quantite: u32) -> cote::Cote {
        cote::Cote {
            unite,
            quantite,
            total: unite * f64::from(quantite),
        }
    }

    #[test]
    fn passe_en_divine_au_dela_d_une_divine() {
        let p = estimation(
            &c(47.5, 20),
            Some(500.0),
            "Rise",
            "2026-11-18T19:00:00Z".into(),
        );
        assert_eq!((p.currency.as_str(), p.median), ("divine", 1.9));
        assert_eq!((p.quantity, p.unit_price), (Some(20), Some(0.095)));
        let p = estimation(&c(2.0, 1), Some(500.0), "Rise", String::new());
        assert_eq!((p.currency.as_str(), p.median), ("exalted", 2.0));
        assert_eq!((p.quantity, p.unit_price), (None, None));
        // Cours inconnu : on reste en Exalted.
        assert_eq!(
            estimation(&c(900.0, 1), None, "Rise", String::new()).currency,
            "exalted"
        );
    }
}
