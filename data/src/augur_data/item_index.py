"""Prix de référence des requêtes curées (fragments, bases…) à partir des annonces nettoyées.

On garde la meilleure annonce de chaque vendeur, puis on prend la médiane
pondérée par la fraîcheur de l'annonce. Le carnet est lu trié par prix
croissant : cette médiane représente le prix auquel on peut réellement
acheter, pas un prix demandé moyen.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime

import polars as pl

from .robust import recency_weight, weighted_median, weighted_quantile


@dataclass(frozen=True)
class ItemPrice:
    league: str
    query_id: str
    category: str
    value: float
    low: float
    high: float
    sellers: int
    updated_at: str | None


def confidence(sample: int, low: float, high: float) -> str:
    spread = high / low if low > 0 else float("inf")
    if sample >= 8 and spread <= 1.5:
        return "high"
    if sample >= 4 and spread <= 3:
        return "medium"
    return "low"


def build_item_prices(
    cleaned: pl.DataFrame, *, now: datetime, half_life_hours: float = 24.0
) -> list[ItemPrice]:
    kept = cleaned.filter(pl.col("exclusion").is_null())
    if kept.is_empty():
        return []
    best = (
        kept.with_columns(pl.coalesce("seller_id", "listing_id").alias("_seller"))
        .sort("unit_price_ex")
        .unique(subset=["league", "query_id", "_seller"], keep="first", maintain_order=True)
    )
    out: list[ItemPrice] = []
    for (league, query_id), grp in best.group_by("league", "query_id", maintain_order=True):
        prices = grp["unit_price_ex"].to_list()
        stamps = [
            s or o for s, o in zip(grp["indexed_at"].to_list(), grp["observed_at"].to_list(), strict=True)
        ]
        weights = [
            recency_weight((now - datetime.fromisoformat(ts)).total_seconds() / 3600, half_life_hours)
            if ts
            else 0.5
            for ts in stamps
        ]
        out.append(
            ItemPrice(
                league=str(league),
                query_id=str(query_id),
                category=grp["category"][0],
                value=weighted_median(prices, weights),
                low=weighted_quantile(prices, weights, 0.25),
                high=weighted_quantile(prices, weights, 0.75),
                sellers=grp.height,
                updated_at=max((t for t in stamps if t), default=None),
            )
        )
    return out
