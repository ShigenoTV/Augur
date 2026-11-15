#!/usr/bin/env python3
"""Identifiants de stats du site de trade de PoE 1 pour les mods de la Forge PoE 1
(`crates/poe2-core/data/trade-stats-poe1.json`).

« Crafter ou acheter » (Forge PoE 1) ouvre la recherche de l'objet visé sur le site de trade de
PoE 1 : chaque ligne d'un mod voulu y devient un filtre de stat (`explicit.stat_…`). Ce script relie
chaque texte de mod de `forge-poe1.json.gz` (valeurs remplacées par `#`) à ses identifiants,
relevés dans `src/Data/TradeSiteStats.lua` de Path of Building Community (PoE 1, licence MIT),
copie de https://www.pathofexile.com/api/trade/data/stats. Le fichier vient du paquet PoB d'Augur
(`crates/pob-poe1/paquet/pob-poe1.tar.gz`).

Une stat locale (« (Local) » sur le site) et sa stat globale partagent un texte : la recherche
accepte l'une ou l'autre. Même format et même normalisation que `trade_stats.py` (PoE 2).

Utilisation :

    python tools/gamedata/trade_stats_poe1.py

Python 3.11+, bibliothèque standard seulement. Sortie déterministe.
"""

from __future__ import annotations

import argparse
import gzip
import json
import re
import tarfile
from pathlib import Path

from trade_stats import normaliser

SCHEMA = 1
SOURCE = (
    "Path of Building Community (PoE 1), src/Data/TradeSiteStats.lua "
    "(https://github.com/PathOfBuildingCommunity/PathOfBuilding), licence MIT"
)
RACINE = Path(__file__).resolve().parents[2]
PAQUET = RACINE / "crates" / "pob-poe1" / "paquet" / "pob-poe1.tar.gz"
FORGE = RACINE / "crates" / "poe2-core" / "data" / "forge-poe1.json.gz"
SORTIE = RACINE / "crates" / "poe2-core" / "data" / "trade-stats-poe1.json"

_ENTREE = re.compile(
    r'\["id"\]\s*=\s*"([^"]+)",\s*\["text"\]\s*=\s*"((?:[^"\\]|\\.)*)",\s*\["type"\]\s*=\s*"([^"]+)"'
)


def lire_stats_pob(lua: str) -> dict[str, dict]:
    """Texte normalisé -> {ids} pour les stats explicites du site de trade."""
    table: dict[str, set[str]] = {}
    for ident, texte, genre in _ENTREE.findall(lua):
        if genre != "explicit":
            continue
        texte = texte.replace('\\"', '"').replace("\\n", "\n")
        texte = re.sub(r"\s*\(Local\)$", "", texte)
        table.setdefault(normaliser(texte), set()).add(ident)
    return {k: {"ids": sorted(v)} for k, v in table.items()}


def lua_du_paquet(paquet: Path) -> str:
    with tarfile.open(paquet, "r:gz") as tar:
        f = tar.extractfile("src/Data/TradeSiteStats.lua")
        if f is None:
            raise SystemExit("TradeSiteStats.lua absent du paquet PoB")
        return f.read().decode("utf-8")


def chercher(ligne: str, table: dict[str, dict]) -> dict | None:
    """Entrée du site de trade pour une ligne de mod. Une stat « reduced » (ou un « -# ») est rangée
    par le site sous sa forme « increased » (ou « # ») négative : `negate`."""
    if ligne in table:
        return table[ligne]
    for avant, apres in (("reduced", "increased"), ("less", "more")):
        if avant in ligne and (e := table.get(ligne.replace(avant, apres))):
            return {**e, "negate": True}
    if ligne.startswith("-#") and (e := table.get(ligne[1:])):
        return {**e, "negate": True}
    return None


def construire(lignes: set[str], table: dict[str, dict]) -> dict:
    trouvees = {l: e for l in sorted(lignes) if (e := chercher(l, table))}
    return {
        "schema": SCHEMA,
        "source": SOURCE,
        "stats": trouvees,
        "sans_stat": sorted(l for l in lignes if l not in trouvees),
    }


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--paquet", type=Path, default=PAQUET)
    p.add_argument("--forge", type=Path, default=FORGE)
    p.add_argument("--out", type=Path, default=SORTIE)
    a = p.parse_args()
    table = lire_stats_pob(lua_du_paquet(a.paquet))
    sortie = construire(lignes_forge_poe1(a.forge), table)
    a.out.write_text(
        json.dumps(sortie, ensure_ascii=False, indent=1, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        f"{len(sortie['stats'])} lignes reliées, {len(sortie['sans_stat'])} sans stat de trade"
    )


def lignes_forge_poe1(chemin: Path) -> set[str]:
    """Lignes des mods de forge-poe1.json.gz (liste `mods` à la racine)."""
    with gzip.open(chemin, "rt", encoding="utf-8") as f:
        donnees = json.load(f)
    lignes: set[str] = set()
    for mod in donnees["mods"]:
        for ligne in mod["texte"].split("\n"):
            n = normaliser(ligne)
            if n:
                lignes.add(n)
    return lignes


if __name__ == "__main__":
    main()
