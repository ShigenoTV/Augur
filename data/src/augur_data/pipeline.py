"""Chaîne complète, du stockage brut au document servi à l'application.

Fonctions pures autour du `RawStore`, appelées telles quelles par la CLI et
par les assets Dagster (`augur_data.dagster_defs`).
"""

from __future__ import annotations

from datetime import datetime
from typing import Any

import polars as pl

from .cleaning import clean_listings, quality_report
from .contract import build_payload
from .currency_index import Rate, build_rates
from .item_index import build_item_prices
from .sources import currency_exchange, trade_sampler
from .storage import RawStore, read_snapshots, slug


def load_currency_observations(store: RawStore, realm: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for snap in read_snapshots(store, f"raw/{currency_exchange.SOURCE}/{realm}/"):
        rows.extend(currency_exchange.parse(snap.envelope))
    return rows


def load_trade_listings(store: RawStore, realm: str, league_slug: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for snap in read_snapshots(store, f"raw/{trade_sampler.SOURCE}/{realm}/{league_slug}/"):
        rows.extend(trade_sampler.parse(snap.envelope))
    return rows


def build_prices(
    store: RawStore,
    *,
    league: str,
    realm: str = "poe2",
    now: datetime,
    names: dict[str, str] | None = None,
) -> tuple[dict[str, Any], pl.DataFrame, dict[str, Rate]]:

    rates = build_rates(load_currency_observations(store, realm), league=league, now=now)
    cleaned = clean_listings(
        load_trade_listings(store, realm, slug(league)),
        {c: r.value for c, r in rates.items()},
    )
    items = build_item_prices(cleaned, now=now)
    quality = {
        "currencies": len(rates),
        "currencies_triangulated": sum(r.method.startswith("triangulated") for r in rates.values()),
        "currency_observations_excluded": sum(r.excluded for r in rates.values()),
        "trade": quality_report(cleaned),
    }
    payload = build_payload(
        league=league,
        realm=realm,
        rates=rates,
        items=items,
        names=names or {},
        quality=quality,
        now=now,
    )
    return payload, cleaned, rates
