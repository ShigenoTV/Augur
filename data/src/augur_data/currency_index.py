"""Indice de prix des devises (plan, S3 Data), exprimé en exalts.

Méthode :
1. chaque observation de paire est vue dans les deux sens (A→B et B→A) ;
2. par devise, exclusion des valeurs aberrantes (z-score modifié sur le log
   du taux, seuil 3,5) ;
3. médiane pondérée par le volume et par la fraîcheur (demi-vie réglable) ;
4. les devises sans paire directe avec l'exalt sont triangulées via une
   devise pivot (divine, puis chaos), et marquées comme telles.
"""

from __future__ import annotations

import math
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime
from typing import Any

from .robust import mad_inliers, recency_weight, weighted_median, weighted_quantile

NUMERAIRE = "exalted"
PIVOTS = ("divine", "chaos")


@dataclass(frozen=True)
class Rate:
    currency: str
    value: float  # en exalts
    low: float  # quantile pondéré 25 %
    high: float  # quantile pondéré 75 %
    observations: int
    excluded: int
    method: str  # direct | triangulated | numeraire
    updated_at: str | None


def _directed(obs: list[dict[str, Any]]) -> dict[tuple[str, str], list[tuple[float, float, str]]]:
    out: dict[tuple[str, str], list[tuple[float, float, str]]] = defaultdict(list)
    for o in obs:
        r, w = float(o["rate"]), float(o.get("weight") or 1.0)
        if r <= 0 or not math.isfinite(r):
            continue
        out[(o["base"], o["quote"])].append((r, w, o["observed_at"]))
        # Le poids reste le volume côté base, converti en unités de la nouvelle base.
        out[(o["quote"], o["base"])].append((1 / r, w * r, o["observed_at"]))
    return out


def _robust_rate(
    rows: list[tuple[float, float, str]], now: datetime, half_life_hours: float
) -> tuple[float, float, float, int, int, str]:
    vals = [r for r, _, _ in rows]
    keep = mad_inliers(vals) if len(vals) >= 3 else [True] * len(vals)
    kept = [row for row, k in zip(rows, keep, strict=True) if k]
    weights = []
    for _, w, ts in kept:
        age_h = (now - datetime.fromisoformat(ts)).total_seconds() / 3600
        weights.append(w * recency_weight(age_h, half_life_hours))
    v = [r for r, _, _ in kept]
    return (
        weighted_median(v, weights),
        weighted_quantile(v, weights, 0.25),
        weighted_quantile(v, weights, 0.75),
        len(kept),
        len(rows) - len(kept),
        max(ts for _, _, ts in kept),
    )


def build_rates(
    observations: list[dict[str, Any]],
    *,
    league: str,
    now: datetime,
    half_life_hours: float = 6.0,
) -> dict[str, Rate]:
    obs = [o for o in observations if o.get("league") == league]
    pairs = _directed(obs)
    rates: dict[str, Rate] = {NUMERAIRE: Rate(NUMERAIRE, 1.0, 1.0, 1.0, 0, 0, "numeraire", None)}
    for (base, quote), rows in pairs.items():
        if quote != NUMERAIRE or base == NUMERAIRE:
            continue
        v, lo, hi, n, ex, ts = _robust_rate(rows, now, half_life_hours)
        rates[base] = Rate(base, v, lo, hi, n, ex, "direct", ts)
    currencies = {c for pair in pairs for c in pair} - set(rates)
    for c in sorted(currencies):
        for pivot in PIVOTS:
            if pivot in rates and (c, pivot) in pairs:
                v, lo, hi, n, ex, ts = _robust_rate(pairs[(c, pivot)], now, half_life_hours)
                p = rates[pivot]
                rates[c] = Rate(c, v * p.value, lo * p.low, hi * p.high, n, ex, f"triangulated:{pivot}", ts)
                break
    return rates
