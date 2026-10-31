//! Machine à états pure d'une politique de rate limit.
//!
//! Aucune horloge interne : chaque appel reçoit `now`. Le comportement est
//! donc déterministe et testable en temps simulé (voir `tests/saturation.rs`).
//!
//! Principes :
//! - **Rien n'est codé en dur** : les fenêtres viennent des en-têtes renvoyés
//!   par le serveur. Tant qu'on ne les connaît pas, une seule requête à la
//!   fois est autorisée (amorçage prudent).
//! - **Marge de sécurité** : on ne consomme jamais la totalité d'une fenêtre,
//!   car le budget est partagé (le joueur qui utilise le site de trade dans son
//!   navigateur consomme la même règle `ip`).
//! - **Réconciliation** : si le serveur compte plus de requêtes que nous, on
//!   s'aligne sur lui (hypothèse la plus prudente).
//! - **Priorités** : les requêtes de fond ne peuvent pas entamer la réserve
//!   gardée pour les actions explicites du joueur.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::headers::{PolicySnapshot, WindowLimit};

/// Priorité d'une requête.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Priority {
    /// Déclenchée par une action explicite du joueur (Ctrl+C, bouton).
    Interactive,
    /// Rafraîchissement ou ingestion en arrière-plan.
    Background,
}

/// Réglages du limiteur. Les valeurs par défaut sont volontairement prudentes.
#[derive(Debug, Clone, Copy)]
pub struct LimiterConfig {
    /// Fraction de chaque fenêtre jamais consommée (arrondie au supérieur).
    pub safety_margin: f64,
    /// Fraction de la capacité restante réservée aux requêtes interactives.
    pub background_headroom: f64,
    /// Requêtes simultanées autorisées tant que la politique est inconnue.
    pub bootstrap_concurrency: u32,
    /// Attente avant de réessayer pendant l'amorçage.
    pub bootstrap_poll: Duration,
    /// Blocage appliqué sur un 429 qui n'indique aucune durée.
    pub fallback_restriction: Duration,
}

impl Default for LimiterConfig {
    fn default() -> Self {
        Self {
            safety_margin: 0.10,
            background_headroom: 0.25,
            bootstrap_concurrency: 1,
            bootstrap_poll: Duration::from_millis(250),
            fallback_restriction: Duration::from_secs(60),
        }
    }
}

/// Résultat d'une demande de jeton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// La requête peut partir maintenant.
    Granted,
    /// Réessayer après ce délai.
    Wait(Duration),
}

#[derive(Debug, Clone)]
struct TrackedWindow {
    rule: String,
    limit: WindowLimit,
    hits: VecDeque<Instant>,
}

impl TrackedWindow {
    fn period(&self) -> Duration {
        Duration::from_secs(u64::from(self.limit.period_s))
    }

    fn prune(&mut self, now: Instant) {
        let period = self.period();
        while self.hits.front().is_some_and(|&t| t + period <= now) {
            self.hits.pop_front();
        }
    }
}

/// Vue en lecture d'une fenêtre, pour l'interface (file d'attente visible).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowView {
    pub rule: String,
    pub hits: u32,
    pub max_hits: u32,
    pub usable_hits: u32,
    pub period_s: u32,
}

/// Vue en lecture d'une politique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyView {
    pub known: bool,
    pub windows: Vec<WindowView>,
    pub blocked_for: Option<Duration>,
}

/// L'état local d'une politique de rate limit.
#[derive(Debug, Clone)]
pub struct PolicyLimiter {
    cfg: LimiterConfig,
    known: bool,
    windows: Vec<TrackedWindow>,
    blocked_until: Option<Instant>,
    in_flight: u32,
}

impl PolicyLimiter {
    pub fn new(cfg: LimiterConfig) -> Self {
        Self {
            cfg,
            known: false,
            windows: Vec::new(),
            blocked_until: None,
            in_flight: 0,
        }
    }

    /// Vrai une fois qu'une réponse a fait connaître les fenêtres.
    pub fn is_known(&self) -> bool {
        self.known
    }

    fn usable(&self, limit: &WindowLimit) -> u32 {
        let margin = (f64::from(limit.max_hits) * self.cfg.safety_margin).ceil() as u32;
        limit.max_hits.saturating_sub(margin).max(1)
    }

    fn cap(&self, limit: &WindowLimit, priority: Priority) -> u32 {
        let usable = self.usable(limit);
        match priority {
            Priority::Interactive => usable,
            Priority::Background => {
                let reserved = (f64::from(usable) * self.cfg.background_headroom).ceil() as u32;
                usable.saturating_sub(reserved).max(1)
            }
        }
    }

    /// Demande l'autorisation d'envoyer une requête à l'instant `now`.
    /// En cas d'accord, la requête est comptée immédiatement.
    pub fn try_acquire(&mut self, now: Instant, priority: Priority) -> Decision {
        if let Some(until) = self.blocked_until {
            if until > now {
                return Decision::Wait(until - now);
            }
            self.blocked_until = None;
        }

        if !self.known {
            if self.in_flight >= self.cfg.bootstrap_concurrency {
                return Decision::Wait(self.cfg.bootstrap_poll);
            }
            self.in_flight += 1;
            return Decision::Granted;
        }

        let mut wait = Duration::ZERO;
        for i in 0..self.windows.len() {
            self.windows[i].prune(now);
            let w = &self.windows[i];
            let cap = self.cap(&w.limit, priority) as usize;
            if w.hits.len() >= cap {
                // Il faut que (len - cap + 1) entrées expirent.
                let idx = w.hits.len() - cap;
                let free_at = w.hits[idx] + w.period();
                wait = wait.max(free_at.saturating_duration_since(now));
            }
        }
        if !wait.is_zero() {
            return Decision::Wait(wait);
        }

        for w in &mut self.windows {
            w.hits.push_back(now);
        }
        self.in_flight += 1;
        Decision::Granted
    }

    /// À appeler à chaque réponse HTTP, avec la politique lue dans ses
    /// en-têtes (si présente) et son code de statut.
    pub fn on_response(&mut self, now: Instant, snapshot: Option<&PolicySnapshot>, status: u16) {
        self.in_flight = self.in_flight.saturating_sub(1);

        if let Some(s) = snapshot {
            self.apply_snapshot(now, s);
        }

        let mut restriction = snapshot
            .map(|s| Duration::from_secs(u64::from(s.max_active_restriction_s())))
            .unwrap_or_default();
        if status == 429 && restriction.is_zero() {
            restriction = self
                .windows
                .iter()
                .map(|w| Duration::from_secs(u64::from(w.limit.restriction_s)))
                .max()
                .filter(|d| !d.is_zero())
                .unwrap_or(self.cfg.fallback_restriction);
        }
        if !restriction.is_zero() {
            let until = now + restriction;
            self.blocked_until = Some(self.blocked_until.map_or(until, |b| b.max(until)));
        }
    }

    /// À appeler quand la requête n'a obtenu aucune réponse (erreur réseau,
    /// annulation). Le jeton reste compté dans les fenêtres : on ne sait pas
    /// si le serveur l'a reçu.
    pub fn on_transport_error(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
    }

    fn apply_snapshot(&mut self, now: Instant, s: &PolicySnapshot) {
        let mut next = Vec::new();
        for rule in &s.rules {
            for limit in &rule.limits {
                // On conserve l'historique local d'une fenêtre déjà connue.
                let mut hits = self
                    .windows
                    .iter()
                    .find(|w| w.rule == rule.name && w.limit.period_s == limit.period_s)
                    .map(|w| w.hits.clone())
                    .unwrap_or_default();
                if !self.known {
                    // Requête d'amorçage : elle a été comptée côté serveur.
                    hits.push_back(now);
                }
                let mut w = TrackedWindow {
                    rule: rule.name.clone(),
                    limit: *limit,
                    hits,
                };
                w.prune(now);
                // Réconciliation avec le compteur du serveur.
                if let Some(state) = rule.states.iter().find(|st| st.period_s == limit.period_s) {
                    let local = w.hits.len() as u32;
                    for _ in local..state.hits {
                        w.hits.push_back(now);
                    }
                }
                next.push(w);
            }
        }
        self.windows = next;
        self.known = true;
    }

    /// État lisible pour l'interface et la télémétrie.
    pub fn view(&self, now: Instant) -> PolicyView {
        let windows = self
            .windows
            .iter()
            .map(|w| {
                let period = w.period();
                let hits = w.hits.iter().filter(|&&t| t + period > now).count() as u32;
                WindowView {
                    rule: w.rule.clone(),
                    hits,
                    max_hits: w.limit.max_hits,
                    usable_hits: self.usable(&w.limit),
                    period_s: w.limit.period_s,
                }
            })
            .collect();
        let blocked_for = self.blocked_until.filter(|&u| u > now).map(|u| u - now);
        PolicyView {
            known: self.known,
            windows,
            blocked_for,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headers::parse_policy_from_pairs;

    fn snapshot(state: &str) -> PolicySnapshot {
        parse_policy_from_pairs([
            ("X-Rate-Limit-Policy", "p"),
            ("X-Rate-Limit-Rules", "client"),
            ("X-Rate-Limit-Client", "10:5:10"),
            ("X-Rate-Limit-Client-State", state),
        ])
        .unwrap()
        .unwrap()
    }

    #[test]
    fn amorcage_une_requete_a_la_fois() {
        let t0 = Instant::now();
        let mut l = PolicyLimiter::new(LimiterConfig::default());
        assert_eq!(l.try_acquire(t0, Priority::Interactive), Decision::Granted);
        assert!(matches!(
            l.try_acquire(t0, Priority::Interactive),
            Decision::Wait(_)
        ));
        l.on_response(t0, Some(&snapshot("1:5:0")), 200);
        assert!(l.is_known());
        assert_eq!(l.try_acquire(t0, Priority::Interactive), Decision::Granted);
    }

    #[test]
    fn respecte_la_marge_de_securite() {
        let t0 = Instant::now();
        let mut l = PolicyLimiter::new(LimiterConfig::default());
        l.try_acquire(t0, Priority::Interactive);
        l.on_response(t0, Some(&snapshot("1:5:0")), 200);
        // 10 max, marge 10 % → 9 utilisables, dont 1 déjà consommé.
        for _ in 0..8 {
            assert_eq!(l.try_acquire(t0, Priority::Interactive), Decision::Granted);
        }
        assert_eq!(
            l.try_acquire(t0, Priority::Interactive),
            Decision::Wait(Duration::from_secs(5))
        );
    }

    #[test]
    fn le_fond_laisse_une_reserve_au_joueur() {
        let t0 = Instant::now();
        let mut l = PolicyLimiter::new(LimiterConfig::default());
        l.try_acquire(t0, Priority::Background);
        l.on_response(t0, Some(&snapshot("1:5:0")), 200);
        let mut granted = 1;
        while l.try_acquire(t0, Priority::Background) == Decision::Granted {
            granted += 1;
        }
        // 9 utilisables, 25 % réservés (3) → 6 pour le fond.
        assert_eq!(granted, 6);
        assert_eq!(l.try_acquire(t0, Priority::Interactive), Decision::Granted);
    }

    #[test]
    fn s_aligne_sur_le_compteur_du_serveur() {
        let t0 = Instant::now();
        let mut l = PolicyLimiter::new(LimiterConfig::default());
        l.try_acquire(t0, Priority::Interactive);
        // Le navigateur du joueur a consommé 8 requêtes de la même règle.
        l.on_response(t0, Some(&snapshot("9:5:0")), 200);
        assert!(matches!(
            l.try_acquire(t0, Priority::Interactive),
            Decision::Wait(_)
        ));
    }

    #[test]
    fn un_429_bloque_pendant_la_restriction() {
        let t0 = Instant::now();
        let mut l = PolicyLimiter::new(LimiterConfig::default());
        l.try_acquire(t0, Priority::Interactive);
        let mut s = snapshot("11:5:10");
        s.retry_after_s = Some(10);
        l.on_response(t0, Some(&s), 429);
        assert_eq!(
            l.try_acquire(t0, Priority::Interactive),
            Decision::Wait(Duration::from_secs(10))
        );
        assert_eq!(l.view(t0).blocked_for, Some(Duration::from_secs(10)));
    }

    #[test]
    fn un_429_sans_en_tetes_applique_le_repli() {
        let t0 = Instant::now();
        let mut l = PolicyLimiter::new(LimiterConfig::default());
        l.try_acquire(t0, Priority::Interactive);
        l.on_response(t0, None, 429);
        assert_eq!(
            l.try_acquire(t0, Priority::Interactive),
            Decision::Wait(Duration::from_secs(60))
        );
    }
}
