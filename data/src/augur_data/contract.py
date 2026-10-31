"""Format des prix servis à l'application : `GET /prices/{league}`.

Le schéma JSON de référence vit dans `contracts/prices.v1.schema.json` à la
racine du dépôt : c'est le contrat partagé avec l'API axum (chantier back) et
l'overlay (chantier front). Toute évolution incompatible crée un v2.

Choix de format :
- tous les prix sont en **exalts** (`unit`), la devise de référence de PoE 2 ;
- chaque prix porte sa fourchette (`low`/`high`, quantiles 25-75 %), sa taille
  d'échantillon et un niveau de confiance, pour que l'overlay affiche
  « approximatif » plutôt qu'un chiffre faussement précis ;
- `fresh_until` et `frozen` permettent le mode dégradé du plan (section 1.7) :
  un prix ancien reste affiché, avec son horodatage, plutôt qu'une erreur ;
- l'`etag` ne dépend que du contenu utile, pas de l'heure de génération, pour
  que le CDN et le client ne retéléchargent rien quand les prix n'ont pas bougé.
"""

from __future__ import annotations

import hashlib
import json
from datetime import datetime, timedelta
from typing import Any

from .currency_index import Rate
from .item_index import ItemPrice, confidence

SCHEMA_VERSION = 1
CURRENCY_NAMES = {
    "exalted": "Exalted Orb",
    "divine": "Divine Orb",
    "chaos": "Chaos Orb",
    "annul": "Orb of Annulment",
    "regal": "Regal Orb",
    "alch": "Orb of Alchemy",
    "transmute": "Orb of Transmutation",
    "aug": "Orb of Augmentation",
    "vaal": "Vaal Orb",
    "mirror": "Mirror of Kalandra",
    "chance": "Orb of Chance",
}


def _r(x: float) -> float:
    """Arrondi à 4 chiffres significatifs : assez pour l'affichage, stable pour l'ETag."""
    return float(f"{x:.4g}")


def build_payload(
    *,
    league: str,
    realm: str,
    rates: dict[str, Rate],
    items: list[ItemPrice],
    names: dict[str, str],
    quality: dict[str, Any],
    now: datetime,
    fresh_for: timedelta = timedelta(minutes=5),
    frozen: bool = False,
) -> dict[str, Any]:
    entries: list[dict[str, Any]] = []
    for r in sorted(rates.values(), key=lambda r: r.currency):
        entries.append(
            {
                "id": f"currency/{r.currency}",
                "kind": "currency",
                "name": CURRENCY_NAMES.get(r.currency, r.currency),
                "price": _r(r.value),
                "low": _r(r.low),
                "high": _r(r.high),
                "sample_size": r.observations,
                "confidence": "high"
                if r.method == "numeraire"
                else confidence(r.observations, r.low, r.high),
                "method": r.method,
                "source": "currency_exchange",
                "updated_at": r.updated_at,
            }
        )
    for it in sorted(items, key=lambda i: i.query_id):
        if it.league != league:
            continue
        entries.append(
            {
                "id": f"{it.category}/{it.query_id}",
                "kind": "item",
                "name": names.get(it.query_id, it.query_id),
                "price": _r(it.value),
                "low": _r(it.low),
                "high": _r(it.high),
                "sample_size": it.sellers,
                "confidence": confidence(it.sellers, it.low, it.high),
                "method": "trade_median",
                "source": "trade_sample",
                "updated_at": it.updated_at,
            }
        )
    stamps = [e["updated_at"] for e in entries if e["updated_at"]]
    data_as_of = max(stamps) if stamps else None
    content = {"realm": realm, "league": league, "data_as_of": data_as_of, "items": entries}
    etag = hashlib.sha256(json.dumps(content, sort_keys=True).encode()).hexdigest()[:32]
    return {
        "schema_version": SCHEMA_VERSION,
        "realm": realm,
        "league": league,
        "unit": "exalted",
        "generated_at": now.isoformat(),
        "data_as_of": data_as_of,
        "fresh_until": (now + fresh_for).isoformat(),
        "frozen": frozen,
        "etag": f'"{etag}"',
        "items": entries,
        "quality": quality,
    }
