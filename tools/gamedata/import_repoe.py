#!/usr/bin/env python3
"""Tables de mods et de bases d'objets PoE 2 depuis l'export RePoE.

Repris et adapté de l'importeur de poe2-craft-assistant (même auteur).
Entrées, à télécharger depuis https://repoe-fork.github.io/poe2/ :

    mods.min.json         tous les mods du jeu
    base_items.min.json   toutes les bases d'objets

Sortie : un dossier de tables prêt pour `tools/datapack/augur_datapack.py build`
(format : docs/devops/paquets-de-donnees.md, détail : docs/data/donnees-de-jeu.md).

    tables/mods.json      affixes d'équipement (préfixes et suffixes), avec stat_id
    tables/bases.json     bases d'équipement publiées, une entrée par vraie base
    tables/source.json    provenance et comptages

La sortie est déterministe (JSON canonique, aucune date) : deux exécutions sur
les mêmes entrées produisent les mêmes octets, donc les mêmes blobs sur le CDN.

Les poids de spawn sont ceux des données du jeu. Les poids estimés par Craft of
Exile, utilisés par poe2-craft-assistant, ne sont **pas** repris : ce sont des
données d'un tiers, à ne pas redistribuer sans son accord.

Utilisation :

    python tools/gamedata/import_repoe.py --mods mods.min.json \\
        --bases base_items.min.json --out var/gamedata --repoe-version 4.5.5.2

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

SCHEMA_VERSION = 1

# Domaines RePoE des affixes d'équipement : `item` (armes, armures, bijoux),
# `misc` (joyaux), `desecrated` (mods désacralisés des seigneurs).
MOD_DOMAINS = {"item", "misc", "desecrated"}
MOD_SLOTS = {"prefix", "suffix"}

# Classes d'objets qui reçoivent des affixes. Les Talismans et Pièges, ajoutés
# à la main dans poe2-craft-assistant, restent à vérifier dans RePoE.
EQUIP_CLASSES = {
    "Amulet", "Belt", "Body Armour", "Boots", "Bow", "Buckler", "Claw",
    "Crossbow", "Dagger", "Flail", "Focus", "Gloves", "Helmet", "Jewel",
    "One Hand Axe", "One Hand Mace", "One Hand Sword", "Quiver", "Ring",
    "Sceptre", "Shield", "Spear", "Staff", "Two Hand Axe", "Two Hand Mace",
    "Two Hand Sword", "Wand", "Warstaff",
}  # fmt: skip

# « [Resistances|Fire Resistance] » -> « Fire Resistance », « [Life] » -> « Life ».
BRACKET = re.compile(r"\[([^\]|]+)(?:\|([^\]]+))?\]")


class RepoeError(Exception):
    """Entrée RePoE inattendue : le format a probablement changé."""


def clean_text(text: str) -> str:
    """Retire les balises d'affichage du jeu, garde une ligne par stat."""
    return BRACKET.sub(lambda m: m.group(2) or m.group(1), text)


def _need(entry: dict, key: str, kind: type, where: str):
    value = entry.get(key)
    if not isinstance(value, kind):
        raise RepoeError(f"{where} : champ « {key} » absent ou invalide")
    return value


def import_mods(mods: dict) -> list[dict]:
    """Affixes d'équipement, triés par id.

    Contrairement à poe2-craft-assistant (craft seulement), on garde aussi les
    mods d'Essence et ceux de poids nul : le parseur et le pricing doivent
    reconnaître tout mod qu'on peut trouver sur un objet. `tirable` dit si le
    mod peut sortir d'un tirage classique.
    """
    if not isinstance(mods, dict):
        raise RepoeError("mods.min.json : un objet { id: mod } est attendu")
    out = []
    for mod_id, m in mods.items():
        if not isinstance(m, dict):
            raise RepoeError(f"mod {mod_id} : objet attendu")
        if (
            m.get("domain") not in MOD_DOMAINS
            or m.get("generation_type") not in MOD_SLOTS
        ):
            continue
        where = f"mod {mod_id}"
        stats = _need(m, "stats", list, where)
        if not stats:
            continue  # mod sans effet (marqueurs internes)
        groups = m.get("groups") or []
        spawn = [
            {
                "tag": _need(s, "tag", str, where),
                "weight": _need(s, "weight", int, where),
            }
            for s in _need(m, "spawn_weights", list, where)
        ]
        essence_only = bool(m.get("is_essence_only"))
        out.append(
            {
                "id": mod_id,
                "nom": m.get("name") or "",
                "domaine": m["domain"],
                "slot": m["generation_type"],
                # Groupe d'exclusion du jeu : deux mods du même groupe ne
                # coexistent jamais sur un objet.
                "groupe": groups[0] if groups else mod_id,
                "groupes": groups,
                "type": m.get("type") or "",
                "niveau": _need(m, "required_level", int, where),
                "texte": clean_text(m.get("text") or ""),
                "stats": [
                    {
                        "id": _need(s, "id", str, where),
                        "min": _need(s, "min", int, where),
                        "max": _need(s, "max", int, where),
                    }
                    for s in stats
                ],
                "tags": m.get("implicit_tags") or [],
                "spawn": spawn,
                "essence_seulement": essence_only,
                "tirable": not essence_only
                and any(s["weight"] > 0 and s["tag"] != "default" for s in spawn),
            }
        )
    out.sort(key=lambda x: x["id"])
    return out


def import_bases(items: dict) -> list[dict]:
    """Bases d'équipement publiées, une entrée par vraie base, triées par id.

    poe2-craft-assistant gardait un représentant par classe et archétype ; ici
    on garde toutes les bases, car le parseur doit reconnaître le type de base
    exact d'un objet copié en jeu (et son implicite).
    """
    if not isinstance(items, dict):
        raise RepoeError("base_items.min.json : un objet { id: base } est attendu")
    out = []
    for base_id, b in items.items():
        if not isinstance(b, dict):
            raise RepoeError(f"base {base_id} : objet attendu")
        if (
            b.get("domain") != "item"
            or b.get("release_state") != "released"
            or b.get("item_class") not in EQUIP_CLASSES
        ):
            continue
        where = f"base {base_id}"
        req = b.get("requirements") or {}
        out.append(
            {
                "id": base_id,
                "nom": _need(b, "name", str, where),
                "classe": b["item_class"],
                "tags": _need(b, "tags", list, where),
                "niveau_drop": b.get("drop_level") or 1,
                "implicites": b.get("implicits") or [],
                "exigences": {
                    "niveau": req.get("level") or 0,
                    "force": req.get("strength") or 0,
                    "dexterite": req.get("dexterity") or 0,
                    "intelligence": req.get("intelligence") or 0,
                },
            }
        )
    out.sort(key=lambda x: x["id"])
    return out


def canonical_json(obj: object) -> bytes:
    """Même forme que tools/datapack : clés triées, compact, UTF-8, saut de ligne final."""
    return (
        json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode("utf-8")


def build_tables(
    mods: dict, items: dict, repoe_version: str | None
) -> dict[str, bytes]:
    out_mods = import_mods(mods)
    out_bases = import_bases(items)
    if not out_mods or not out_bases:
        raise RepoeError("aucun mod ou aucune base retenu : le format RePoE a changé ?")
    groups = Counter(m["groupe"] for m in out_mods)
    source = {
        "schema": SCHEMA_VERSION,
        "source": "repoe-fork.github.io/poe2 (mods.min.json, base_items.min.json)",
        "version_repoe": repoe_version,
        "poids": "données du jeu (aucun poids estimé par un tiers)",
        "comptes": {
            "mods": len(out_mods),
            "mods_tirables": sum(m["tirable"] for m in out_mods),
            "groupes": len(groups),
            "bases": len(out_bases),
        },
    }
    return {
        "tables/mods.json": canonical_json(
            {"schema": SCHEMA_VERSION, "mods": out_mods}
        ),
        "tables/bases.json": canonical_json(
            {"schema": SCHEMA_VERSION, "bases": out_bases}
        ),
        "tables/source.json": canonical_json(source),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--mods", type=Path, required=True, help="mods.min.json de RePoE")
    ap.add_argument(
        "--bases", type=Path, required=True, help="base_items.min.json de RePoE"
    )
    ap.add_argument("--out", type=Path, required=True, help="dossier de sortie")
    ap.add_argument(
        "--repoe-version",
        help="version affichée sur repoe-fork.github.io/poe2 (ex. 4.5.5.2)",
    )
    args = ap.parse_args(argv)
    try:
        mods = json.loads(args.mods.read_text(encoding="utf-8"))
        items = json.loads(args.bases.read_text(encoding="utf-8"))
        tables = build_tables(mods, items, args.repoe_version)
    except (OSError, json.JSONDecodeError, RepoeError) as e:
        print(f"erreur : {e}", file=sys.stderr)
        return 1
    for name, data in tables.items():
        path = args.out / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    comptes = json.loads(tables["tables/source.json"])["comptes"]
    print(
        f"{args.out} : {comptes['mods']} mods ({comptes['mods_tirables']} tirables, "
        f"{comptes['groupes']} groupes), {comptes['bases']} bases"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
