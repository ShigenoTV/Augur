//! Gestionnaire asynchrone partagé : le « budget manager ».
//!
//! Un seul `BudgetManager` par processus (appli Windows ou worker
//! d'ingestion). Aucun code ne doit appeler GGG sans passer par lui.
//!
//! ```no_run
//! # async fn demo(mgr: &augur_ratelimit::BudgetManager) {
//! use augur_ratelimit::{Priority, parse_policy_from_pairs};
//! let permit = mgr.acquire("trade2-search", Priority::Interactive).await;
//! // … envoyer la requête HTTP …
//! let headers = [("X-Rate-Limit-Policy", "trade-search-request-limit")];
//! let snapshot = parse_policy_from_pairs(headers).ok().flatten();
//! permit.complete(snapshot.as_ref(), 200);
//! # }
//! ```
//!
//! Les clés passées à `acquire` désignent un endpoint côté appelant. GGG
//! peut faire partager une même politique à plusieurs endpoints : dès qu'une
//! réponse révèle le nom de la politique, la clé y est rattachée.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio::time::{Instant, sleep};

use crate::headers::PolicySnapshot;
use crate::limiter::{Decision, LimiterConfig, PolicyLimiter, PolicyView, Priority};

#[derive(Default)]
struct State {
    /// Limiteurs indexés par nom de politique (ou par clé d'endpoint tant
    /// que la politique est inconnue).
    limiters: HashMap<String, PolicyLimiter>,
    /// Clé d'endpoint → nom de politique.
    aliases: HashMap<String, String>,
    /// Requêtes interactives en attente, par politique.
    interactive_waiting: HashMap<String, u32>,
}

impl State {
    fn key<'a>(&'a self, endpoint: &'a str) -> &'a str {
        self.aliases.get(endpoint).map_or(endpoint, String::as_str)
    }
}

/// Le budget manager.
pub struct BudgetManager {
    cfg: LimiterConfig,
    state: Mutex<State>,
}

impl Default for BudgetManager {
    fn default() -> Self {
        Self::new(LimiterConfig::default())
    }
}

impl BudgetManager {
    pub fn new(cfg: LimiterConfig) -> Self {
        Self {
            cfg,
            state: Mutex::new(State::default()),
        }
    }

    /// Attend qu'une requête vers `endpoint` puisse partir sans risque.
    pub async fn acquire(&self, endpoint: &str, priority: Priority) -> Permit<'_> {
        let mut registered = false;
        loop {
            let wait = {
                let mut st = self.state.lock().expect("verrou du budget manager");
                let key = st.key(endpoint).to_owned();
                let interactive_waiting = st.interactive_waiting.get(&key).copied().unwrap_or(0);
                let decision = if priority == Priority::Background && interactive_waiting > 0 {
                    Decision::Wait(self.cfg.bootstrap_poll)
                } else {
                    let now = Instant::now().into_std();
                    let cfg = self.cfg;
                    st.limiters
                        .entry(key.clone())
                        .or_insert_with(|| PolicyLimiter::new(cfg))
                        .try_acquire(now, priority)
                };
                match decision {
                    Decision::Granted => {
                        if registered && let Some(n) = st.interactive_waiting.get_mut(&key) {
                            *n = n.saturating_sub(1);
                        }
                        return Permit {
                            mgr: self,
                            endpoint: endpoint.to_owned(),
                            done: false,
                        };
                    }
                    Decision::Wait(d) => {
                        if priority == Priority::Interactive && !registered {
                            *st.interactive_waiting.entry(key).or_default() += 1;
                            registered = true;
                        }
                        d
                    }
                }
            };
            sleep(wait).await;
        }
    }

    fn complete(&self, endpoint: &str, snapshot: Option<&PolicySnapshot>, status: u16) {
        let now = Instant::now().into_std();
        let mut st = self.state.lock().expect("verrou du budget manager");
        let old_key = st.key(endpoint).to_owned();
        let cfg = self.cfg;

        // Rattache l'endpoint à sa politique dès qu'elle est connue.
        let key = match snapshot {
            Some(s) if s.policy != old_key => {
                let policy = s.policy.clone();
                if let Some(limiter) = st.limiters.remove(&old_key) {
                    st.limiters.entry(policy.clone()).or_insert(limiter);
                }
                if let Some(n) = st.interactive_waiting.remove(&old_key) {
                    *st.interactive_waiting.entry(policy.clone()).or_default() += n;
                }
                st.aliases.insert(endpoint.to_owned(), policy.clone());
                policy
            }
            _ => old_key,
        };
        st.limiters
            .entry(key)
            .or_insert_with(|| PolicyLimiter::new(cfg))
            .on_response(now, snapshot, status);
    }

    fn abort(&self, endpoint: &str) {
        let mut st = self.state.lock().expect("verrou du budget manager");
        let key = st.key(endpoint).to_owned();
        if let Some(l) = st.limiters.get_mut(&key) {
            l.on_transport_error();
        }
    }

    /// État de toutes les politiques connues, pour l'UI et la télémétrie.
    pub fn snapshot(&self) -> Vec<(String, PolicyView)> {
        let now = Instant::now().into_std();
        let st = self.state.lock().expect("verrou du budget manager");
        let mut out: Vec<_> = st
            .limiters
            .iter()
            .map(|(k, l)| (k.clone(), l.view(now)))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

/// Jeton d'envoi. Doit être clos par [`Permit::complete`] ; s'il est
/// abandonné, la requête est considérée comme partie sans réponse.
#[must_use = "une requête autorisée doit être suivie de complete()"]
pub struct Permit<'a> {
    mgr: &'a BudgetManager,
    endpoint: String,
    done: bool,
}

impl Permit<'_> {
    /// Transmet la réponse reçue (politique lue dans les en-têtes, statut).
    pub fn complete(mut self, snapshot: Option<&PolicySnapshot>, status: u16) {
        self.done = true;
        self.mgr.complete(&self.endpoint, snapshot, status);
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.mgr.abort(&self.endpoint);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headers::parse_policy_from_pairs;
    use std::sync::Arc;
    use std::time::Duration;

    fn snap(policy: &str, state: &str) -> PolicySnapshot {
        parse_policy_from_pairs([
            ("X-Rate-Limit-Policy", policy),
            ("X-Rate-Limit-Rules", "ip"),
            ("X-Rate-Limit-Ip", "4:10:60"),
            ("X-Rate-Limit-Ip-State", state),
        ])
        .unwrap()
        .unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn deux_endpoints_partagent_une_politique() {
        let mgr = BudgetManager::default();
        mgr.acquire("search", Priority::Interactive)
            .await
            .complete(Some(&snap("trade", "1:10:0")), 200);
        mgr.acquire("fetch", Priority::Interactive)
            .await
            .complete(Some(&snap("trade", "2:10:0")), 200);
        let views = mgr.snapshot();
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].0, "trade");
        assert_eq!(views[0].1.windows[0].hits, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn attend_la_fin_de_la_fenetre() {
        let mgr = BudgetManager::default();
        let start = Instant::now();
        // 4 max, marge 10 % → 3 utilisables.
        for i in 1..=3 {
            let p = mgr.acquire("search", Priority::Interactive).await;
            p.complete(Some(&snap("trade", &format!("{i}:10:0"))), 200);
        }
        let p = mgr.acquire("search", Priority::Interactive).await;
        assert!(start.elapsed() >= Duration::from_secs(10));
        p.complete(Some(&snap("trade", "1:10:0")), 200);
    }

    #[tokio::test(start_paused = true)]
    async fn l_interactif_passe_avant_le_fond() {
        let mgr = Arc::new(BudgetManager::default());
        // Amorçage puis saturation de la fenêtre.
        for i in 1..=3 {
            let p = mgr.acquire("search", Priority::Interactive).await;
            p.complete(Some(&snap("trade", &format!("{i}:10:0"))), 200);
        }
        let order = Arc::new(Mutex::new(Vec::new()));
        let bg = {
            let (mgr, order) = (mgr.clone(), order.clone());
            tokio::spawn(async move {
                let p = mgr.acquire("search", Priority::Background).await;
                order.lock().unwrap().push("fond");
                p.complete(None, 200);
            })
        };
        tokio::task::yield_now().await;
        let fg = {
            let (mgr, order) = (mgr.clone(), order.clone());
            tokio::spawn(async move {
                let p = mgr.acquire("search", Priority::Interactive).await;
                order.lock().unwrap().push("joueur");
                p.complete(None, 200);
            })
        };
        fg.await.unwrap();
        bg.await.unwrap();
        assert_eq!(*order.lock().unwrap(), vec!["joueur", "fond"]);
    }

    #[tokio::test(start_paused = true)]
    async fn un_jeton_abandonne_libere_l_amorcage() {
        let mgr = BudgetManager::default();
        drop(mgr.acquire("search", Priority::Interactive).await);
        // Sans libération, cet appel attendrait indéfiniment.
        let p = tokio::time::timeout(
            Duration::from_secs(1),
            mgr.acquire("search", Priority::Interactive),
        )
        .await
        .expect("l'amorçage doit être libéré");
        p.complete(None, 200);
    }
}
