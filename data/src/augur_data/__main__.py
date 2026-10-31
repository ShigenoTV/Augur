"""CLI : `python -m augur_data {collect-cx,collect-trade,build}`.

Les identifiants du studio viennent de l'environnement (secrets de
déploiement) : AUGUR_GGG_CLIENT_ID, AUGUR_GGG_CONTACT, AUGUR_GGG_TOKEN.
"""

from __future__ import annotations

import argparse
import json
import logging
import os
import sys
from datetime import UTC, datetime
from pathlib import Path

from .ggg import ClientIdentity, GGGClient, KillSwitchError
from .pipeline import build_prices
from .ratelimit import HeaderDrivenBudget, RateLimitedError
from .sources import currency_exchange, trade_sampler
from .storage import LocalRawStore

log = logging.getLogger("augur_data")


def _client() -> GGGClient:
    return GGGClient(
        ClientIdentity.from_env(),
        HeaderDrivenBudget(),
        access_token=os.environ.get("AUGUR_GGG_TOKEN"),
    )


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="augur_data")
    p.add_argument("--store", type=Path, default=Path("var"), help="racine du stockage brut local")
    p.add_argument("--realm", default="poe2")
    sub = p.add_subparsers(dest="cmd", required=True)

    cx = sub.add_parser("collect-cx", help="une heure d'agrégats de l'échange de devises")
    cx.add_argument("--change-id", type=int)

    tr = sub.add_parser("collect-trade", help="échantillons de trade sur les requêtes curées")
    tr.add_argument("--league", required=True)
    tr.add_argument("--queries", type=Path, default=Path("queries/poe2-s1.json"))

    b = sub.add_parser("build", help="construit le document GET /prices/{league}")
    b.add_argument("--league", required=True)
    b.add_argument("--queries", type=Path, default=Path("queries/poe2-s1.json"))
    b.add_argument("--out", type=Path, required=True)

    args = p.parse_args(argv)
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    store = LocalRawStore(args.store)

    try:
        if args.cmd == "collect-cx":
            with _client() as c:
                snap = currency_exchange.collect(c, store, realm=args.realm, change_id=args.change_id)
            log.info("snapshot écrit : %s", snap.key)
        elif args.cmd == "collect-trade":
            queries = trade_sampler.load_queries(args.queries)
            with _client() as c:
                for q in queries:
                    snap = trade_sampler.collect_one(c, store, q, league=args.league, realm=args.realm)
                    log.info("%s → %s", q.id, snap.key)
        else:
            names = {q.id: q.name for q in trade_sampler.load_queries(args.queries)}
            payload, _, _ = build_prices(
                store, league=args.league, realm=args.realm, now=datetime.now(UTC), names=names
            )
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
            log.info("%d prix écrits dans %s", len(payload["items"]), args.out)
    except KillSwitchError as e:
        log.warning("%s", e)
        return 3
    except RateLimitedError as e:
        # On n'insiste jamais : la prochaine exécution planifiée reprendra.
        log.error("%s ; arrêt de la collecte", e)
        return 4
    return 0


if __name__ == "__main__":
    sys.exit(main())
