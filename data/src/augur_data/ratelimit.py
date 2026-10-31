"""Budget de rate limits GGG piloté par les en-têtes.

Le plan fondateur (section 0, point 1) impose que le rate limiter soit un
composant d'architecture, piloté par les en-têtes renvoyés par GGG et jamais
par des valeurs codées en dur. Le budget manager central est prototypé par le
chantier Architecture ; ce module définit le contrat que le collecteur attend
(`BudgetManager`) et une implémentation locale conforme, utilisable tant que
le service central n'est pas en place.

Rappel du format GGG (documentation développeur, à revérifier en S1) :

    X-Rate-Limit-Policy: trade-search-request-limit
    X-Rate-Limit-Rules: Ip,Account
    X-Rate-Limit-Ip: 8:10:60,15:60:120          (hits max : période s : pénalité s)
    X-Rate-Limit-Ip-State: 1:10:0,1:60:0         (hits en cours : période s : restriction active s)
    Retry-After: 60                              (sur une réponse 429)
"""

from __future__ import annotations

import threading
import time
from collections import deque
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from typing import Protocol

# Priorités alignées sur le budget manager Rust (crates/augur-ratelimit) :
# plus le nombre est petit, plus la requête passe tôt. L'ingestion de fond
# cède toujours la place aux besoins interactifs.
PRIORITY_INTERACTIVE = 0
PRIORITY_INGESTION = 10


class BudgetManager(Protocol):
    """Contrat entre les collecteurs et le budget manager.

    Même forme que le budget manager Rust du chantier Architecture
    (`acquire(policy, priorité)` puis `observe(en-têtes, statut)`), pour que
    le collecteur puisse basculer dessus sans changer de logique.

    `acquire` bloque jusqu'à ce qu'une requête soit autorisée sur la file
    `key`. `observe` transmet au budget la réponse reçue, pour qu'il apprenne
    la politique réelle. Aucun collecteur n'appelle GGG sans passer par là.
    """

    def acquire(self, key: str, priority: int = PRIORITY_INGESTION) -> None: ...

    def observe(self, key: str, status: int, headers: Mapping[str, str]) -> None: ...


class RateLimitedError(RuntimeError):
    """GGG a répondu 429 ou signale une restriction active."""

    def __init__(self, key: str, retry_after: float) -> None:
        super().__init__(f"file {key!r} bloquée par GGG pendant {retry_after:.0f} s")
        self.key = key
        self.retry_after = retry_after


@dataclass(frozen=True)
class Rule:
    max_hits: int
    period: float
    penalty: float


@dataclass(frozen=True)
class RuleState:
    hits: int
    period: float
    active_restriction: float


def _parse_triplets(value: str) -> list[tuple[float, float, float]]:
    out: list[tuple[float, float, float]] = []
    for chunk in value.split(","):
        parts = chunk.strip().split(":")
        if len(parts) != 3:
            continue
        try:
            out.append((float(parts[0]), float(parts[1]), float(parts[2])))
        except ValueError:
            continue
    return out


def parse_rate_limit_headers(
    headers: Mapping[str, str],
) -> tuple[str | None, dict[str, list[Rule]], dict[str, list[RuleState]]]:
    """Extrait politique, règles et états des en-têtes `X-Rate-Limit-*`."""
    h = {k.lower(): v for k, v in headers.items()}
    policy = h.get("x-rate-limit-policy")
    rules: dict[str, list[Rule]] = {}
    states: dict[str, list[RuleState]] = {}
    for name in (r.strip() for r in h.get("x-rate-limit-rules", "").split(",")):
        if not name:
            continue
        raw_rule = h.get(f"x-rate-limit-{name.lower()}")
        raw_state = h.get(f"x-rate-limit-{name.lower()}-state")
        if raw_rule:
            rules[name] = [Rule(int(a), b, c) for a, b, c in _parse_triplets(raw_rule)]
        if raw_state:
            states[name] = [RuleState(int(a), b, c) for a, b, c in _parse_triplets(raw_state)]
    return policy, rules, states


@dataclass
class _Queue:
    policy: str | None = None
    rules: list[Rule] = field(default_factory=list)
    sent: deque[float] = field(default_factory=deque)
    blocked_until: float = 0.0
    in_flight_probe: bool = False


class HeaderDrivenBudget:
    """Implémentation locale du `BudgetManager`.

    - Tant qu'aucune réponse n'a révélé la politique d'une file, une seule
      requête « sonde » est autorisée.
    - Ensuite, chaque règle (max, période) est respectée sur une fenêtre
      glissante, avec une marge de sécurité (`margin` hits en moins).
    - L'état renvoyé par le serveur prime : si GGG compte plus de hits que
      nous (autre process, autre machine sur la même IP), on s'aligne.
    - 429, `Retry-After` ou restriction active bloquent la file.
    """

    def __init__(
        self,
        *,
        margin: int = 1,
        clock: Callable[[], float] = time.monotonic,
        sleep: Callable[[float], None] = time.sleep,
        max_wait: float = 900.0,
    ) -> None:
        self._margin = margin
        self._clock = clock
        self._sleep = sleep
        self._max_wait = max_wait
        self._queues: dict[str, _Queue] = {}
        self._lock = threading.Lock()

    def _q(self, key: str) -> _Queue:
        return self._queues.setdefault(key, _Queue())

    def wait_time(self, key: str) -> float:
        """Secondes à attendre avant la prochaine requête sur `key` (0 si libre)."""
        with self._lock:
            return self._wait_time_locked(self._q(key), self._clock())

    def _wait_time_locked(self, q: _Queue, now: float) -> float:
        wait = max(0.0, q.blocked_until - now)
        if not q.rules:
            # Politique inconnue : une seule sonde à la fois.
            return wait if not q.in_flight_probe else max(wait, 1.0)
        for rule in q.rules:
            allowed = max(1, rule.max_hits - self._margin)
            recent = [t for t in q.sent if t > now - rule.period]
            if len(recent) >= allowed:
                oldest_blocking = sorted(recent)[len(recent) - allowed]
                wait = max(wait, oldest_blocking + rule.period - now)
        return wait

    def acquire(self, key: str, priority: int = PRIORITY_INGESTION) -> None:
        # Un seul consommateur par process ici : la priorité est portée par le
        # contrat et honorée par le budget manager central.
        del priority
        while True:
            with self._lock:
                q = self._q(key)
                now = self._clock()
                wait = self._wait_time_locked(q, now)
                if wait <= 0:
                    q.sent.append(now)
                    if not q.rules:
                        q.in_flight_probe = True
                    longest = max((r.period for r in q.rules), default=3600.0)
                    while q.sent and q.sent[0] <= now - longest:
                        q.sent.popleft()
                    return
            if wait > self._max_wait:
                raise RateLimitedError(key, wait)
            self._sleep(wait)

    def observe(self, key: str, status: int, headers: Mapping[str, str]) -> None:
        policy, rules, states = parse_rate_limit_headers(headers)
        with self._lock:
            q = self._q(key)
            now = self._clock()
            q.in_flight_probe = False
            if policy:
                q.policy = policy
            if rules:
                q.rules = [r for rs in rules.values() for r in rs]
            # Alignement sur l'état vu par le serveur.
            for rs in states.values():
                for st in rs:
                    if st.active_restriction > 0:
                        q.blocked_until = max(q.blocked_until, now + st.active_restriction)
                    ours = sum(1 for t in q.sent if t > now - st.period)
                    for _ in range(max(0, st.hits - ours)):
                        q.sent.append(now)
            if status == 429:
                retry = _retry_after(headers)
                q.blocked_until = max(q.blocked_until, now + retry)

    def snapshot(self) -> dict[str, dict[str, object]]:
        """État lisible, pour les logs et le futur tableau de bord des budgets."""
        with self._lock:
            now = self._clock()
            return {
                key: {
                    "policy": q.policy,
                    "rules": [(r.max_hits, r.period) for r in q.rules],
                    "blocked_for_s": max(0.0, q.blocked_until - now),
                    "wait_s": self._wait_time_locked(q, now),
                }
                for key, q in self._queues.items()
            }


def _retry_after(headers: Mapping[str, str]) -> float:
    for k, v in headers.items():
        if k.lower() == "retry-after":
            try:
                return float(v)
            except ValueError:
                break
    # Sans Retry-After exploitable, on prend la pénalité la plus prudente.
    _, rules, _ = parse_rate_limit_headers(headers)
    penalties = [r.penalty for rs in rules.values() for r in rs]
    return max(penalties, default=60.0)
