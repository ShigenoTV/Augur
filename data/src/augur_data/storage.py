"""Stockage brut immuable des réponses GGG.

Plan fondateur (S1 Data) : « stockage brut dans l'object storage, aucun
retraitement destructif ». Chaque réponse est écrite une seule fois, telle
quelle, dans une enveloppe qui décrit d'où elle vient. Tout le reste
(nettoyage, indices, modèles) se recalcule à partir de ces snapshots.

Disposition des clés, identique en local et sur l'object storage S3 :

    raw/{source}/{realm}/{league}/{AAAA}/{MM}/{JJ}/{horodatage}-{sha256[:12]}.json.gz

Le backend local sert au développement et aux tests ; le backend S3 (R2 ou
Hetzner Object Storage) sera branché par le chantier DevOps avec le même
contrat `RawStore`.
"""

from __future__ import annotations

import gzip
import hashlib
import json
import re
from collections.abc import Iterator
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Protocol

from . import __version__

ENVELOPE_VERSION = 1
_SAFE = re.compile(r"[^A-Za-z0-9._-]+")


def slug(value: str) -> str:
    return _SAFE.sub("_", value).strip("_") or "_"


@dataclass(frozen=True)
class Snapshot:
    key: str
    sha256: str
    envelope: dict[str, Any]


class RawStore(Protocol):
    def put(self, key: str, data: bytes) -> None: ...

    def get(self, key: str) -> bytes: ...

    def list(self, prefix: str) -> Iterator[str]: ...


class LocalRawStore:
    """Object storage simulé sur disque, en écriture unique."""

    def __init__(self, root: Path) -> None:
        self.root = Path(root)

    def put(self, key: str, data: bytes) -> None:
        path = self.root / key
        path.parent.mkdir(parents=True, exist_ok=True)
        # 'xb' : échoue si l'objet existe déjà. Un snapshot ne s'écrase jamais.
        with path.open("xb") as fh:
            fh.write(data)

    def get(self, key: str) -> bytes:
        return (self.root / key).read_bytes()

    def list(self, prefix: str) -> Iterator[str]:
        base = self.root / prefix
        if not base.exists():
            return
        for p in sorted(base.rglob("*.json.gz")):
            yield p.relative_to(self.root).as_posix()


def make_envelope(
    *,
    source: str,
    realm: str,
    league: str,
    request: dict[str, Any],
    status: int,
    headers: dict[str, str],
    body: Any,
    fetched_at: datetime | None = None,
) -> dict[str, Any]:
    fetched_at = fetched_at or datetime.now(UTC)
    return {
        "envelope_version": ENVELOPE_VERSION,
        "collector_version": __version__,
        "source": source,
        "realm": realm,
        "league": league,
        "fetched_at": fetched_at.isoformat(),
        "request": request,
        "response": {"status": status, "headers": headers, "body": body},
    }


def write_snapshot(store: RawStore, envelope: dict[str, Any]) -> Snapshot:
    payload = json.dumps(envelope, ensure_ascii=False, sort_keys=True).encode()
    digest = hashlib.sha256(payload).hexdigest()
    ts = datetime.fromisoformat(envelope["fetched_at"])
    key = (
        f"raw/{slug(envelope['source'])}/{slug(envelope['realm'])}/{slug(envelope['league'])}/"
        f"{ts:%Y/%m/%d}/{ts:%Y%m%dT%H%M%S%fZ}-{digest[:12]}.json.gz"
    )
    # mtime=0 : même contenu, mêmes octets compressés.
    store.put(key, gzip.compress(payload, mtime=0))
    return Snapshot(key=key, sha256=digest, envelope=envelope)


def read_snapshots(store: RawStore, prefix: str) -> Iterator[Snapshot]:
    for key in store.list(prefix):
        raw = gzip.decompress(store.get(key))
        envelope = json.loads(raw)
        envelope["_key"] = key  # traçabilité : chaque ligne dérivée pointe vers son snapshot
        yield Snapshot(key=key, sha256=hashlib.sha256(raw).hexdigest(), envelope=envelope)
