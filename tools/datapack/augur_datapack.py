#!/usr/bin/env python3
"""Construction, signature et vérification des paquets de données d'Augur.

Un paquet de données contient les tables du jeu extraites pour un patch
(mods, stats, bases d'objets, gemmes…). Il est publié sur le CDN sous une
forme entièrement immuable, sauf le petit fichier de canal qui désigne le
paquet courant. Le format est décrit dans docs/devops/paquets-de-donnees.md.

Arborescence produite (relative à la racine des données sur le CDN) :

    {game}/blobs/{sha256}.{ext}                 contenu adressé par empreinte
    {game}/manifests/{patch}/{sha256}.json      manifeste du paquet
    {game}/manifests/{patch}/{sha256}.json.sig  signature ed25519 du manifeste
    {game}/channels/{channel}.json              pointeur signé vers le manifeste courant
                                                (enveloppe : charge utile + signature dans
                                                un seul fichier, mise à jour atomique)

Commandes :

    keygen   génère une paire de clés (la clé privée ne doit jamais être commitée)
    build    construit les blobs et le manifeste signé à partir d'un dossier
    channel  publie (localement) le pointeur de canal signé vers un manifeste
    verify   vérifie toute la chaîne canal → manifeste → blobs

Dépendance : `cryptography` (ed25519). Python 3.11+.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import sys
import time
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)
from cryptography.hazmat.primitives.serialization import (
    Encoding,
    PublicFormat,
)

SCHEMA_VERSION = 1
GAMES = {"poe2"}
CHANNELS = {"stable", "beta"}
PATCH_RE = re.compile(r"^\d+\.\d+\.\d+[a-z]?(?:\.\d+)?$")
NAME_RE = re.compile(r"^[a-z0-9][a-z0-9_\-./]*$")
EXT_RE = re.compile(r"^[a-z0-9]{1,8}$")
SHA_RE = re.compile(r"^[0-9a-f]{64}$")
DEFAULT_KEY_ENV = "AUGUR_DATA_SIGNING_KEY"


class DataPackError(Exception):
    """Erreur de construction ou de vérification d'un paquet."""


# --------------------------------------------------------------------------
# Clés et signatures
# --------------------------------------------------------------------------


def _b64(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


def public_key_b64(pub: Ed25519PublicKey) -> str:
    return _b64(pub.public_bytes(Encoding.Raw, PublicFormat.Raw))


def key_id(pub: Ed25519PublicKey) -> str:
    """Identifiant court d'une clé publique : 16 premiers hex du SHA-256."""
    raw = pub.public_bytes(Encoding.Raw, PublicFormat.Raw)
    return hashlib.sha256(raw).hexdigest()[:16]


def load_private_key(seed_b64: str) -> Ed25519PrivateKey:
    try:
        seed = base64.b64decode(seed_b64.strip(), validate=True)
    except ValueError as exc:  # binascii.Error hérite de ValueError
        raise DataPackError("clé privée : base64 invalide") from exc
    if len(seed) != 32:
        raise DataPackError("clé privée : 32 octets attendus (graine ed25519)")
    return Ed25519PrivateKey.from_private_bytes(seed)


def load_public_key(pub_b64: str) -> Ed25519PublicKey:
    try:
        raw = base64.b64decode(pub_b64.strip(), validate=True)
    except ValueError as exc:
        raise DataPackError("clé publique : base64 invalide") from exc
    if len(raw) != 32:
        raise DataPackError("clé publique : 32 octets attendus")
    return Ed25519PublicKey.from_public_bytes(raw)


def sign_bytes(priv: Ed25519PrivateKey, payload: bytes) -> bytes:
    """Renvoie le contenu du fichier .sig (JSON canonique) pour `payload`."""
    sig = {
        "alg": "ed25519",
        "key_id": key_id(priv.public_key()),
        "sig": _b64(priv.sign(payload)),
    }
    return canonical_json(sig)


def verify_bytes(trusted: list[Ed25519PublicKey], payload: bytes, sig_file: bytes) -> str:
    """Vérifie `payload` contre un fichier .sig. Renvoie le key_id utilisé."""
    try:
        sig = json.loads(sig_file)
        if sig.get("alg") != "ed25519":
            raise DataPackError("signature : algorithme non pris en charge")
        raw = base64.b64decode(sig["sig"], validate=True)
        kid = sig["key_id"]
    except (ValueError, KeyError, TypeError) as exc:
        raise DataPackError("signature : fichier .sig illisible") from exc
    for pub in trusted:
        if key_id(pub) != kid:
            continue
        try:
            pub.verify(raw, payload)
            return kid
        except InvalidSignature as exc:
            raise DataPackError(f"signature invalide (clé {kid})") from exc
    raise DataPackError(f"signature faite par une clé non reconnue ({kid})")


# --------------------------------------------------------------------------
# Utilitaires
# --------------------------------------------------------------------------


def canonical_json(obj: object) -> bytes:
    """JSON déterministe : clés triées, séparateurs compacts, UTF-8, \\n final."""
    return (
        json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode("utf-8")


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def _check_choice(value: str, allowed: set[str], what: str) -> None:
    if value not in allowed:
        raise DataPackError(f"{what} inconnu : {value!r} (attendu : {sorted(allowed)})")


# --------------------------------------------------------------------------
# Construction
# --------------------------------------------------------------------------


@dataclass(frozen=True)
class BuildResult:
    manifest_key: str
    manifest_sha256: str
    files: int


def build_pack(
    src: Path,
    out: Path,
    game: str,
    patch: str,
    priv: Ed25519PrivateKey,
    min_app_version: str | None = None,
    created_at: str | None = None,
) -> BuildResult:
    """Copie le contenu de `src` en blobs adressés et écrit le manifeste signé."""
    _check_choice(game, GAMES, "jeu")
    if not PATCH_RE.match(patch):
        raise DataPackError(f"numéro de patch invalide : {patch!r}")
    if not src.is_dir():
        raise DataPackError(f"dossier source introuvable : {src}")

    entries = []
    for path in sorted(p for p in src.rglob("*") if p.is_file()):
        rel = path.relative_to(src).as_posix()
        if not NAME_RE.match(rel) or ".." in rel.split("/"):
            raise DataPackError(f"nom de fichier refusé (minuscules, chiffres, - _ . /) : {rel}")
        ext = path.suffix.lstrip(".").lower() or "bin"
        if not EXT_RE.match(ext):
            raise DataPackError(f"extension refusée : {rel}")
        data = path.read_bytes()
        digest = sha256_hex(data)
        blob_key = f"blobs/{digest}.{ext}"
        _write(out / game / blob_key, data)
        entries.append({"name": rel, "sha256": digest, "size": len(data), "path": blob_key})

    if not entries:
        raise DataPackError(f"aucun fichier dans {src}")

    manifest: dict[str, object] = {
        "schema": SCHEMA_VERSION,
        "game": game,
        "patch": patch,
        "created_at": created_at or utc_now(),
        "files": entries,
    }
    if min_app_version:
        manifest["min_app_version"] = min_app_version

    body = canonical_json(manifest)
    digest = sha256_hex(body)
    key = f"manifests/{patch}/{digest}.json"
    _write(out / game / key, body)
    _write(out / game / (key + ".sig"), sign_bytes(priv, body))
    return BuildResult(manifest_key=key, manifest_sha256=digest, files=len(entries))


def write_channel(
    out: Path,
    game: str,
    channel: str,
    manifest_key: str,
    priv: Ed25519PrivateKey,
    sequence: int | None = None,
    published_at: str | None = None,
) -> Path:
    """Écrit le pointeur de canal signé. Le manifeste doit exister dans `out`."""
    _check_choice(game, GAMES, "jeu")
    _check_choice(channel, CHANNELS, "canal")
    manifest_path = out / game / manifest_key
    if not manifest_path.is_file():
        raise DataPackError(f"manifeste introuvable : {manifest_path}")
    body = manifest_path.read_bytes()
    manifest = json.loads(body)
    pointer = {
        "schema": SCHEMA_VERSION,
        "game": game,
        "channel": channel,
        "patch": manifest["patch"],
        "manifest": manifest_key,
        "manifest_sha256": sha256_hex(body),
        # Strictement croissant : l'application refuse un canal dont la
        # séquence est inférieure à la dernière vue (anti-rollback).
        "sequence": sequence if sequence is not None else int(time.time()),
        "published_at": published_at or utc_now(),
    }
    payload = canonical_json(pointer)
    # Enveloppe : la signature voyage avec la charge utile dans un seul objet,
    # pour qu'un client ne lise jamais un pointeur neuf avec une signature ancienne.
    envelope = json.loads(sign_bytes(priv, payload))
    envelope["payload"] = _b64(payload)
    path = out / game / "channels" / f"{channel}.json"
    _write(path, canonical_json(envelope))
    return path


# --------------------------------------------------------------------------
# Vérification
# --------------------------------------------------------------------------


class Source:
    """Lit des objets depuis un dossier local ou une URL de base (CDN)."""

    def __init__(self, root: str):
        self.root = root.rstrip("/")
        self.remote = root.startswith(("https://", "http://"))

    def get(self, key: str) -> bytes:
        if self.remote:
            req = urllib.request.Request(
                f"{self.root}/{key}", headers={"User-Agent": "augur-datapack-verify"}
            )
            with urllib.request.urlopen(req, timeout=30) as resp:  # noqa: S310
                return resp.read()
        path = Path(self.root) / key
        if not path.is_file():
            raise DataPackError(f"objet manquant : {key}")
        return path.read_bytes()


def verify_channel(
    source: Source,
    game: str,
    channel: str,
    trusted: list[Ed25519PublicKey],
    min_sequence: int = 0,
) -> dict:
    """Vérifie canal → manifeste → blobs, comme le fera l'application."""
    _check_choice(game, GAMES, "jeu")
    _check_choice(channel, CHANNELS, "canal")

    envelope_bytes = source.get(f"{game}/channels/{channel}.json")
    try:
        ch_body = base64.b64decode(json.loads(envelope_bytes)["payload"], validate=True)
    except (ValueError, KeyError, TypeError) as exc:
        raise DataPackError("canal : enveloppe illisible") from exc
    verify_bytes(trusted, ch_body, envelope_bytes)
    pointer = json.loads(ch_body)
    if pointer.get("schema") != SCHEMA_VERSION:
        raise DataPackError("canal : version de schéma non prise en charge")
    if pointer.get("game") != game or pointer.get("channel") != channel:
        raise DataPackError("canal : jeu ou canal ne correspondent pas à l'URL")
    if int(pointer["sequence"]) < min_sequence:
        raise DataPackError("canal : séquence en recul (tentative de rollback ?)")

    m_key = pointer["manifest"]
    if not re.fullmatch(r"manifests/[^/]+/[0-9a-f]{64}\.json", m_key):
        raise DataPackError(f"canal : chemin de manifeste invalide : {m_key}")
    m_body = source.get(f"{game}/{m_key}")
    if sha256_hex(m_body) != pointer["manifest_sha256"]:
        raise DataPackError("manifeste : empreinte différente de celle du canal")
    if not m_key.endswith(f"/{pointer['manifest_sha256']}.json"):
        raise DataPackError("manifeste : nom de fichier différent de son empreinte")
    verify_bytes(trusted, m_body, source.get(f"{game}/{m_key}.sig"))
    manifest = json.loads(m_body)
    if manifest.get("patch") != pointer["patch"] or manifest.get("game") != game:
        raise DataPackError("manifeste : patch ou jeu incohérents avec le canal")

    for entry in manifest["files"]:
        if not SHA_RE.match(entry["sha256"]) or not entry["path"].startswith(
            f"blobs/{entry['sha256']}."
        ):
            raise DataPackError(f"manifeste : entrée invalide : {entry.get('name')}")
        blob = source.get(f"{game}/{entry['path']}")
        if len(blob) != entry["size"] or sha256_hex(blob) != entry["sha256"]:
            raise DataPackError(f"blob corrompu : {entry['name']}")
    return pointer


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------


def _private_key_from_env(var: str) -> Ed25519PrivateKey:
    value = os.environ.get(var)
    if not value:
        raise DataPackError(f"variable d'environnement {var} absente (clé de signature)")
    return load_private_key(value)


def _trusted_keys(values: list[str]) -> list[Ed25519PublicKey]:
    keys = [load_public_key(v) for raw in values for v in raw.split(",") if v.strip()]
    if not keys:
        raise DataPackError("aucune clé publique de confiance fournie")
    return keys


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="augur_datapack", description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("keygen", help="génère une paire de clés ed25519")

    b = sub.add_parser("build", help="construit un paquet signé")
    b.add_argument("--src", type=Path, required=True)
    b.add_argument("--out", type=Path, required=True)
    b.add_argument("--game", default="poe2")
    b.add_argument("--patch", required=True)
    b.add_argument("--min-app-version")
    b.add_argument("--key-env", default=DEFAULT_KEY_ENV)

    c = sub.add_parser("channel", help="écrit le pointeur de canal signé")
    c.add_argument("--out", type=Path, required=True)
    c.add_argument("--game", default="poe2")
    c.add_argument("--channel", required=True)
    c.add_argument("--manifest", required=True, help="clé du manifeste, ex. manifests/0.4.1/<sha>.json")
    c.add_argument("--key-env", default=DEFAULT_KEY_ENV)

    v = sub.add_parser("verify", help="vérifie la chaîne complète d'un canal")
    v.add_argument("--root", required=True, help="dossier local ou URL de base du CDN")
    v.add_argument("--game", default="poe2")
    v.add_argument("--channel", required=True)
    v.add_argument("--pubkey", action="append", required=True, help="clé(s) publique(s) base64")

    args = parser.parse_args(argv)
    try:
        if args.cmd == "keygen":
            priv = Ed25519PrivateKey.generate()
            from cryptography.hazmat.primitives.serialization import (
                NoEncryption,
                PrivateFormat,
            )

            seed = priv.private_bytes(Encoding.Raw, PrivateFormat.Raw, NoEncryption())
            print("# Clé PRIVÉE (secret GitHub AUGUR_DATA_SIGNING_KEY, ne jamais commiter) :")
            print(_b64(seed))
            print("# Clé publique (à embarquer dans l'application) :")
            print(public_key_b64(priv.public_key()))
            print(f"# key_id : {key_id(priv.public_key())}")
        elif args.cmd == "build":
            res = build_pack(
                args.src, args.out, args.game, args.patch,
                _private_key_from_env(args.key_env), args.min_app_version,
            )
            print(json.dumps({"manifest": res.manifest_key, "sha256": res.manifest_sha256, "files": res.files}))
        elif args.cmd == "channel":
            path = write_channel(
                args.out, args.game, args.channel, args.manifest,
                _private_key_from_env(args.key_env),
            )
            print(path)
        elif args.cmd == "verify":
            pointer = verify_channel(Source(args.root), args.game, args.channel, _trusted_keys(args.pubkey))
            print(f"OK : {args.game}/{args.channel} → patch {pointer['patch']} (séquence {pointer['sequence']})")
    except DataPackError as exc:
        print(f"erreur : {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
