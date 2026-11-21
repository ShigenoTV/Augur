#!/usr/bin/env python3
"""Cas golden mesurés en jeu : lesquels sont à re-mesurer après un changement de version du jeu.

Chaque cas de `tests/golden/` porte le patch du client sur lequel il a été mesuré (`patch: "0.5.5e"`) et
sa confiance (`mesuré`, `confirmé` ou `supposé`). Un cas `mesuré` ou `confirmé` devient « à re-mesurer »
dès que les données embarquées viennent d'une version majeure plus récente du jeu (0.5 → 1.0) : la formule
peut encore passer, mais plus rien ne garantit qu'elle colle au jeu.

On compare seulement la version majeure (`0.5`, `1.0`…) : RePoE numérote les correctifs (`0.5.5.10`) et
GGG les nomme par lettre (`0.5.5e`), et les deux ne se correspondent pas.

Utilisation (depuis la racine du dépôt) :

    python tools/gamedata/cas_mesures.py                  # version des données embarquées (forge.json.gz)
    python tools/gamedata/cas_mesures.py --version 1.0.0.1
    python tools/gamedata/cas_mesures.py --github         # avertissements GitHub Actions (la CI reste verte)

Python 3.11+, bibliothèque standard seulement (lecture ligne à ligne des YAML, sans PyYAML).
"""

from __future__ import annotations

import argparse
import gzip
import json
import re
import sys
from pathlib import Path

RACINE = Path(__file__).resolve().parents[2]
GOLDEN = Path("tests/golden")
FORGE = Path("crates/poe2-core/data/forge.json.gz")
CONFIANCES_MESUREES = {"mesuré", "confirmé"}

_ID = re.compile(r"^(?:- )?id:\s*(\S+)\s*$")
_CHAMP = re.compile(r"^(?:  )?(patch|confiance):\s*\"?([^\"#]+?)\"?\s*(?:#.*)?$")


def version_majeure(version: str) -> tuple[int, int] | None:
    """`"0.5.5e"` -> `(0, 5)`, `"1.0.0.1"` -> `(1, 0)`, inconnue -> `None`."""
    m = re.match(r"\s*(\d+)\.(\d+)", version or "")
    return (int(m.group(1)), int(m.group(2))) if m else None


def lire_cas(racine: Path = RACINE) -> list[dict]:
    """Un dictionnaire `{id, patch, confiance, fichier}` par cas golden."""
    cas = []
    for fichier in sorted((racine / GOLDEN).rglob("*.yaml")):
        courant: dict | None = None
        for ligne in fichier.read_text(encoding="utf-8").splitlines():
            m = _ID.match(ligne)
            if m:
                courant = {"id": m.group(1), "patch": "", "confiance": "", "fichier": str(fichier.relative_to(racine))}
                cas.append(courant)
                continue
            m = _CHAMP.match(ligne)
            if m and courant is not None and not courant[m.group(1)]:
                courant[m.group(1)] = m.group(2).strip()
    return cas


def a_remesurer(cas: list[dict], version_donnees: str) -> list[dict]:
    """Cas mesurés en jeu sur une version majeure plus ancienne que celle des données."""
    cible = version_majeure(version_donnees)
    if cible is None:
        return []
    return [
        c
        for c in cas
        if c["confiance"] in CONFIANCES_MESUREES
        and (v := version_majeure(c["patch"])) is not None
        and v < cible
    ]


def version_embarquee(racine: Path = RACINE) -> str:
    chemin = racine / FORGE
    if not chemin.exists():
        return "?"
    return json.loads(gzip.decompress(chemin.read_bytes())).get("repoe_version") or "?"


def principal(argv: list[str] | None = None, racine: Path = RACINE) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--version", help="version du jeu des données (par défaut : celle de forge.json.gz)")
    p.add_argument("--github", action="store_true", help="écrire des avertissements GitHub Actions")
    a = p.parse_args(argv)
    version = a.version or version_embarquee(racine)
    cas = lire_cas(racine)
    mesures = [c for c in cas if c["confiance"] in CONFIANCES_MESUREES]
    vieux = a_remesurer(cas, version)
    print(f"Données du jeu : {version}. Cas golden : {len(cas)}, dont {len(mesures)} mesurés en jeu.")
    if not vieux:
        print("Aucun cas mesuré à refaire.")
        return 0
    print(f"{len(vieux)} cas mesurés sur une version plus ancienne, à re-mesurer en jeu :")
    for c in vieux:
        print(f"- {c['id']} (mesuré en {c['patch']}, {c['fichier']})")
        if a.github:
            print(
                f"::warning file={c['fichier']},title=Cas à re-mesurer::{c['id']} a été mesuré en {c['patch']}, "
                f"les données viennent de {version}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(principal())
