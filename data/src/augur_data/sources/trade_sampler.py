"""Source 2 : échantillonnage du site de trade sur une liste curée de requêtes.

Plan fondateur (S1 Data) : collecteur d'échantillons de trade sur une liste
curée (devises, fragments, bases recherchées), en passant par le budget
manager, stockage brut sans retraitement.

Règles de conduite :
- une requête curée = 1 search + au plus ceil(max_results / 10) fetch ;
- tri par prix croissant, on ne lit que le haut du carnet (les annonces les
  moins chères sont celles qui font le prix) ;
- aucune requête n'est lancée en dehors du budget manager, et le collecteur
  s'arrête net au premier 429 au lieu d'insister ;
- les recherches « utilisateur » ne passent jamais par ici : elles partent du
  poste du joueur (plan, section 0 point 2).
"""

from __future__ import annotations

import json
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import quote

from ..ggg import WWW_BASE, GGGClient
from ..storage import RawStore, Snapshot, make_envelope, write_snapshot

SOURCE = "trade_sample"
FETCH_BATCH = 10  # limite du endpoint fetch


@dataclass(frozen=True)
class CuratedQuery:
    id: str
    category: str
    name: str
    query: dict[str, Any]
    max_results: int = 20


def load_queries(path: Path) -> list[CuratedQuery]:
    raw = json.loads(Path(path).read_text(encoding="utf-8"))
    out = [CuratedQuery(**q) for q in raw["queries"]]
    ids = [q.id for q in out]
    if len(ids) != len(set(ids)):
        raise ValueError("identifiants de requêtes curées en double")
    for q in out:
        if not 1 <= q.max_results <= 100:
            raise ValueError(f"{q.id}: max_results doit rester entre 1 et 100")
    return out


def _chunks(seq: list[str], n: int) -> Iterator[list[str]]:
    for i in range(0, len(seq), n):
        yield seq[i : i + n]


def collect_one(
    client: GGGClient, store: RawStore, q: CuratedQuery, *, league: str, realm: str = "poe2"
) -> Snapshot:
    league_path = quote(league, safe="")
    body = {"query": q.query, "sort": {"price": "asc"}}
    search = client.request(
        "POST",
        f"{WWW_BASE}/api/trade2/search/{realm}/{league_path}",
        budget_key="trade-search",
        json=body,
    )
    ids = list((search.body or {}).get("result") or [])[: q.max_results]
    search_id = (search.body or {}).get("id")
    fetched: list[dict[str, Any]] = []
    for batch in _chunks(ids, FETCH_BATCH):
        resp = client.request(
            "GET",
            f"{WWW_BASE}/api/trade2/fetch/{','.join(batch)}",
            budget_key="trade-fetch",
            params={"query": search_id} if search_id else None,
        )
        fetched.extend((resp.body or {}).get("result") or [])
    env = make_envelope(
        source=SOURCE,
        realm=realm,
        league=league,
        request={"query_id": q.id, "category": q.category, "name": q.name, "search": body},
        status=search.status,
        headers=search.headers,
        body={"total": (search.body or {}).get("total"), "search_id": search_id, "result": fetched},
    )
    return write_snapshot(store, env)


def parse(envelope: dict[str, Any]) -> list[dict[str, Any]]:
    """Aplati un snapshot en annonces (une ligne par annonce, prix non converti)."""
    req = envelope["request"]
    out: list[dict[str, Any]] = []
    for r in (envelope["response"]["body"] or {}).get("result") or []:
        if not r:
            continue
        listing = r.get("listing") or {}
        price = listing.get("price") or {}
        item = r.get("item") or {}
        out.append(
            {
                "source": SOURCE,
                "league": envelope["league"],
                "query_id": req["query_id"],
                "category": req["category"],
                "listing_id": r.get("id"),
                "indexed_at": listing.get("indexed"),
                "observed_at": envelope["fetched_at"],
                "seller": (listing.get("account") or {}).get("name"),
                "price_type": price.get("type"),
                "amount": price.get("amount"),
                "currency": price.get("currency"),
                "stack_size": item.get("stackSize") or 1,
                "snapshot_key": envelope.get("_key"),
            }
        )
    return out
