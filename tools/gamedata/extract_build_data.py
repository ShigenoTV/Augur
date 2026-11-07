#!/usr/bin/env python3
"""Données embarquées par le moteur pour comparer un objet au personnage.

Entrées :

    skills.json   RePoE PoE 2 (https://repoe-fork.github.io/poe2/skills.json)
    tree.json     arbre passif de Path of Building PoE 2
                  (src/TreeData/<version>/tree.json, licence MIT)

Sortie (JSON canonique, déterministe) :

    crates/poe2-core/data/gemmes.json   Lightning Bolt et Arc par niveau, et
                                        le texte des effets de chaque support
    crates/poe2-core/data/arbre.json    texte des stats de chaque passif

Utilisation :

    python tools/gamedata/extract_build_data.py --skills skills.json \\
        --tree tree.json --tree-version 0_4 --out crates/poe2-core/data

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

# Compétences dont le moteur sait calculer le DPS (archétypes vérifiés).
ACTIVES = {"LightningBoltPlayer": "Lightning Bolt", "ArcPlayer": "Arc"}

# « [Chain|Chains] » -> « Chains », « [Lightning] » -> « Lightning ».
BRACKET = re.compile(r"\[([^\]|]+)(?:\|([^\]]+))?\]")


class DonneesError(Exception):
    """Entrée inattendue : le format RePoE ou PoB a probablement changé."""


def clean(text: str) -> str:
    return BRACKET.sub(lambda m: m.group(2) or m.group(1), text)


def _stat_value(stat_set: dict, level: dict, stat_id: str) -> float | None:
    """Valeur d'une stat d'un niveau : les valeurs sont alignées sur `static.stats`."""
    ids = [s.get("id") for s in stat_set["static"]["stats"]]
    if stat_id not in ids:
        return None
    i = ids.index(stat_id)
    values = level.get("stats") or []
    if i < len(values) and values[i] is not None:
        return float(values[i]["value"])
    static = stat_set["static"]["stats"][i]
    return float(static["value"]) if "value" in static else None


def active(skills: dict, skill_id: str, name: str) -> dict:
    s = skills.get(skill_id)
    if not isinstance(s, dict):
        raise DonneesError(f"compétence {skill_id} absente de skills.json")
    stat_set = s["stat_sets"][0]
    static = stat_set["static"]
    niveaux = {}
    for lvl, data in sorted(stat_set["per_level"].items(), key=lambda kv: int(kv[0])):
        dmin = _stat_value(stat_set, data, "spell_minimum_base_lightning_damage")
        dmax = _stat_value(stat_set, data, "spell_maximum_base_lightning_damage")
        if dmin is None or dmax is None:
            raise DonneesError(f"{skill_id} niveau {lvl} : dégâts absents")
        n = {"foudre": [dmin, dmax]}
        chains = _stat_value(stat_set, data, "number_of_chains")
        if chains is not None:
            n["chaines"] = chains
        niveaux[lvl] = n
    out = {
        "nom": name,
        "temps_incantation": s["cast_time"] / 1000,
        "critique_base": static["crit_chance"] / 100,
        "niveaux": niveaux,
    }
    for q in static.get("quality_stats") or []:
        per_mille = (q.get("stats") or {}).get("number_of_chains")
        if per_mille is not None:
            out["chaines_par_qualite"] = per_mille / 1000
    ids = {x.get("id"): x for x in static["stats"]}
    if "arc_damage_+%_final_from_infusion_consumption" in ids:
        out["infusion"] = {
            "element": "foudre",
            "degats_more": ids["arc_damage_+%_final_from_infusion_consumption"]["value"],
            "chaines": ids["arc_chains_gained_from_infusion_consumption"]["value"],
        }
    return out


def supports(skills: dict) -> dict:
    """Texte des effets de chaque support (statique, sinon niveau 1)."""
    out = {}
    for skill_id, s in skills.items():
        if not isinstance(s, dict) or not s.get("is_support"):
            continue
        sets = s.get("stat_sets") or []
        if not sets:
            continue
        texts = dict((sets[0].get("static") or {}).get("stat_text") or {})
        first = (sets[0].get("per_level") or {}).get("1") or {}
        texts.update(first.get("stat_text") or {})
        lines = sorted({clean(t) for t in texts.values() if t})
        if lines:
            out[skill_id] = lines
    return dict(sorted(out.items()))


def tree(data: dict) -> dict:
    nodes = data.get("nodes")
    if not isinstance(nodes, dict):
        raise DonneesError("tree.json : « nodes » absent")
    out = {}
    for node_id, n in nodes.items():
        stats = n.get("stats") if isinstance(n, dict) else None
        if stats:
            out[node_id] = [clean(s) for s in stats]
    return dict(sorted(out.items(), key=lambda kv: int(kv[0])))


def write(path: Path, obj) -> None:
    path.write_text(
        json.dumps(obj, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--skills", type=Path, required=True)
    p.add_argument("--tree", type=Path, required=True)
    p.add_argument("--tree-version", required=True)
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args(argv)
    try:
        skills = json.loads(a.skills.read_text(encoding="utf-8"))
        tree_data = json.loads(a.tree.read_text(encoding="utf-8-sig"))
        gemmes = {
            "actives": {k: active(skills, k, v) for k, v in ACTIVES.items()},
            "supports": supports(skills),
        }
        arbre = {"version": a.tree_version, "passifs": tree(tree_data)}
    except (DonneesError, KeyError, TypeError, ValueError) as e:
        print(f"erreur : {e}", file=sys.stderr)
        return 1
    a.out.mkdir(parents=True, exist_ok=True)
    write(a.out / "gemmes.json", gemmes)
    write(a.out / "arbre.json", arbre)
    print(
        f"{len(gemmes['supports'])} supports, {len(arbre['passifs'])} passifs "
        f"(arbre {a.tree_version})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
