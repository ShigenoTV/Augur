#!/usr/bin/env python3
"""Identifiants de stats du site de trade pour les mods de Forge (`crates/poe2-core/data/trade-stats.json`).

« Crafter ou acheter » ouvre une recherche de l'objet visé sur le site de trade : chaque ligne d'un mod
voulu y devient un filtre de stat (`explicit.stat_…`). Ce script relie chaque texte de mod de
`forge.json.gz` (valeurs remplacées par `#`) à ses identifiants de trade, relevés dans les données
d'Exiled Exchange 2 (`renderer/public/data/en/stats.ndjson`, licence MIT, © Alexander Drozdov et
contributeurs). Seules les lignes utilisées par Forge sont gardées.

Un texte peut avoir deux identifiants (stat globale et stat locale d'une pièce d'équipement : la
recherche accepte l'un ou l'autre). `negate` : le site de trade range la stat sous sa forme inverse
(« reduced » rangé en « increased » négatif).

La normalisation est la même que `poe2_core::forge::achat::normaliser` (un test Rust vérifie que chaque
ligne de mod de Forge est trouvée).

Utilisation :

    curl -sSLO https://raw.githubusercontent.com/Kvan7/Exiled-Exchange-2/master/renderer/public/data/en/stats.ndjson
    python tools/gamedata/trade_stats.py --forge crates/poe2-core/data/forge.json.gz \\
        --stats stats.ndjson --out crates/poe2-core/data/trade-stats.json

Python 3.11+, bibliothèque standard seulement. Sortie déterministe.
"""

from __future__ import annotations

import argparse
import gzip
import json
import re
from pathlib import Path

SCHEMA = 1
SOURCE = (
    "Exiled Exchange 2, renderer/public/data/en/stats.ndjson "
    "(https://github.com/Kvan7/Exiled-Exchange-2), licence MIT"
)

_PLAGE = re.compile(r"\(-?\d+(?:\.\d+)?--?\d+(?:\.\d+)?\)")
_NOMBRE = re.compile(r"(?<![\w#])[+-]?\d+(?:\.\d+)?")


def normaliser(ligne: str) -> str:
    """Texte d'une ligne de mod, valeurs remplacées par `#`, signe « + » retiré, en minuscules."""
    t = _PLAGE.sub("#", ligne)
    t = _NOMBRE.sub("#", t)
    return t.replace("+#", "#").strip().lower()


def lignes_forge(chemin: Path) -> set[str]:
    with gzip.open(chemin, "rt", encoding="utf-8") as f:
        donnees = json.load(f)
    lignes: set[str] = set()
    for mod in donnees["mods"]["mods"]:
        for ligne in mod["texte"].split("\n"):
            n = normaliser(ligne)
            if n:
                lignes.add(n)
    return lignes


def table_trade(lignes_ndjson) -> dict[str, dict]:
    """Texte normalisé -> {ids, negate} (première stat qui le reconnaît)."""
    table: dict[str, dict] = {}
    for brut in lignes_ndjson:
        brut = brut.strip()
        if not brut:
            continue
        stat = json.loads(brut)
        ids = ((stat.get("trade") or {}).get("ids") or {}).get("explicit")
        if not ids:
            continue
        for m in stat.get("matchers", []):
            cle = normaliser(m["string"])
            entree = {"ids": sorted(ids)}
            if m.get("negate"):
                entree["negate"] = True
            table.setdefault(cle, entree)
    return table


def construire(lignes: set[str], table: dict[str, dict]) -> dict:
    trouvees = {l: table[l] for l in sorted(lignes) if l in table}
    return {
        "schema": SCHEMA,
        "source": SOURCE,
        "stats": trouvees,
        "sans_stat": sorted(l for l in lignes if l not in table),
    }


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--forge", type=Path, required=True)
    p.add_argument("--stats", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args()
    with a.stats.open(encoding="utf-8") as f:
        table = table_trade(f)
    sortie = construire(lignes_forge(a.forge), table)
    a.out.write_text(
        json.dumps(sortie, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"{len(sortie['stats'])} lignes reliées, {len(sortie['sans_stat'])} sans stat de trade")


if __name__ == "__main__":
    main()
