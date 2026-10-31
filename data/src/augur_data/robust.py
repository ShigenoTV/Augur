"""Statistiques robustes : médiane et quantiles pondérés, exclusion par MAD."""

from __future__ import annotations

import math
from collections.abc import Sequence

# Seuil classique d'Iglewicz et Hoaglin pour le z-score modifié.
MAD_Z_THRESHOLD = 3.5


def weighted_quantile(values: Sequence[float], weights: Sequence[float], q: float) -> float:
    """Quantile pondéré (interpolation sur les poids cumulés centrés)."""
    if not values:
        raise ValueError("aucune valeur")
    pairs = sorted((v, w) for v, w in zip(values, weights, strict=True) if w > 0)
    if not pairs:
        raise ValueError("aucun poids positif")
    total = sum(w for _, w in pairs)
    cum, centers = 0.0, []
    for _, w in pairs:
        centers.append((cum + w / 2) / total)
        cum += w
    if q <= centers[0]:
        return pairs[0][0]
    if q >= centers[-1]:
        return pairs[-1][0]
    for i in range(1, len(pairs)):
        if q <= centers[i]:
            lo, hi = centers[i - 1], centers[i]
            t = (q - lo) / (hi - lo)
            return pairs[i - 1][0] + t * (pairs[i][0] - pairs[i - 1][0])
    return pairs[-1][0]


def weighted_median(values: Sequence[float], weights: Sequence[float]) -> float:
    return weighted_quantile(values, weights, 0.5)


def median(values: Sequence[float]) -> float:
    return weighted_median(values, [1.0] * len(values))


def modified_z_scores(values: Sequence[float]) -> list[float]:
    """z-score modifié sur les valeurs fournies (0 partout si MAD nulle)."""
    if not values:
        return []
    med = median(values)
    mad = median([abs(v - med) for v in values])
    if mad == 0:
        return [0.0] * len(values)
    return [0.6745 * (v - med) / mad for v in values]


def mad_inliers(values: Sequence[float], threshold: float = MAD_Z_THRESHOLD) -> list[bool]:
    """Masque des valeurs conservées, calculé sur le log des prix (prix > 0)."""
    logs = [math.log(v) for v in values]
    return [abs(z) <= threshold for z in modified_z_scores(logs)]


def recency_weight(age_hours: float, half_life_hours: float) -> float:
    return 0.5 ** (max(0.0, age_hours) / half_life_hours)
