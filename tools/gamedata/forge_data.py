#!/usr/bin/env python3
"""Données embarquées par Forge (`crates/poe2-core/data/forge.json.gz`).

Réunit dans un seul fichier compressé :

- les mods tirables du domaine `item` (hors Essence) de `tables/mods.json` ;
- les bases de `tables/bases.json` ;
- le relevé de poids poe2db de `tools/theorycraft/donnees/poids-poe2db.json` (estimations de
  Krakenbul, CC BY-NC-SA 3.0 : l'attribution suit ces données, voir `data/README.md`) ;
- en complément (`--complement`), les poids Craft of Exile des mods que poe2db n'a pas mesurés
  (`tools/theorycraft/donnees/poids-coe.json`, voir `LICENCE-poids-coe.md`) ;
- avec `--essences`, les mods possibles de chaque Essence et de chaque Alloy par classe d'objet
  (`tools/theorycraft/donnees/essences-coe.json`, Craft of Exile). Ces mods sont gardés même quand ils
  ne sont pas tirables (mods réservés aux Essences et aux Alloys).

Seuls les champs lus par `poe2_core::forge::donnees` sont gardés. La sortie est déterministe
(JSON canonique, gzip sans date) : deux exécutions sur les mêmes entrées donnent les mêmes octets.

Utilisation :

    python tools/gamedata/forge_data.py --tables var/gamedata/tables \\
        --poids tools/theorycraft/donnees/poids-poe2db.json \\
        --complement tools/theorycraft/donnees/poids-coe.json \\
        --essences tools/theorycraft/donnees/essences-coe.json \\
        --out crates/poe2-core/data/forge.json.gz

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import gzip
import json
from pathlib import Path

SCHEMA = 1
CHAMPS_MOD = (
    "id",
    "nom",
    "domaine",
    "slot",
    "groupe",
    "groupes",
    "type",
    "niveau",
    "texte",
    "spawn",
    "essence_seulement",
)
CHAMPS_BASE = ("id", "nom", "classe", "tags", "niveau_drop")


def construire(
    mods: dict,
    bases: dict,
    releve: dict,
    source: dict,
    complement: dict | None = None,
    essences: dict | None = None,
) -> dict:
    liste = essences["essences"] if essences else []
    cibles = {m for e in liste for ms in e["cibles"].values() for m in ms}
    connus = {m["id"] for m in mods["mods"]}
    if absents := sorted(cibles - connus):
        raise ValueError(f"mods garantis absents des tables : {', '.join(absents)}")
    garde = [
        {k: m[k] for k in CHAMPS_MOD if k in m}
        for m in mods["mods"]
        if m["id"] in cibles
        or (
            m["domaine"] == "item"
            and not m.get("essence_seulement")
            and any(s["weight"] > 0 for s in m["spawn"])
        )
    ]
    sortie = {
        "schema": SCHEMA,
        "repoe_version": source.get("version_repoe", ""),
        "mods": {
            "schema": mods["schema"],
            "mods": sorted(garde, key=lambda m: m["id"]),
        },
        "bases": {
            "schema": bases["schema"],
            "bases": sorted(
                ({k: b[k] for k in CHAMPS_BASE if k in b} for b in bases["bases"]),
                key=lambda b: b["id"],
            ),
        },
        "releve": {**releve, "complement": complement} if complement else releve,
    }
    if liste:
        sortie["essences"] = liste
    return sortie


def ecrire(donnees: dict, sortie: Path) -> None:
    texte = json.dumps(
        donnees, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    )
    sortie.parent.mkdir(parents=True, exist_ok=True)
    with (
        sortie.open("wb") as f,
        gzip.GzipFile(
            filename="", mode="wb", fileobj=f, mtime=0, compresslevel=9
        ) as gz,
    ):
        gz.write(texte.encode("utf-8"))


def principal(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--tables", type=Path, required=True)
    p.add_argument("--poids", type=Path, required=True)
    p.add_argument(
        "--complement",
        type=Path,
        help="poids Craft of Exile des mods sans poids poe2db (tools/theorycraft/donnees/poids-coe.json)",
    )
    p.add_argument(
        "--essences",
        type=Path,
        help="mod garanti de chaque Essence par classe (tools/theorycraft/donnees/essences-coe.json)",
    )
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args(argv)
    lire = lambda chemin: json.loads(chemin.read_text(encoding="utf-8"))
    donnees = construire(
        lire(a.tables / "mods.json"),
        lire(a.tables / "bases.json"),
        lire(a.poids),
        lire(a.tables / "source.json"),
        lire(a.complement) if a.complement else None,
        lire(a.essences) if a.essences else None,
    )
    ecrire(donnees, a.out)
    print(
        f"{a.out} : {len(donnees['mods']['mods'])} mods, {len(donnees['bases']['bases'])} bases, "
        f"{len(donnees['releve']['pages'])} pages de poids, {len(donnees.get('essences', []))} Essences, "
        f"RePoE {donnees['repoe_version']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
