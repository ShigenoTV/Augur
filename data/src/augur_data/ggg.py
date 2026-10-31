"""Client HTTP vers les API de GGG, à usage exclusif de l'ingestion backend.

Garde-fous (plan fondateur section 0, SECURITY.md) :
- toute requête passe par le `BudgetManager` (acquire avant, observe après) ;
- User-Agent identifiable : `OAuth {clientId}/{version} (contact: {email})` ;
- jeton OAuth du studio lu dans l'environnement, jamais dans le code ni les logs ;
- aucun cookie, et en particulier jamais de `POESESSID` ;
- kill switch : variable `AUGUR_INGESTION_KILL_SWITCH=1` ou fichier sentinelle,
  vérifié avant chaque requête.
"""

from __future__ import annotations

import os
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import httpx

from . import __version__
from .ratelimit import PRIORITY_INGESTION, BudgetManager, RateLimitedError

WWW_BASE = "https://www.pathofexile.com"
API_BASE = "https://api.pathofexile.com"

# En-têtes de réponse conservés dans les snapshots bruts. Aucun en-tête de
# requête n'est conservé (il porterait le jeton).
KEPT_RESPONSE_HEADERS = ("date", "etag", "last-modified", "retry-after", "x-rate-limit-")


class KillSwitchError(RuntimeError):
    """L'ingestion est coupée par le kill switch."""


@dataclass(frozen=True)
class ClientIdentity:
    client_id: str
    contact_email: str
    version: str = __version__

    @property
    def user_agent(self) -> str:
        return f"OAuth {self.client_id}/{self.version} (contact: {self.contact_email})"

    @classmethod
    def from_env(cls) -> ClientIdentity:
        try:
            return cls(
                client_id=os.environ["AUGUR_GGG_CLIENT_ID"],
                contact_email=os.environ["AUGUR_GGG_CONTACT"],
            )
        except KeyError as exc:
            raise RuntimeError(
                f"variable d'environnement manquante : {exc.args[0]} "
                "(identité OAuth du studio, fournie par les secrets de déploiement)"
            ) from None


def kill_switch_engaged(env: Mapping[str, str] | None = None) -> bool:
    env = os.environ if env is None else env
    if env.get("AUGUR_INGESTION_KILL_SWITCH", "").strip() in {"1", "true", "on"}:
        return True
    sentinel = env.get("AUGUR_INGESTION_KILL_FILE")
    return bool(sentinel) and Path(sentinel).exists()


@dataclass(frozen=True)
class Response:
    status: int
    url: str
    headers: dict[str, str]
    body: Any


def _kept_headers(headers: Mapping[str, str]) -> dict[str, str]:
    return {
        k.lower(): v
        for k, v in headers.items()
        if any(k.lower().startswith(p) for p in KEPT_RESPONSE_HEADERS)
    }


class GGGClient:
    def __init__(
        self,
        identity: ClientIdentity,
        budget: BudgetManager,
        *,
        access_token: str | None = None,
        transport: httpx.BaseTransport | None = None,
        timeout: float = 20.0,
    ) -> None:
        self._identity = identity
        self._budget = budget
        self._token = access_token
        self._http = httpx.Client(
            headers={"User-Agent": identity.user_agent, "Accept": "application/json"},
            transport=transport,
            timeout=timeout,
            follow_redirects=False,
        )

    def close(self) -> None:
        self._http.close()

    def __enter__(self) -> GGGClient:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def request(
        self,
        method: str,
        url: str,
        *,
        budget_key: str,
        priority: int = PRIORITY_INGESTION,
        json: Any = None,
        params: Mapping[str, str] | None = None,
        authenticated: bool = False,
    ) -> Response:
        if kill_switch_engaged():
            raise KillSwitchError("ingestion GGG coupée par le kill switch")
        headers: dict[str, str] = {}
        if authenticated:
            if not self._token:
                raise RuntimeError("jeton OAuth du studio absent pour un endpoint authentifié")
            headers["Authorization"] = f"Bearer {self._token}"
        self._budget.acquire(budget_key, priority)
        # Un client httpx neuf n'a pas de cookies ; on s'assure qu'il n'en garde
        # jamais, pour qu'aucun cookie de session ne puisse transiter.
        self._http.cookies.clear()
        resp = self._http.request(method, url, json=json, params=params, headers=headers)
        self._budget.observe(budget_key, resp.status_code, resp.headers)
        if resp.status_code == 429:
            raise RateLimitedError(budget_key, float(resp.headers.get("retry-after", 60)))
        resp.raise_for_status()
        return Response(
            status=resp.status_code,
            url=str(resp.request.url.copy_with(query=None)),
            headers=_kept_headers(resp.headers),
            body=resp.json(),
        )
