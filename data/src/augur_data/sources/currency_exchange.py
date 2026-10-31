"""Source 1 : API officielle d'échange de devises (OAuth, scope `service:cxapi`).

Endpoint documenté par GGG : `GET https://api.pathofexile.com/currency-exchange/{realm}/{id}`
qui renvoie des agrégats horaires par marché (paire de devises), avec
`next_change_id` pour avancer d'heure en heure.

Contraintes connues (cartographie du chantier Architecture) : le scope
`service:cxapi` n'est accordé qu'à un client **confidentiel** en
`client_credentials`, donc uniquement côté backend. Le format exact des
champs ci-dessous est à confirmer sur la documentation officielle dès que
l'application OAuth du studio est enregistrée : le parseur est volontairement
tolérant et rejette proprement ce qu'il ne comprend pas.
"""

from __future__ import annotations

import math
from datetime import UTC, datetime
from typing import Any

from ..ggg import API_BASE, GGGClient
from ..storage import RawStore, Snapshot, make_envelope, write_snapshot

SOURCE = "currency_exchange"
BUDGET_KEY = "cxapi"


def collect(
    client: GGGClient, store: RawStore, *, realm: str = "poe2", change_id: int | None = None
) -> Snapshot:
    """Récupère une heure d'agrégats et l'écrit telle quelle dans le stockage brut."""
    url = f"{API_BASE}/currency-exchange/{realm}"
    if change_id is not None:
        url += f"/{change_id}"
    resp = client.request("GET", url, budget_key=BUDGET_KEY, authenticated=True)
    env = make_envelope(
        source=SOURCE,
        realm=realm,
        league="_all",  # une réponse couvre toutes les ligues du royaume
        request={"method": "GET", "url": resp.url},
        status=resp.status,
        headers=resp.headers,
        body=resp.body,
    )
    return write_snapshot(store, env)


def _positive(x: Any) -> float | None:
    try:
        v = float(x)
    except (TypeError, ValueError):
        return None
    return v if v > 0 and math.isfinite(v) else None


def parse(envelope: dict[str, Any]) -> list[dict[str, Any]]:
    """Transforme un snapshot en observations de paires de devises.

    Une observation = « 1 `base` vaut `rate` `quote` » sur l'heure, pondérée
    par le volume échangé côté `base`. Le taux préféré est le ratio des
    volumes échangés (prix moyen pondéré de l'heure) ; à défaut, la moyenne
    géométrique des ratios extrêmes.
    """
    body = envelope["response"]["body"] or {}
    hour = body.get("next_change_id")
    observed_at = (
        datetime.fromtimestamp(int(hour) - 3600, UTC).isoformat()
        if isinstance(hour, int | float)
        else envelope["fetched_at"]
    )
    out: list[dict[str, Any]] = []
    for m in body.get("markets") or []:
        try:
            a, b = str(m["market_id"]).split("|")
        except (KeyError, ValueError):
            continue
        vol = m.get("volume_traded") or {}
        va, vb = _positive(vol.get(a)), _positive(vol.get(b))
        rate = vb / va if va and vb else None
        method = "volume"
        if rate is None:
            lo, hi = m.get("lowest_ratio") or {}, m.get("highest_ratio") or {}
            r_lo = (_positive(lo.get(b)) or 0) / (_positive(lo.get(a)) or math.inf)
            r_hi = (_positive(hi.get(b)) or 0) / (_positive(hi.get(a)) or math.inf)
            if r_lo > 0 and r_hi > 0:
                rate, method = math.sqrt(r_lo * r_hi), "ratio_mid"
        if rate is None:
            continue
        out.append(
            {
                "source": SOURCE,
                "league": m.get("league") or envelope["league"],
                "observed_at": observed_at,
                "base": a,
                "quote": b,
                "rate": rate,
                "weight": va or 1.0,
                "method": method,
                "snapshot_key": envelope.get("_key"),
            }
        )
    return out
