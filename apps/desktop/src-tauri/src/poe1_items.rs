//! Overlay de PoE 1 : cote poe.ninja de l'objet copié et recherche sur le site
//! de trade de PoE 1.
//!
//! poe.ninja est interrogé à la demande, une page à la fois, seulement pour les
//! objets qu'il cote (devises, cartes, uniques). Chaque page est gardée 30
//! minutes : relire plusieurs objets de la même famille ne refait aucune requête.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use poe2_core::parser::{self, Objet, Rarete};
use poe2_core::poe1::{self, ninja, trade};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::contracts::{AppError, ErrorCode, app_error};
use crate::game::Game;

/// Durée de vie d'une page poe.ninja en mémoire.
const DUREE_CACHE: Duration = Duration::from_secs(30 * 60);

#[derive(Default)]
pub struct Poe1Ninja {
    pages: Mutex<HashMap<(String, ninja::Page), (Instant, Value)>>,
}

impl Poe1Ninja {
    fn en_cache(&self, ligue: &str, page: ninja::Page) -> Option<Value> {
        let pages = self.pages.lock().unwrap_or_else(|e| e.into_inner());
        pages
            .get(&(ligue.to_string(), page))
            .filter(|(at, _)| at.elapsed() < DUREE_CACHE)
            .map(|(_, v)| v.clone())
    }

    fn garder(&self, ligue: &str, page: ninja::Page, doc: Value) {
        let mut pages = self.pages.lock().unwrap_or_else(|e| e.into_inner());
        pages.retain(|_, (at, _)| at.elapsed() < DUREE_CACHE);
        pages.insert((ligue.to_string(), page), (Instant::now(), doc));
    }

    pub(crate) async fn page(
        &self,
        client: &reqwest::Client,
        ligue: &str,
        page: ninja::Page,
    ) -> Result<Value, AppError> {
        if let Some(doc) = self.en_cache(ligue, page) {
            return Ok(doc);
        }
        let url = format!(
            "{}/{}&league={}",
            ninja::API,
            page.chemin(),
            crate::forge_prices::encode(ligue)
        );
        let doc = crate::forge_prices::get_json(client, &url).await?;
        self.garder(ligue, page, doc.clone());
        Ok(doc)
    }
}

/// Ce que l'overlay de PoE 1 affiche pour l'objet copié.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Poe1ItemInfo {
    pub name: Option<String>,
    pub base: String,
    pub item_class: Option<String>,
    pub rarity: &'static str,
    pub item_level: Option<u32>,
    pub links: u32,
    pub influences: Vec<String>,
    pub corrupted: bool,
    /// Lignes de mods, telles que copiées (implicites d'abord).
    pub mods: Vec<String>,
    /// Cote poe.ninja, si l'objet en a une.
    pub price: Option<ninja::Cote>,
    /// Pourquoi il n'y a pas de cote (objet non coté, poe.ninja injoignable).
    pub price_note: Option<String>,
    /// Ce que la recherche de trade cherchera (Alt+U).
    pub trade_summary: Vec<String>,
    pub league: String,
}

fn rarete(r: Rarete) -> &'static str {
    match r {
        Rarete::Normal => "normal",
        Rarete::Magique => "magic",
        Rarete::Rare => "rare",
        Rarete::Unique => "unique",
        Rarete::Devise => "currency",
        Rarete::Gemme => "gem",
        Rarete::CarteDivination => "divinationCard",
    }
}

fn dernier_objet(app: &AppHandle) -> Result<Objet, AppError> {
    let texte = app.state::<crate::AppState>().last_text().ok_or_else(|| {
        app_error(
            ErrorCode::Internal,
            "Aucun objet copié : fais Ctrl+C sur un objet en jeu.",
        )
    })?;
    parser::parser(&texte).map_err(|e| app_error(ErrorCode::Internal, e.to_string()))
}

fn ligue(app: &AppHandle) -> String {
    app.state::<crate::settings::SettingsStore>()
        .get()
        .league_for(Game::Poe1)
        .to_string()
}

/// Objet copié en dernier, avec sa cote poe.ninja.
pub async fn info(app: &AppHandle) -> Result<Poe1ItemInfo, AppError> {
    let objet = dernier_objet(app)?;
    let ligue = ligue(app);
    let (_, trade_summary) = trade::requete(&objet);
    let mut info = Poe1ItemInfo {
        name: objet.nom.clone(),
        base: objet.base.clone(),
        item_class: objet.classe.clone(),
        rarity: rarete(objet.rarete),
        item_level: objet.niveau_objet,
        links: poe1::liens(&objet),
        influences: objet.influences.clone(),
        corrupted: objet.corrompu,
        mods: objet.mods.iter().map(|m| m.texte.clone()).collect(),
        price: None,
        price_note: None,
        trade_summary,
        league: ligue.clone(),
    };
    let pages = ninja::pages(&objet);
    if pages.is_empty() {
        info.price_note = Some(match objet.rarete {
            Rarete::Gemme => "Pas encore de cote pour les gemmes.".into(),
            _ => "poe.ninja ne cote pas ce genre d'objet : compare sur le site de trade.".into(),
        });
        return Ok(info);
    }
    let client = crate::forge_prices::client(&app.package_info().version.to_string())?;
    let ninja = app.state::<Poe1Ninja>();
    let devises = ninja::Page::Echange("Currency");
    for page in pages {
        let doc = match ninja.page(&client, &ligue, page).await {
            Ok(doc) => doc,
            Err(e) => {
                info.price_note = Some(e.message);
                return Ok(info);
            }
        };
        let cours = if page == devises {
            ninja::cours_divine(&doc)
        } else {
            None
        };
        if let Some(mut cote) = ninja::coter(page, &doc, &objet, cours) {
            if cote.divine.is_none() {
                // Cours de la Divine : page des devises (souvent déjà en mémoire).
                if let Ok(d) = ninja.page(&client, &ligue, devises).await
                    && let Some(cours) = ninja::cours_divine(&d)
                {
                    cote.divine = Some((cote.chaos / cours * 100.0).round() / 100.0);
                }
            }
            info.price = Some(cote);
            return Ok(info);
        }
    }
    info.price_note = Some(format!("Pas de cote poe.ninja en {ligue}."));
    Ok(info)
}

/// Ouvre la recherche de l'objet copié sur le site de trade de PoE 1.
pub fn open_trade(app: &AppHandle) -> Result<String, AppError> {
    let objet = dernier_objet(app)?;
    let (requete, _) = trade::requete(&objet);
    let url = crate::upgrade_search::adresse_requete_poe1(&ligue(app), &requete);
    crate::upgrade_search::ouvrir(&url)?;
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_de_trade_poe1() {
        let objet = parser::parser(
            "Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n--------\nStack Size: 1/20",
        )
        .unwrap();
        let (q, _) = trade::requete(&objet);
        let url = crate::upgrade_search::adresse_requete_poe1("Hardcore Covens", &q);
        assert!(
            url.starts_with("https://www.pathofexile.com/trade/search/Hardcore%20Covens?q="),
            "{url}"
        );
    }
}
