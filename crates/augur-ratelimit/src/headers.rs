//! Lecture des en-têtes de rate limit renvoyés par les API de GGG.
//!
//! Format documenté sur `pathofexile.com/developer/docs` :
//!
//! ```text
//! X-Rate-Limit-Policy: trade-search-request-limit
//! X-Rate-Limit-Rules: Ip,Account
//! X-Rate-Limit-Ip: 8:10:60,15:60:120,60:300:1800
//! X-Rate-Limit-Ip-State: 1:10:0,1:60:0,1:300:0
//! Retry-After: 60            (seulement sur un 429)
//! ```
//!
//! Chaque règle porte une ou plusieurs fenêtres séparées par des virgules.
//! Une fenêtre de définition vaut `hits_max:période_s:restriction_s`, une
//! fenêtre d'état vaut `hits_courants:période_s:restriction_active_s`.

use thiserror::Error;

/// Une fenêtre d'une règle, telle que définie par le serveur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowLimit {
    /// Nombre maximal de requêtes dans la fenêtre.
    pub max_hits: u32,
    /// Durée de la fenêtre glissante, en secondes.
    pub period_s: u32,
    /// Durée de la restriction appliquée si la fenêtre est dépassée, en secondes.
    pub restriction_s: u32,
}

/// L'état courant d'une fenêtre, tel que vu par le serveur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowState {
    /// Requêtes comptées par le serveur dans la fenêtre.
    pub hits: u32,
    /// Durée de la fenêtre, en secondes (sert à apparier état et limite).
    pub period_s: u32,
    /// Restriction en cours, en secondes ; 0 si aucune.
    pub active_restriction_s: u32,
}

/// Une règle (`ip`, `account`, `client`…) et ses fenêtres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Nom de la règle, normalisé en minuscules.
    pub name: String,
    pub limits: Vec<WindowLimit>,
    pub states: Vec<WindowState>,
}

/// Tout ce qu'une réponse nous apprend sur une politique de rate limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySnapshot {
    /// Nom de la politique (plusieurs endpoints peuvent la partager).
    pub policy: String,
    pub rules: Vec<Rule>,
    /// Valeur de `Retry-After`, si présente.
    pub retry_after_s: Option<u32>,
}

impl PolicySnapshot {
    /// Restriction active la plus longue signalée par le serveur, en secondes.
    pub fn max_active_restriction_s(&self) -> u32 {
        let from_states = self
            .rules
            .iter()
            .flat_map(|r| r.states.iter())
            .map(|s| s.active_restriction_s)
            .max()
            .unwrap_or(0);
        from_states.max(self.retry_after_s.unwrap_or(0))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HeaderError {
    #[error("en-tête {header} manquant alors que X-Rate-Limit-Rules le référence")]
    Missing { header: String },
    #[error("valeur invalide pour {header} : {value:?}")]
    Invalid { header: String, value: String },
}

/// Lit une politique à partir d'une fonction de recherche d'en-tête
/// insensible à la casse. Renvoie `Ok(None)` si la réponse ne porte aucune
/// politique (endpoint non limité, ou réponse d'un proxy).
///
/// La fonction de recherche permet de rester indépendant du client HTTP
/// (reqwest, tauri-plugin-http, hyper…).
pub fn parse_policy<'a, F>(get: F) -> Result<Option<PolicySnapshot>, HeaderError>
where
    F: Fn(&str) -> Option<&'a str>,
{
    let Some(policy) = get("x-rate-limit-policy") else {
        return Ok(None);
    };
    let retry_after_s = match get("retry-after") {
        Some(v) => Some(parse_u32("retry-after", v.trim())?),
        None => None,
    };
    let rules_header = get("x-rate-limit-rules").unwrap_or("");
    let mut rules = Vec::new();
    for raw_name in rules_header
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let name = raw_name.to_ascii_lowercase();
        let limit_header = format!("x-rate-limit-{name}");
        let state_header = format!("x-rate-limit-{name}-state");
        let limits_raw = get(&limit_header).ok_or_else(|| HeaderError::Missing {
            header: limit_header.clone(),
        })?;
        let limits = parse_triples(&limit_header, limits_raw)?
            .into_iter()
            .map(|(a, b, c)| WindowLimit {
                max_hits: a,
                period_s: b,
                restriction_s: c,
            })
            .collect();
        let states = match get(&state_header) {
            Some(raw) => parse_triples(&state_header, raw)?
                .into_iter()
                .map(|(a, b, c)| WindowState {
                    hits: a,
                    period_s: b,
                    active_restriction_s: c,
                })
                .collect(),
            None => Vec::new(),
        };
        rules.push(Rule {
            name,
            limits,
            states,
        });
    }
    Ok(Some(PolicySnapshot {
        policy: policy.trim().to_owned(),
        rules,
        retry_after_s,
    }))
}

/// Variante pratique pour une liste de paires (nom, valeur).
pub fn parse_policy_from_pairs<'a, I>(pairs: I) -> Result<Option<PolicySnapshot>, HeaderError>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let pairs: Vec<(&str, &str)> = pairs.into_iter().collect();
    parse_policy(|name| {
        pairs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    })
}

fn parse_triples(header: &str, raw: &str) -> Result<Vec<(u32, u32, u32)>, HeaderError> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|triple| {
            let mut it = triple.split(':');
            let parts = (it.next(), it.next(), it.next(), it.next());
            match parts {
                (Some(a), Some(b), Some(c), None) => Ok((
                    parse_u32(header, a)?,
                    parse_u32(header, b)?,
                    parse_u32(header, c)?,
                )),
                _ => Err(HeaderError::Invalid {
                    header: header.to_owned(),
                    value: raw.to_owned(),
                }),
            }
        })
        .collect()
}

fn parse_u32(header: &str, v: &str) -> Result<u32, HeaderError> {
    v.trim().parse().map_err(|_| HeaderError::Invalid {
        header: header.to_owned(),
        value: v.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade_headers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("X-Rate-Limit-Policy", "trade-search-request-limit"),
            ("X-Rate-Limit-Rules", "Ip,Account"),
            ("X-Rate-Limit-Ip", "8:10:60,15:60:120,60:300:1800"),
            ("X-Rate-Limit-Ip-State", "1:10:0,1:60:0,1:300:0"),
            ("X-Rate-Limit-Account", "3:5:60"),
            ("X-Rate-Limit-Account-State", "1:5:0"),
        ]
    }

    #[test]
    fn lit_une_politique_multi_regles() {
        let p = parse_policy_from_pairs(trade_headers()).unwrap().unwrap();
        assert_eq!(p.policy, "trade-search-request-limit");
        assert_eq!(p.rules.len(), 2);
        assert_eq!(p.rules[0].name, "ip");
        assert_eq!(
            p.rules[0].limits[2],
            WindowLimit {
                max_hits: 60,
                period_s: 300,
                restriction_s: 1800
            }
        );
        assert_eq!(
            p.rules[1].states[0],
            WindowState {
                hits: 1,
                period_s: 5,
                active_restriction_s: 0
            }
        );
        assert_eq!(p.max_active_restriction_s(), 0);
    }

    #[test]
    fn sans_politique_renvoie_none() {
        assert_eq!(
            parse_policy_from_pairs([("content-type", "application/json")]).unwrap(),
            None
        );
    }

    #[test]
    fn retry_after_et_restriction_active() {
        let mut h = trade_headers();
        h[3] = ("X-Rate-Limit-Ip-State", "9:10:60,9:60:0,9:300:0");
        h.push(("Retry-After", "60"));
        let p = parse_policy_from_pairs(h).unwrap().unwrap();
        assert_eq!(p.retry_after_s, Some(60));
        assert_eq!(p.max_active_restriction_s(), 60);
    }

    #[test]
    fn regle_manquante_est_une_erreur() {
        let h = vec![
            ("X-Rate-Limit-Policy", "p"),
            ("X-Rate-Limit-Rules", "client"),
        ];
        assert!(matches!(
            parse_policy_from_pairs(h),
            Err(HeaderError::Missing { .. })
        ));
    }

    #[test]
    fn triple_mal_forme_est_une_erreur() {
        let h = vec![
            ("X-Rate-Limit-Policy", "p"),
            ("X-Rate-Limit-Rules", "client"),
            ("X-Rate-Limit-Client", "10:5"),
        ];
        assert!(matches!(
            parse_policy_from_pairs(h),
            Err(HeaderError::Invalid { .. })
        ));
    }
}
