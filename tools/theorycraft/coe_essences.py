#!/usr/bin/env python3
"""Essences et Alloys de PoE 2 d'après Craft of Exile : mods garantis par classe d'objet.

Le fichier de données de l'émulateur de Craft of Exile (`json/poe2/<version>/data.json`, préfixé
`coedata=`) décrit chaque Essence dans `essences` :

- `entries[]` : `id`, `label` (nom, dans le fichier de langue), `type` (indice dans `types` : Lesser,
  Normal, Greater, Perfect, Corrupted, Alloy) ;
- `byessences[id][classe]` : mods que l'Essence (ou l'Alloy) peut poser sur cette classe d'objet. La
  classe est le champ `class` des classes de Craft of Exile (un seul pour toutes les armures de corps,
  par exemple).

Règles de l'émulateur (`poe2_essence`), reprises par le moteur de Forge :

- Lesser, Normal, Greater : objet Magique, qui devient Rare avec un mod garanti ;
- Perfect, Corrupted, Alloy : objet Rare ; retire un mod (du côté du mod posé si ce côté est plein, sinon
  au hasard), puis pose le mod ;
- plusieurs mods possibles pour la classe (Essence of Enhancement sur une armure, Essence of the Infinite,
  Alloys...) : un seul est posé, tiré à chances égales parmi ceux sans conflit de groupe avec l'objet ;
- refusée si l'objet porte déjà un mod crafted, ou si tous ses mods possibles sont en conflit de groupe ;
  le niveau d'objet n'est pas regardé.

Les identifiants sont ceux de poe.ninja (nom en minuscules, apostrophes retirées, mots joints par des
tirets), qui donne les prix (catégorie Essences, et Verisium pour les Alloys).

    python3 tools/theorycraft/coe_essences.py --data data.json --langue english.json \\
        --date 2026-11-08 --out tools/theorycraft/donnees/essences-coe.json

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

from coe_poids import SOURCE, lire_js

TYPES = ("lesser", "normal", "greater", "perfect", "corrupted", "alloy")

# Groupe de classes Craft of Exile -> classe d'objet des tables RePoE.
CLASSES = {
    "Amulets": "Amulet",
    "Belts": "Belt",
    "Body Armours": "Body Armour",
    "Boots": "Boots",
    "Bows": "Bow",
    "Bucklers": "Buckler",
    "Claws": "Claw",
    "Crossbows": "Crossbow",
    "Daggers": "Dagger",
    "Flails": "Flail",
    "Foci": "Focus",
    "Gloves": "Gloves",
    "Helmets": "Helmet",
    "One Hand Axes": "One Hand Axe",
    "One Hand Maces": "One Hand Mace",
    "One Hand Swords": "One Hand Sword",
    "Quarterstaves": "Warstaff",
    "Quivers": "Quiver",
    "Rings": "Ring",
    "Sceptres": "Sceptre",
    "Shields": "Shield",
    "Spears": "Spear",
    "Staves": "Staff",
    "Two Hand Axes": "Two Hand Axe",
    "Two Hand Maces": "Two Hand Mace",
    "Two Hand Swords": "Two Hand Sword",
    "Wands": "Wand",
}


def slug(nom: str) -> str:
    """Identifiant poe.ninja d'un nom : « Perfect Essence of the Body » -> perfect-essence-of-the-body,
    « The Runebinder's Alloy » -> the-runebinders-alloy."""
    return re.sub(r"[^a-z0-9]+", "-", nom.lower().replace("'", "")).strip("-")


def classes_repoe(coe: dict, langue: list) -> dict[str, str]:
    """Champ `class` de Craft of Exile -> classe RePoE (groupes sans équivalent écartés)."""
    out: dict[str, str] = {}
    for c in coe["classes"]["entries"]:
        nom = langue[c["label"]].split(" (")[0]
        if nom in CLASSES:
            out.setdefault(str(c["class"]), CLASSES[nom])
    return out


def essences(coe: dict, langue: list) -> tuple[list[dict], int]:
    """Essences et Alloys repris, et nombre de couples (Essence, classe) à plusieurs mods possibles."""
    cle = {str(m["id"]): m["key"] for m in coe["mods"]["entries"]}
    classes = classes_repoe(coe, langue)
    e = coe["essences"]
    out, multiples = [], 0
    for x in e["entries"]:
        if x["type"] >= len(TYPES):
            raise ValueError(f"type d'Essence inconnu : {x['type']}")
        cibles: dict[str, list[str]] = {}
        for classe, mods in e["byessences"].get(str(x["id"]), {}).items():
            if classe not in classes or not mods:
                continue
            multiples += len(mods) > 1
            # ordre de Craft of Exile gardé : sans effet sur le tirage (chances égales)
            cibles[classes[classe]] = [cle[str(m)] for m in mods]
        if cibles:
            nom = langue[x["label"]]
            out.append(
                {
                    "id": slug(nom),
                    "nom": nom,
                    "type": TYPES[x["type"]],
                    "cibles": dict(sorted(cibles.items())),
                }
            )
    return sorted(out, key=lambda x: x["id"]), multiples


def principal(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--data", type=Path, required=True)
    p.add_argument("--langue", type=Path, required=True)
    p.add_argument("--date", required=True)
    p.add_argument("--version", default="0.5.5.5")
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args(argv)
    liste, multiples = essences(lire_js(a.data), lire_js(a.langue))
    sortie = {
        "source": SOURCE,
        "date_releve": a.date,
        "version_donnees": a.version,
        "essences": liste,
    }
    a.out.write_text(
        json.dumps(sortie, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8"
    )
    couples = sum(len(x["cibles"]) for x in liste)
    print(
        f"{a.out} : {len(liste)} Essences et Alloys, {couples} couples (Essence, classe), "
        f"dont {multiples} à plusieurs mods possibles"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
