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


# Stats de gemme des ailments à dégâts -> (stat du moteur, nature) ; la
# valeur est celle du niveau. Les drapeaux (sans valeur) valent la constante.
AILMENTS = {
    "base_chance_to_poison_on_hit_%": ("poison_chance", "added"),
    "chance_to_poison_on_hit_with_attacks_%": ("poison_chance", "added"),
    "poison_chance_+%": ("poison_chance", "increased"),
    "base_chance_to_inflict_bleeding_%": ("bleed_chance", "added"),
    "bleed_on_hit_with_attacks_%": ("bleed_chance", "added"),
    "base_chance_to_ignite_%": ("ignite_chance", "added"),
    "ignite_chance_+%": ("ignite_chance", "increased"),
    "active_skill_ignite_chance_+%_final": ("ignite_chance", "more"),
    "base_ignite_effect_+%": ("ignite_magnitude", "increased"),
    "active_skill_ignite_effect_+%_final": ("ignite_magnitude", "more"),
    "active_skill_bleeding_effect_+%_final": ("bleed_magnitude", "more"),
    "ignite_duration_+%": ("ignite_duration", "increased"),
    "base_bleed_duration_+%": ("bleed_duration", "increased"),
}
DRAPEAUX_AILMENTS = {
    "global_poison_on_hit": ("poison_chance", "added", 100.0),
    "attacks_inflict_bleeding_on_hit": ("bleed_chance", "added", 100.0),
    "global_bleed_on_hit": ("bleed_chance", "added", 100.0),
    "always_ignite": ("ignite_chance", "added", 100.0),
    "never_ignite": ("ignite_chance", "more", -100.0),
    "active_skill_never_freeze_shock_ignite": ("ignite_chance", "more", -100.0),
    "cannot_poison": ("poison_chance", "more", -100.0),
    "cannot_cause_bleeding": ("bleed_chance", "more", -100.0),
}


def _ailments(stat_set: dict, level: dict) -> list:
    """Effets de la gemme sur les ailments à dégâts : [stat, nature, valeur]."""
    out = []
    ids = {x.get("id") for x in (stat_set.get("static") or {}).get("stats") or []}
    for stat_id, (stat, nature) in AILMENTS.items():
        v = _stat_value(stat_set, level, stat_id)
        if v:
            out.append([stat, nature, v])
    for stat_id, (stat, nature, v) in DRAPEAUX_AILMENTS.items():
        if stat_id in ids:
            out.append([stat, nature, v])
    return out


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


def _dot(stat_set: dict, level: dict) -> dict:
    """Dégâts dans le temps de base de la compétence, par seconde et par type."""
    out = {}
    for jeu, cle in ELEMENTS.items():
        v = _stat_value(stat_set, level, f"base_{jeu}_damage_to_deal_per_minute")
        if v:
            out[cle] = round(v / 60, 4)
    return out


def _set_de_dot(s: dict) -> dict | None:
    """Premier jeu de stats qui porte des dégâts dans le temps à chaque niveau."""
    for st in s.get("stat_sets") or []:
        levels = st.get("per_level") or {}
        if levels and all(_dot(st, lv) for lv in levels.values()):
            return st
    return None


def _dot_de_sort(s: dict) -> bool:
    """Les mods de dégâts des sorts s'appliquent-ils à ses dégâts dans le temps ?"""
    types = ((s.get("active_skill") or {}).get("types")) or []
    if "DegenOnlySpellDamage" in types:
        return True
    return any(
        x.get("id") == "spell_damage_modifiers_apply_to_skill_dot"
        for st in s.get("stat_sets") or []
        for x in (st.get("static") or {}).get("stats") or []
    )


def _ajouter_dot(s: dict, out: dict) -> None:
    """Ajoute aux niveaux de `out` les dégâts dans le temps de la compétence."""
    st = _set_de_dot(s)
    if st is None:
        return
    for lvl, data in st["per_level"].items():
        if lvl in out["niveaux"]:
            out["niveaux"][lvl]["dot"] = _dot(st, data)
    if _dot_de_sort(s):
        out["dot_sort"] = True


def active(skills: dict, skill_id: str, name: str, verifiee: bool = True) -> dict:
    s = skills.get(skill_id)
    if not isinstance(s, dict):
        raise DonneesError(f"compétence {skill_id} absente de skills.json")
    stat_set = s["stat_sets"][0] if verifiee else _set_de_degats(s)
    # Sort sans coup (Contagion, Tornado…) : seulement des dégâts dans le temps.
    sans_coup = stat_set is None and not verifiee and _set_de_dot(s) is not None
    if sans_coup:
        stat_set = _set_de_dot(s)
    if stat_set is None:
        raise DonneesError(f"{skill_id} : aucun jeu de stats avec des dégâts")
    static = stat_set["static"]
    niveaux = {}
    for lvl, data in sorted(stat_set["per_level"].items(), key=lambda kv: int(kv[0])):
        degats = _degats(stat_set, data)
        if not degats and not sans_coup:
            raise DonneesError(f"{skill_id} niveau {lvl} : dégâts absents")
        n = {"degats": degats}
        ailments = _ailments(stat_set, data)
        if ailments:
            n["ailments"] = ailments
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
    if not verifiee:
        _ajouter_dot(s, out)
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
        if _set_de_degats(s) is not None or _set_de_dot(s) is not None:
            return name
        return None
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
        m = minion(skill_id, s)
        if m:
            out[skill_id] = m
            continue
        a = attaque(skill_id, s)
        if a:
            out[skill_id] = a
    return dict(sorted(out.items()))


# Niveau des minions selon le niveau de la gemme (2 par niveau) et dégâts de
# base d'un monstre allié par niveau (1 à 100) : Path of Building PoE 2,
# src/Data/Misc.lua (`minionLevelTable`, `monsterAllyDamageTable`), licence MIT.
DEGATS_ALLIE = [
    3.11, 4.42, 5.82, 7.31, 8.92, 10.63, 12.46, 14.42, 16.51, 18.73,
    21.1, 23.62, 26.31, 29.16, 32.19, 35.42, 38.83, 42.46, 46.31, 50.39,
    54.71, 59.29, 64.14, 69.27, 74.69, 80.43, 86.5, 92.91, 99.69, 106.84,
    114.4, 122.37, 130.79, 139.67, 149.04, 158.91, 169.32, 180.29, 191.86, 204.04,
    216.86, 230.37, 244.6, 259.57, 275.32, 291.9, 309.34, 327.69, 346.98, 367.27,
    388.59, 411.01, 434.57, 459.32, 485.33, 512.66, 541.35, 571.49, 603.14, 636.37,
    671.26, 707.87, 746.3, 786.63, 828.94, 873.34, 919.91, 968.76, 1019.99, 1073.72,
    1130.06, 1189.13, 1251.06, 1315.98, 1384.03, 1455.34, 1530.08, 1608.4, 1690.46, 1776.43,
    1866.5, 1960.84, 2059.66, 2163.16, 2271.56, 2385.06, 2503.91, 2628.36, 2758.64, 2895.03,
    3037.8, 3187.24, 3343.66, 3507.35, 3678.66, 3857.93, 4045.51, 4241.77, 4447.11, 4661.93,
]


def minion(skill_id: str, s: dict) -> dict | None:
    """Gemme de minions : dégâts relatifs d'un minion allié à son niveau.

    Les dégâts propres à chaque minion (multiplicateur, temps d'attaque) ne
    sont pas dans RePoE : le DPS est un indice, juste en % entre deux objets.
    """
    if not skill_id.endswith("Player") or s.get("is_support"):
        return None
    a = s.get("active_skill") or {}
    name = (a.get("display_name") or "").strip()
    types = a.get("types") or []
    if not name or "[" in name or "{" in name or "CreatesMinion" not in types:
        return None
    ids = {
        x.get("id")
        for st in s.get("stat_sets") or []
        for x in (st.get("static") or {}).get("stats") or []
    }
    if "base_deal_no_damage" in ids:
        return None
    mt = a.get("minion_types") or []
    genre = "minion_attaque" if "Attack" in mt else "minion_sort" if "Spell" in mt else None
    levels = sorted((s.get("per_level") or {}), key=int)
    if genre is None or len(levels) < 2:
        return None
    niveaux = {}
    for lvl in levels:
        d = DEGATS_ALLIE[min(2 * int(lvl), 100) - 1]
        niveaux[lvl] = {"degats": {"physique": [d, d]}}
    return {
        "nom": name,
        "genre": genre,
        "temps_incantation": 1.0,
        "critique_base": 5.0,
        "niveaux": niveaux,
        "elements": [],
        "verifiee": False,
    }


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
        ailments = _ailments(st, data)
        if ailments:
            niveaux[lvl]["ailments"] = ailments
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
    _ajouter_dot(s, out)
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
