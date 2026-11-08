#!/usr/bin/env python3
"""Contrôle des pools de Forge avec Craft of Exile.

Pour chaque base calculable et chaque niveau d'objet exporté, compare la chance de chaque mod
(poids / poids total de son côté, préfixe ou suffixe) à celle que donne le fichier de données de
Craft of Exile pour la classe de la base. Mods retenus côté Craft of Exile : domaine objet, préfixe
ou suffixe, sans influence, niveau requis atteint, poids mesuré (> 1).

    POOLS=pools.jsonl cargo test -p poe2-core --release --test forge_controle_coe -- --ignored
    python3 tools/theorycraft/coe_controle.py --data data.json --langue english.json --pools pools.jsonl

Contrôle aussi les mods écartés par le niveau minimum des monnaies Greater / Perfect (35, 44, 50, 70).

Résultat du 08/11/2026 (données 0.5.5.5) : 1 714 bases × 4 niveaux, 6 856 pools identiques.
Code de sortie 1 s'il reste un écart. Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path

from coe_poids import lire_js

PREFIXE, SUFFIXE, SANS_INFLUENCE, DOMAINE_OBJET = 1, 2, 6, 1


def chances(mods: dict[str, tuple[str, int]]) -> dict[str, float]:
    total: dict[str, int] = defaultdict(int)
    for cote, w in mods.values():
        total[cote] += w
    return {k: w / total[cote] for k, (cote, w) in mods.items()}


def pool_coe(coe: dict, classe: int, ilvl: int) -> dict[str, tuple[str, int]]:
    mods = {m["id"]: m for m in coe["mods"]["entries"]}
    groupes = {g["id"]: g for g in coe["modgroups"]["entries"]}
    pool = {}
    for id_mod, w in coe["classmods"].get(str(classe), {}).items():
        m = mods[int(id_mod)]
        g = groupes[m["group"]]
        if (
            w > 1
            and m["minlvl"] <= ilvl
            and g["domain"] == DOMAINE_OBJET
            and g["influence"] == SANS_INFLUENCE
            and g["type"] in (PREFIXE, SUFFIXE)
        ):
            pool[m["key"]] = ("Prefix" if g["type"] == PREFIXE else "Suffix", w)
    return pool


def ecartes_coe(coe: dict, classe: int, ilvl: int, minimum: int) -> set[str]:
    """Mods que le niveau minimum d'une monnaie Greater / Perfect écarte, règle de l'émulateur de
    Craft of Exile : dans un groupe (tiers par niveau croissant), un tier sous le minimum est écarté
    sauf s'il est le dernier tier accessible à l'ilvl."""
    mods = {m["id"]: m for m in coe["mods"]["entries"]}
    groupes: dict[int, list[dict]] = defaultdict(list)
    for id_mod in sorted(coe["classmods"].get(str(classe), {}), key=int):
        m = mods[int(id_mod)]
        groupes[m["group"]].append(m)
    ecartes = set()
    for tiers in groupes.values():
        k = max((i for i, m in enumerate(tiers) if m["minlvl"] <= ilvl), default=-1)
        for i, m in enumerate(tiers):
            if m["minlvl"] <= ilvl and m["minlvl"] < minimum and i < k:
                ecartes.add(m["key"])
    return ecartes


def controler(coe: dict, langue: list, pools: list[dict]) -> list[str]:
    classe_de = {}
    for it in coe["items"]["entries"]:
        if str(it["class"]) in coe["classmods"]:
            classe_de.setdefault(langue[it["label"]], it["class"])
    ecarts = []
    for p in pools:
        classe = classe_de.get(p["nom"])
        if classe is None:
            ecarts.append(f"{p['nom']} : absente de Craft of Exile")
            continue
        a = chances({k: (s, w) for k, s, w in p["mods"]})
        b = chances(pool_coe(coe, classe, p["ilvl"]))
        pire = max((abs(a.get(k, 0) - b.get(k, 0)), k) for k in a.keys() | b.keys())
        if pire[0] > 1e-9:
            ecarts.append(f"{p['nom']} ilvl {p['ilvl']} : {pire[1]} écart {pire[0]:.4f}")
            continue
        presents = {k for k, _, _ in p["mods"]}
        for minimum, nous in p.get("ecartes", {}).items():
            eux = ecartes_coe(coe, classe, p["ilvl"], int(minimum)) & presents
            if set(nous) != eux:
                diff = sorted(set(nous) ^ eux)[:3]
                ecarts.append(f"{p['nom']} ilvl {p['ilvl']} niveau min {minimum} : {diff}")
    return ecarts


def principal(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--data", type=Path, required=True)
    p.add_argument("--langue", type=Path, required=True)
    p.add_argument("--pools", type=Path, required=True)
    a = p.parse_args(argv)
    pools = [json.loads(l) for l in a.pools.read_text(encoding="utf-8").splitlines() if l]
    ecarts = controler(lire_js(a.data), lire_js(a.langue), pools)
    for e in ecarts[:50]:
        print(e)
    print(f"{len(pools) - len(ecarts)} pools identiques sur {len(pools)}")
    return 1 if ecarts else 0


if __name__ == "__main__":
    raise SystemExit(principal())
