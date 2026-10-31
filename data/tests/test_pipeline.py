import json
from pathlib import Path

import jsonschema

from augur_data.contract import build_payload
from augur_data.pipeline import build_prices
from augur_data.sources import currency_exchange, trade_sampler
from augur_data.storage import LocalRawStore
from conftest import LEAGUE, NOW

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads((ROOT / "contracts" / "prices.v1.schema.json").read_text())
QUERIES = ROOT / "data" / "queries" / "poe2-s1.json"


def collect_all(client, store):
    currency_exchange.collect(client, store)
    queries = trade_sampler.load_queries(QUERIES)
    for q in queries:
        trade_sampler.collect_one(client, store, q, league=LEAGUE)
    return queries


def test_end_to_end_payload_matches_contract(client, fake_ggg, tmp_path):
    store = LocalRawStore(tmp_path)
    queries = collect_all(client, store)
    # 1 cx + par requête : 1 search + 2 fetch (20 résultats max, lots de 10)
    assert len(fake_ggg.requests) == 1 + 3 * len(queries)
    payload, cleaned, rates = build_prices(
        store, league=LEAGUE, now=NOW, names={q.id: q.name for q in queries}
    )
    jsonschema.validate(payload, SCHEMA)
    ids = {i["id"] for i in payload["items"]}
    assert {"currency/exalted", "currency/divine", "currency/annul", "fragment/breach-splinter"} <= ids
    assert payload["quality"]["trade"]["excluded_by_reason"]["leurre_bas"] == len(queries)
    breach = next(i for i in payload["items"] if i["id"] == "fragment/breach-splinter")
    assert breach["name"] == "Breach Splinter" and breach["confidence"] in {"high", "medium"}


def test_etag_ignores_generation_time(client, tmp_path):
    from datetime import timedelta

    store = LocalRawStore(tmp_path)
    collect_all(client, store)
    p1, _, _ = build_prices(store, league=LEAGUE, now=NOW)
    p2, _, _ = build_prices(store, league=LEAGUE, now=NOW + timedelta(seconds=1))
    assert p1["generated_at"] != p2["generated_at"]
    assert p1["etag"] == p2["etag"]


def test_empty_league_still_valid():
    payload = build_payload(league="Vide", realm="poe2", rates={}, items=[], names={}, quality={}, now=NOW)
    jsonschema.validate(payload, SCHEMA)
    assert payload["data_as_of"] is None
