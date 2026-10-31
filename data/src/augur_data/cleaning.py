"""Nettoyage des annonces de trade (plan, S2 Data).

Entrée : lignes produites par `sources.trade_sampler.parse`.
Sortie : une ligne par annonce, prix unitaire normalisé en exalts, et une
colonne `exclusion` qui dit pourquoi une annonce est écartée de l'indice
(null si elle est retenue). Rien n'est supprimé : on marque, pour pouvoir
auditer le nettoyage et en mesurer le taux dans le rapport de qualité.

Motifs d'exclusion :
- `prix_absent` : annonce sans prix exploitable ;
- `devise_inconnue` : devise sans taux connu dans l'indice des devises ;
- `doublon` : même annonce vue plusieurs fois pour une requête, on garde l'observation la plus récente ;
- `reannonce` : même vendeur, même requête, même prix unitaire (spam de réannonces) ;
- `leurre_bas` : prix anormalement bas, typique des annonces leurres
  (« price fixing ») qui attirent les whispers et faussent les estimations ;
- `aberrant_haut` : prix anormalement haut (annonce d'ancrage ou erreur) ;
- `mur_vendeur` : un même vendeur occupe la majorité du haut du carnet ; on ne
  garde que sa meilleure annonce pour qu'il ne fixe pas le prix à lui seul.

Les noms de comptes vendeurs sont remplacés par une empreinte : le pipeline
n'a besoin que de savoir si deux annonces viennent du même vendeur.
"""

from __future__ import annotations

import hashlib
import math
from typing import Any

import polars as pl

from .robust import MAD_Z_THRESHOLD, median

MIN_SELLERS_FOR_OUTLIERS = 5
SELLER_WALL_SHARE = 0.5
SELLER_WALL_MIN_LISTINGS = 6

SCHEMA = {
    "source": pl.String,
    "league": pl.String,
    "query_id": pl.String,
    "category": pl.String,
    "listing_id": pl.String,
    "indexed_at": pl.String,
    "observed_at": pl.String,
    "seller": pl.String,
    "price_type": pl.String,
    "amount": pl.Float64,
    "currency": pl.String,
    "stack_size": pl.Int64,
    "snapshot_key": pl.String,
}


def seller_fingerprint(name: str | None) -> str | None:
    if not name:
        return None
    return hashlib.sha256(name.encode()).hexdigest()[:16]


def _flag_group(prices: list[float], sellers: list[str | None]) -> list[str | None]:
    """Motifs d'exclusion statistiques au sein d'une requête curée."""
    n = len(prices)
    out: list[str | None] = [None] * n
    distinct = {s for s in sellers if s}
    # Mur de vendeur : on ne garde que la meilleure annonce du vendeur dominant.
    if n >= SELLER_WALL_MIN_LISTINGS:
        counts: dict[str, int] = {}
        for s in sellers:
            if s:
                counts[s] = counts.get(s, 0) + 1
        for s, c in counts.items():
            if c / n >= SELLER_WALL_SHARE:
                idx = sorted((i for i in range(n) if sellers[i] == s), key=lambda i: prices[i])
                for i in idx[1:]:
                    out[i] = "mur_vendeur"
    if len(distinct) < MIN_SELLERS_FOR_OUTLIERS:
        return out
    # Les z-scores se calculent sur une annonce par vendeur (sa meilleure),
    # pour qu'un vendeur ne puisse pas déplacer la référence.
    best: dict[str, int] = {}
    for i, s in enumerate(sellers):
        key = s or f"_anon{i}"
        if key not in best or prices[i] < prices[best[key]]:
            best[key] = i
    ref_logs = [math.log(prices[i]) for i in best.values()]
    med = median(ref_logs)
    mad_v = median([abs(x - med) for x in ref_logs])
    if mad_v == 0:
        # Carnet très serré : on tolère ±50 % autour de la médiane.
        for i, p in enumerate(prices):
            if out[i] is None and abs(math.log(p) - med) > math.log(1.5):
                out[i] = "leurre_bas" if math.log(p) < med else "aberrant_haut"
        return out
    for i, p in enumerate(prices):
        if out[i] is not None:
            continue
        z = 0.6745 * (math.log(p) - med) / mad_v
        if z < -MAD_Z_THRESHOLD:
            out[i] = "leurre_bas"
        elif z > MAD_Z_THRESHOLD:
            out[i] = "aberrant_haut"
    return out


def clean_listings(rows: list[dict[str, Any]], rates_ex: dict[str, float]) -> pl.DataFrame:
    """Nettoie et normalise les annonces. `rates_ex` : valeur d'une unité de devise en exalts."""
    df = pl.DataFrame(rows, schema=SCHEMA, strict=False)
    if df.is_empty():
        return df.with_columns(
            pl.lit(None, pl.String).alias("seller_id"),
            pl.lit(None, pl.Float64).alias("unit_price_ex"),
            pl.lit(None, pl.String).alias("exclusion"),
        ).drop("seller")
    rates = pl.DataFrame(
        {"currency": list(rates_ex), "rate_ex": list(rates_ex.values())},
        schema={"currency": pl.String, "rate_ex": pl.Float64},
    )
    df = (
        df.with_columns(
            pl.col("seller").map_elements(seller_fingerprint, return_dtype=pl.String).alias("seller_id"),
            pl.col("stack_size").fill_null(1).clip(lower_bound=1),
        )
        .drop("seller")
        .join(rates, on="currency", how="left")
        .with_columns(
            (pl.col("amount") * pl.col("rate_ex") / pl.col("stack_size")).alias("unit_price_ex"),
        )
        .with_columns(
            pl.when(pl.col("amount").is_null() | (pl.col("amount") <= 0) | pl.col("currency").is_null())
            .then(pl.lit("prix_absent"))
            .when(pl.col("rate_ex").is_null())
            .then(pl.lit("devise_inconnue"))
            .otherwise(pl.lit(None, pl.String))
            .alias("exclusion")
        )
        .drop("rate_ex")
    )
    # Doublons : même annonce vue dans plusieurs snapshots.
    df = df.sort("observed_at", descending=True, nulls_last=True).with_columns(
        pl.when(
            pl.col("exclusion").is_null()
            & (pl.col("listing_id").cum_count().over("league", "query_id", "listing_id") > 1)
        )
        .then(pl.lit("doublon"))
        .otherwise(pl.col("exclusion"))
        .alias("exclusion")
    )
    # Réannonces : même vendeur, même requête, même prix unitaire.
    df = df.with_columns(
        pl.when(
            pl.col("exclusion").is_null()
            & pl.col("seller_id").is_not_null()
            & (
                pl.col("listing_id")
                .cum_count()
                .over(
                    "league",
                    "query_id",
                    "seller_id",
                    pl.col("unit_price_ex").round(6),
                    pl.col("exclusion").is_null(),
                )
                > 1
            )
        )
        .then(pl.lit("reannonce"))
        .otherwise(pl.col("exclusion"))
        .alias("exclusion")
    )
    # Leurres, aberrants et murs de vendeurs, par ligue et requête.
    df = df.with_row_index("_row")
    kept = df.filter(pl.col("exclusion").is_null())
    flags: dict[int, str] = {}
    for _, grp in kept.group_by("league", "query_id"):
        rows_idx = grp["_row"].to_list()
        res = _flag_group(grp["unit_price_ex"].to_list(), grp["seller_id"].to_list())
        flags.update({r: f for r, f in zip(rows_idx, res, strict=True) if f})
    if flags:
        flag_df = pl.DataFrame(
            {"_row": list(flags), "_flag": list(flags.values())},
            schema={"_row": pl.UInt32, "_flag": pl.String},
        )
        df = (
            df.join(flag_df, on="_row", how="left")
            .with_columns(pl.coalesce("exclusion", "_flag").alias("exclusion"))
            .drop("_flag")
        )
    return df.drop("_row").sort("league", "query_id", "unit_price_ex", nulls_last=True)


def quality_report(cleaned: pl.DataFrame) -> dict[str, Any]:
    """Indicateurs du tableau de bord de qualité des données."""
    total = cleaned.height
    by_reason = (
        cleaned.group_by("exclusion").len().sort("exclusion", nulls_last=True).to_dicts() if total else []
    )
    reasons = {(r["exclusion"] or "retenue"): r["len"] for r in by_reason}
    return {
        "listings": total,
        "kept": reasons.get("retenue", 0),
        "excluded_by_reason": {k: v for k, v in reasons.items() if k != "retenue"},
        "exclusion_rate": round(1 - reasons.get("retenue", 0) / total, 4) if total else None,
    }


__all__ = ["clean_listings", "quality_report", "seller_fingerprint"]
