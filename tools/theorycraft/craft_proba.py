#!/usr/bin/env python3
"""Probabilités de mods pour le craft (exploration Forge, hors sprint 1).

Sépare deux choses :
- l'ÉLIGIBILITÉ d'un mod sur une base, lue dans RePoE (`mods.json` et
  `base_items.json`) : première étiquette de `spawn_weights` présente sur la
  base, poids > 0, niveau requis <= niveau d'objet. Les `spawn_weights` de
  RePoE ne valent que 0 ou 1 : ce ne sont PAS les vrais poids ;
- les POIDS réels, fournis en entrée (`{id_du_mod: poids}`). Ils ne sont pas
  versés au dépôt (données tierces) ; un poids manquant lève une erreur plutôt
  que d'être inventé.

Usage : python3 tools/theorycraft/craft_proba.py REPOE_DIR POIDS.json BASE ILVL MOD...
"""

from __future__ import annotations

import json
import random
import sys
from pathlib import Path


def poids_eligibilite(mod: dict, etiquettes_base: set[str]) -> float:
    """Poids RePoE du mod pour une base : première étiquette commune, sinon 0."""
    for s in mod.get("spawn_weights", []):
        if s["tag"] == "default" or s["tag"] in etiquettes_base:
            return float(s["weight"])
    return 0.0


def candidats(mods: dict, etiquettes_base: set[str], ilvl: int) -> dict[str, dict]:
    """Mods préfixe/suffixe tirables sur cette base à ce niveau d'objet."""
    res = {}
    for mid, m in mods.items():
        if m.get("domain") != "item" or m.get("generation_type") not in (
            "prefix",
            "suffix",
        ):
            continue
        if m.get("is_essence_only") or m.get("required_level", 0) > ilvl:
            continue
        if poids_eligibilite(m, etiquettes_base) > 0:
            res[mid] = m
    return res


def distribution_prochain_mod(
    cands, poids, presents=(), prefixes_libres=3, suffixes_libres=3
):
    """Probabilité de chaque mod au prochain tirage (Orbe d'exaltation).

    `presents` : identifiants des mods déjà sur l'objet (leurs groupes sont exclus).
    """
    groupes = (
        {g for mid in presents for g in cands[mid]["groups"]} if presents else set()
    )
    pool = {}
    for mid, m in cands.items():
        if mid in presents or groupes & set(m["groups"]):
            continue
        if m["generation_type"] == "prefix" and prefixes_libres <= 0:
            continue
        if m["generation_type"] == "suffix" and suffixes_libres <= 0:
            continue
        if mid not in poids:
            raise KeyError(f"poids manquant pour {mid}")
        if poids[mid] > 0:
            pool[mid] = poids[mid]
    total = sum(pool.values())
    return {mid: w / total for mid, w in pool.items()} if total else {}


def proba_cible(distribution, cibles) -> float:
    """Probabilité que le prochain mod appartienne à `cibles` (ids de mods)."""
    return sum(p for mid, p in distribution.items() if mid in set(cibles))


def esperance_essais(p: float) -> float:
    """Nombre moyen de tirages indépendants pour un succès (loi géométrique)."""
    return float("inf") if p <= 0 else 1.0 / p


def monte_carlo_ajouts(
    cands,
    poids,
    cibles_groupes,
    n_ajouts,
    essais=20000,
    graine=1,
    prefixes_libres=3,
    suffixes_libres=3,
):
    """Probabilité qu'après `n_ajouts` exaltations, chaque groupe cible soit présent."""
    rng = random.Random(graine)
    reussites = 0
    for _ in range(essais):
        presents, pl, sl = [], prefixes_libres, suffixes_libres
        for _ in range(n_ajouts):
            d = distribution_prochain_mod(cands, poids, presents, pl, sl)
            if not d:
                break
            mid = rng.choices(list(d), weights=list(d.values()))[0]
            presents.append(mid)
            if cands[mid]["generation_type"] == "prefix":
                pl -= 1
            else:
                sl -= 1
        groupes = {g for mid in presents for g in cands[mid]["groups"]}
        reussites += set(cibles_groupes) <= groupes
    return reussites / essais


def _principal(argv):
    repoe, poids_fichier, base, ilvl, *cibles = argv
    mods = json.loads((Path(repoe) / "mods.json").read_text())
    bases = json.loads((Path(repoe) / "base_items.json").read_text())
    tags = set(next(b for b in bases.values() if b["name"] == base)["tags"])
    cands = candidats(mods, tags, int(ilvl))
    poids = json.loads(Path(poids_fichier).read_text())
    cands = {k: v for k, v in cands.items() if k in poids}
    d = distribution_prochain_mod(cands, poids)
    p = proba_cible(d, cibles)
    print(
        f"{len(cands)} mods avec poids ; P(cible au prochain tirage) = {p:.4f} ; essais moyens = {esperance_essais(p):.1f}"
    )


if __name__ == "__main__":
    _principal(sys.argv[1:])
