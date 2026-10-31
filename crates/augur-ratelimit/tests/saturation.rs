//! Test de saturation (Definition of Done de l'architecte, sprint 1) :
//! le limiteur encaisse une demande largement supérieure au budget sans
//! provoquer un seul 429, y compris quand une autre source (le navigateur
//! du joueur sur le site de trade) consomme la même règle `ip`.
//!
//! Le serveur GGG est simulé en temps virtuel avec les fenêtres observées
//! publiquement sur la recherche de trade ; le limiteur, lui, ne les connaît
//! pas à l'avance et les apprend par les en-têtes.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use augur_ratelimit::{Decision, LimiterConfig, PolicyLimiter, Priority, parse_policy_from_pairs};

struct FakeServer {
    /// (max, période_s, restriction_s, historique)
    windows: Vec<(u32, u32, u32, VecDeque<Instant>)>,
    restricted_until: Option<Instant>,
}

impl FakeServer {
    fn new() -> Self {
        let w = |m, p, r| (m, p, r, VecDeque::new());
        Self {
            windows: vec![w(8, 10, 60), w(15, 60, 120), w(60, 300, 1800)],
            restricted_until: None,
        }
    }

    fn hit(&mut self, now: Instant) -> (u16, Vec<(String, String)>) {
        for (_, p, _, h) in &mut self.windows {
            let period = Duration::from_secs(u64::from(*p));
            while h.front().is_some_and(|&t| t + period <= now) {
                h.pop_front();
            }
        }
        let mut status = 200;
        let mut retry_after = None;
        if let Some(u) = self.restricted_until.filter(|&u| u > now) {
            status = 429;
            retry_after = Some((u - now).as_secs());
        } else {
            for (m, _, r, h) in &mut self.windows {
                h.push_back(now);
                if h.len() as u32 > *m {
                    status = 429;
                    let until = now + Duration::from_secs(u64::from(*r));
                    self.restricted_until = Some(until);
                    retry_after = Some(u64::from(*r));
                }
            }
        }
        let limits = self
            .windows
            .iter()
            .map(|(m, p, r, _)| format!("{m}:{p}:{r}"))
            .collect::<Vec<_>>()
            .join(",");
        let active = retry_after.unwrap_or(0);
        let state = self
            .windows
            .iter()
            .map(|(_, p, _, h)| format!("{}:{p}:{active}", h.len()))
            .collect::<Vec<_>>()
            .join(",");
        let mut headers = vec![
            (
                "X-Rate-Limit-Policy".to_owned(),
                "trade-search-request-limit".to_owned(),
            ),
            ("X-Rate-Limit-Rules".to_owned(), "Ip".to_owned()),
            ("X-Rate-Limit-Ip".to_owned(), limits),
            ("X-Rate-Limit-Ip-State".to_owned(), state),
        ];
        if let Some(r) = retry_after {
            headers.push(("Retry-After".to_owned(), r.to_string()));
        }
        (status, headers)
    }
}

/// Simule `duration` de demande continue (une requête voulue toutes les
/// `demand_every`), avec une latence réseau et un trafic externe optionnel.
fn run(duration: Duration, demand_every: Duration, external_every: Option<Duration>) -> (u32, u32) {
    let t0 = Instant::now();
    let end = t0 + duration;
    let latency = Duration::from_millis(150);
    let mut server = FakeServer::new();
    let mut limiter = PolicyLimiter::new(LimiterConfig::default());
    let mut now = t0;
    let mut next_external = external_every.map(|e| t0 + e);
    let (mut sent, mut throttled) = (0u32, 0u32);

    while now < end {
        // Trafic externe sur la même IP (navigateur du joueur).
        while let Some(t) = next_external.filter(|&t| t <= now) {
            server.hit(t);
            next_external = external_every.map(|e| t + e);
        }
        match limiter.try_acquire(now, Priority::Interactive) {
            Decision::Granted => {
                let (status, headers) = server.hit(now + latency);
                let snap =
                    parse_policy_from_pairs(headers.iter().map(|(k, v)| (k.as_str(), v.as_str())))
                        .unwrap();
                now += latency * 2;
                limiter.on_response(now, snap.as_ref(), status);
                sent += 1;
                if status == 429 {
                    throttled += 1;
                }
                now += demand_every;
            }
            Decision::Wait(d) => now += d,
        }
    }
    (sent, throttled)
}

#[test]
fn saturation_sans_aucun_429() {
    // Demande : une recherche toutes les 100 ms pendant 1 h, soit 36 000.
    let (sent, throttled) = run(Duration::from_secs(3600), Duration::from_millis(100), None);
    assert_eq!(throttled, 0, "429 reçus sur {sent} requêtes");
    // Le budget doit quand même être bien utilisé : la fenêtre de 300 s
    // autorise 60 requêtes, la marge en garde 54, soit ~650 par heure.
    assert!(sent >= 600, "seulement {sent} requêtes envoyées");
}

#[test]
fn saturation_avec_navigateur_en_parallele() {
    // Le joueur utilise aussi le site de trade : une recherche par minute.
    let (sent, throttled) = run(
        Duration::from_secs(3600),
        Duration::from_millis(100),
        Some(Duration::from_secs(60)),
    );
    assert_eq!(throttled, 0, "429 reçus sur {sent} requêtes");
}
