#!/usr/bin/env python3
"""Données embarquées par le moteur pour comparer un objet au personnage.

Entrées :

    skills.json   RePoE PoE 2 (https://repoe-fork.github.io/poe2/skills.json)
    base_items.json (facultatif) RePoE PoE 2 : bases d'armes, pour les attaques
    tree.json     arbre passif de Path of Building PoE 2
                  (src/TreeData/<version>/tree.json, licence MIT)

Sortie (JSON canonique, déterministe) :

    crates/poe2-core/data/gemmes.json   chaque sort à dégâts par niveau
                                        (Lightning Bolt et Arc vérifiés en jeu,
                                        les autres en formule générique), et le
                                        texte des effets de chaque support
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

# Archétypes vérifiés en jeu : obligatoires dans skills.json.
ACTIVES = {"LightningBoltPlayer": "Lightning Bolt", "ArcPlayer": "Arc"}

# Types de dégâts du jeu -> clés du moteur.
ELEMENTS = {
    "physical": "physique",
    "fire": "feu",
    "cold": "froid",
    "lightning": "foudre",
    "chaos": "chaos",
}
# Tags de compétence -> élément (bonus de niveaux « Fire Spell Skills »…).
TAGS = {"Physical": "physique", "Fire": "feu", "Cold": "froid", "Lightning": "foudre", "Chaos": "chaos"}

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


def _degats(stat_set: dict, level: dict) -> dict:
    """Dégâts de base du sort à ce niveau, par type ({} s'il n'en a pas)."""
    out = {}
    for jeu, cle in ELEMENTS.items():
        dmin = _stat_value(stat_set, level, f"spell_minimum_base_{jeu}_damage")
        dmax = _stat_value(stat_set, level, f"spell_maximum_base_{jeu}_damage")
        if dmin is not None and dmax is not None and dmax > 0:
            out[cle] = [dmin, dmax]
    return out


def _set_de_degats(s: dict) -> dict | None:
    """Premier jeu de stats qui porte des dégâts de sort à chaque niveau.

    Le premier est souvent celui de l'impact ; certains sorts (Frostflame Nova,
    Galvanic Field…) le mettent en second, derrière un effet sans dégâts.
    """
    for st in s.get("stat_sets") or []:
        levels = st.get("per_level") or {}
        if levels and all(_degats(st, lv) for lv in levels.values()):
            return st
    return None


def active(skills: dict, skill_id: str, name: str, verifiee: bool = True) -> dict:
    s = skills.get(skill_id)
    if not isinstance(s, dict):
        raise DonneesError(f"compétence {skill_id} absente de skills.json")
    stat_set = s["stat_sets"][0] if verifiee else _set_de_degats(s)
    if stat_set is None:
        raise DonneesError(f"{skill_id} : aucun jeu de stats avec des dégâts")
    static = stat_set["static"]
    niveaux = {}
    for lvl, data in sorted(stat_set["per_level"].items(), key=lambda kv: int(kv[0])):
        degats = _degats(stat_set, data)
        if not degats:
            raise DonneesError(f"{skill_id} niveau {lvl} : dégâts absents")
        n = {"degats": degats}
        chains = _stat_value(stat_set, data, "number_of_chains")
        if chains is not None:
            n["chaines"] = chains
        niveaux[lvl] = n
    types = ((s.get("active_skill") or {}).get("types")) or []
    elements = sorted({TAGS[t] for t in types if t in TAGS})
    if not elements:
        elements = sorted({e for n in niveaux.values() for e in n["degats"]})
    # Sans temps d'incantation : sort déclenché (ou persistant). Sa cadence
    # dépend de ce qui le déclenche : on prend son temps de recharge, sinon
    # 1 par seconde, ce qui garde juste la comparaison d'objets en %.
    declenche = not s.get("cast_time")
    cooldown = ((s.get("static") or {}).get("cooldown")) or 0
    temps = s["cast_time"] / 1000 if not declenche else (cooldown / 1000 if cooldown else 1.0)
    out = {
        "nom": name,
        "temps_incantation": temps,
        "critique_base": static.get("crit_chance", 0) / 100,
        "niveaux": niveaux,
        "elements": elements,
        "verifiee": verifiee,
    }
    if declenche:
        out["declenche"] = True
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


def sort_generique(skill_id: str, s: dict) -> str | None:
    """Nom du sort si c'est une gemme de joueur à dégâts calculable, sinon None."""
    if not skill_id.endswith("Player") or s.get("is_support"):
        return None
    if skill_id.startswith(("GS", "GT", "Mtx", "Archnemesis")):
        return None
    a = s.get("active_skill") or {}
    name = (a.get("display_name") or "").strip()
    if not name or "[" in name or "Spell" not in (a.get("types") or []):
        return None
    try:
        return name if _set_de_degats(s) is not None else None
    except (KeyError, TypeError, ValueError):
        return None


def actives(skills: dict) -> dict:
    """Archétypes vérifiés, puis tous les autres sorts à dégâts (formule générique)."""
    out = {k: active(skills, k, v) for k, v in ACTIVES.items()}
    for skill_id, s in skills.items():
        if skill_id in out or not isinstance(s, dict):
            continue
        name = sort_generique(skill_id, s)
        if name:
            out[skill_id] = active(skills, skill_id, name, verifiee=False)
            continue
        a = attaque(skill_id, s)
        if a:
            out[skill_id] = a
    return dict(sorted(out.items()))


CONVERSION = "active_skill_base_physical_damage_%_to_convert_to_"


def attaque(skill_id: str, s: dict) -> dict | None:
    """Attaque de joueur à dégâts : efficacité par niveau, vitesse, conversion."""
    if not skill_id.endswith("Player") or s.get("is_support"):
        return None
    if skill_id.startswith(("GS", "GT", "Mtx", "Archnemesis")):
        return None
    a = s.get("active_skill") or {}
    name = (a.get("display_name") or "").strip()
    types = a.get("types") or []
    if not name or "[" in name or "Attack" not in types:
        return None
    sets = s.get("stat_sets") or []
    if not sets or not sets[0].get("per_level"):
        return None
    st = sets[0]
    stats = (st.get("static") or {}).get("stats") or []
    ids = {x.get("id"): x for x in stats}
    if "base_deal_no_damage" in ids:
        return None
    niveaux = {}
    for lvl, data in sorted(st["per_level"].items(), key=lambda kv: int(kv[0])):
        niveaux[lvl] = {"degats": {}, "multiplicateur": float(data.get("damage_multiplier") or 100)}
    conversion = {}
    for jeu, cle in ELEMENTS.items():
        x = ids.get(CONVERSION + jeu)
        if x and x.get("value"):
            conversion[cle] = float(x["value"])
    asm = ((s.get("static") or {}).get("attack_speed_multiplier")) or 0
    out = {
        "nom": name,
        "genre": "attaque",
        "temps_incantation": 1.0,
        "critique_base": 0.0,
        "niveaux": niveaux,
        "elements": sorted({TAGS[t] for t in types if t in TAGS}),
        "verifiee": False,
        "vitesse_attaque": 100.0 + asm,
    }
    if conversion:
        out["conversion"] = {"physique": conversion}
    return out


# Classes d'armes du jeu (RePoE `item_class`) -> classes affichées en jeu.
CLASSES_ARMES = {
    "Claw": "Claws",
    "Dagger": "Daggers",
    "One Hand Sword": "One Hand Swords",
    "One Hand Axe": "One Hand Axes",
    "One Hand Mace": "One Hand Maces",
    "Spear": "Spears",
    "Flail": "Flails",
    "Two Hand Sword": "Two Hand Swords",
    "Two Hand Axe": "Two Hand Axes",
    "Two Hand Mace": "Two Hand Maces",
    "Warstaff": "Quarterstaves",
    "Bow": "Bows",
    "Crossbow": "Crossbows",
    "Talisman": "Talismans",
}


def armes(bases: dict) -> dict:
    """Bases d'armes : dégâts physiques, temps d'attaque, critique."""
    out = {}
    for b in bases.values():
        classe = CLASSES_ARMES.get(b.get("item_class"))
        p = b.get("properties") or {}
        name = (b.get("name") or "").strip()
        if not classe or not name or not p.get("attack_time"):
            continue
        out[name] = {
            "classe": classe,
            "physique": [float(p.get("physical_damage_min") or 0), float(p.get("physical_damage_max") or 0)],
            "temps_attaque": p["attack_time"] / 1000,
            "critique": (p.get("critical_strike_chance") or 0) / 100,
        }
    return dict(sorted(out.items()))


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
    p.add_argument("--bases", type=Path, help="base_items.json de RePoE (armes)")
    p.add_argument("--tree", type=Path, required=True)
    p.add_argument("--tree-version", required=True)
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args(argv)
    try:
        skills = json.loads(a.skills.read_text(encoding="utf-8"))
        tree_data = json.loads(a.tree.read_text(encoding="utf-8-sig"))
        gemmes = {
            "actives": actives(skills),
            "supports": supports(skills),
        }
        if a.bases:
            gemmes["armes"] = armes(json.loads(a.bases.read_text(encoding="utf-8")))
        arbre = {"version": a.tree_version, "passifs": tree(tree_data)}
    except (DonneesError, KeyError, TypeError, ValueError) as e:
        print(f"erreur : {e}", file=sys.stderr)
        return 1
    a.out.mkdir(parents=True, exist_ok=True)
    write(a.out / "gemmes.json", gemmes)
    write(a.out / "arbre.json", arbre)
    print(
        f"{len(gemmes['actives'])} compétences, {len(gemmes.get('armes', {}))} armes, "
        f"{len(gemmes['supports'])} supports, "
        f"{len(arbre['passifs'])} passifs "
        f"(arbre {a.tree_version})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
