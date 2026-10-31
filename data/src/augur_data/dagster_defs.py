"""Orchestration Dagster (plan, section 1.6) : assets versionnés par ligue.

Dépendance optionnelle (`pip install -r requirements-pipelines.txt`).
Lancement local : `dagster dev -m augur_data.dagster_defs`.

Configuration par variables d'environnement, fournies par le déploiement :
AUGUR_LEAGUES (ligues séparées par des virgules), AUGUR_RAW_ROOT,
AUGUR_PRICES_OUT, AUGUR_QUERIES, plus l'identité OAuth lue par `ggg.py`.

Cadence : l'échange de devises publie des agrégats horaires, on les relève
une fois par heure ; l'échantillonnage du trade tourne toutes les 30 minutes
et reste plafonné par le budget manager quoi qu'il arrive.
"""

import json
import os
from datetime import UTC, datetime
from pathlib import Path

import dagster as dg

from .ggg import ClientIdentity, GGGClient
from .pipeline import build_prices
from .ratelimit import HeaderDrivenBudget
from .sources import currency_exchange, trade_sampler
from .storage import LocalRawStore, slug

LEAGUES = [x.strip() for x in os.environ.get("AUGUR_LEAGUES", "Standard").split(",") if x.strip()]
league_partitions = dg.StaticPartitionsDefinition([slug(x) for x in LEAGUES])
_BY_SLUG = {slug(x): x for x in LEAGUES}


def _store() -> LocalRawStore:
    return LocalRawStore(Path(os.environ.get("AUGUR_RAW_ROOT", "var")))


def _queries() -> list[trade_sampler.CuratedQuery]:
    return trade_sampler.load_queries(Path(os.environ.get("AUGUR_QUERIES", "queries/poe2-s1.json")))


def _client() -> GGGClient:
    return GGGClient(
        ClientIdentity.from_env(), HeaderDrivenBudget(), access_token=os.environ.get("AUGUR_GGG_TOKEN")
    )


@dg.asset(group_name="ingestion", description="Agrégats horaires de l'échange de devises (brut).")
def raw_currency_exchange(context: dg.AssetExecutionContext) -> dg.MaterializeResult:
    with _client() as c:
        snap = currency_exchange.collect(c, _store())
    return dg.MaterializeResult(metadata={"snapshot": snap.key, "sha256": snap.sha256})


@dg.asset(
    group_name="ingestion",
    partitions_def=league_partitions,
    description="Échantillons de trade sur les requêtes curées (brut).",
)
def raw_trade_samples(context: dg.AssetExecutionContext) -> dg.MaterializeResult:
    league = _BY_SLUG[context.partition_key]
    keys = []
    with _client() as c:
        for q in _queries():
            keys.append(trade_sampler.collect_one(c, _store(), q, league=league).key)
    return dg.MaterializeResult(metadata={"snapshots": len(keys)})


@dg.asset(
    group_name="prix",
    partitions_def=league_partitions,
    deps=[raw_currency_exchange, raw_trade_samples],
    description="Document servi par GET /prices/{league} (contrat v1).",
)
def prices_document(context: dg.AssetExecutionContext) -> dg.MaterializeResult:
    league = _BY_SLUG[context.partition_key]
    names = {q.id: q.name for q in _queries()}
    payload, _, _ = build_prices(_store(), league=league, now=datetime.now(UTC), names=names)
    out = Path(os.environ.get("AUGUR_PRICES_OUT", "var/prices")) / f"{context.partition_key}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    trade = payload["quality"]["trade"]
    return dg.MaterializeResult(
        metadata={
            "items": len(payload["items"]),
            "etag": payload["etag"],
            "exclusion_rate": trade["exclusion_rate"],
            "excluded_by_reason": dg.MetadataValue.json(trade["excluded_by_reason"]),
        }
    )


cx_job = dg.define_asset_job("collecte_devises", selection=[raw_currency_exchange])
prices_job = dg.define_asset_job("collecte_trade_et_prix", selection=[raw_trade_samples, prices_document])


@dg.schedule(job=prices_job, cron_schedule="*/30 * * * *")
def trade_schedule(context: dg.ScheduleEvaluationContext):
    for key in league_partitions.get_partition_keys():
        yield dg.RunRequest(run_key=f"{key}-{context.scheduled_execution_time:%Y%m%d%H%M}", partition_key=key)


defs = dg.Definitions(
    assets=[raw_currency_exchange, raw_trade_samples, prices_document],
    jobs=[cx_job, prices_job],
    schedules=[dg.ScheduleDefinition(job=cx_job, cron_schedule="7 * * * *"), trade_schedule],
)
