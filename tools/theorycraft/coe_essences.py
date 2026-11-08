#!/usr/bin/env python3
"""Essences de PoE 2 d'après Craft of Exile : mod garanti par classe d'objet.

Le fichier de données de l'émulateur de Craft of Exile (`json/poe2/<version>/data.json`, préfixé
`coedata=`) décrit chaque Essence dans `essences` :

- `entries[]` : `id`, `label` (nom, dans le fichier de langue), `type` (indice dans `types` : Lesser,
  Normal, Greater, Perfect, Corrupted, Alloy) ;
- `byessences[id][classe]` : mods que l'Essence pose sur cette classe d'objet. La classe est le champ
  `class` des classes de Craft of Exile (un seul pour toutes les armures de corps, par exemple).

Règles de l'émulateur (`poe2_essence`), reprises par le moteur de Forge :

- Lesser, Normal, Greater : objet Magique, qui devient Rare avec le mod garanti ;
- Perfect, Corrupted : objet Rare ; retire un mod (du côté du mod garanti si ce côté est plein, sinon au
  hasard), puis pose le mod garanti ;
- refusée si l'objet porte déjà un mod crafted ou un mod du même groupe ; le niveau d'objet n'est pas
  regardé.

Seules les Essences à **un** mod garanti par classe sont reprises pour cette classe (Forge ne modélise pas
encore le tirage entre plusieurs cibles). Les Alloys ne le sont pas (leurs mods manquent aux tables RePoE).
Les identifiants sont ceux de poe.ninja (nom en minuscules, mots joints par des tirets), qui donne les prix.

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

TYPES = ("lesser", "normal", "greater", "perfect", "corrupted")

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
    """Identifiant poe.ninja d'un nom : « Perfect Essence of the Body » -> perfect-essence-of-the-body."""
    return re.sub(r"[^a-z0-9]+", "-", nom.lower()).strip("-")


def classes_repoe(coe: dict, langue: list) -> dict[str, str]:
    """Champ `class` de Craft of Exile -> classe RePoE (groupes sans équivalent écartés)."""
    out: dict[str, str] = {}
    for c in coe["classes"]["entries"]:
        nom = langue[c["label"]].split(" (")[0]
        if nom in CLASSES:
            out.setdefault(str(c["class"]), CLASSES[nom])
    return out


def essences(coe: dict, langue: list) -> tuple[list[dict], int]:
    """Essences reprises et nombre de couples (Essence, classe) écartés car à plusieurs cibles."""
    cle = {str(m["id"]): m["key"] for m in coe["mods"]["entries"]}
    classes = classes_repoe(coe, langue)
    e = coe["essences"]
    out, multiples = [], 0
    for x in e["entries"]:
        if x["type"] >= len(TYPES):
            continue  # Alloy
        cibles: dict[str, str] = {}
        for classe, mods in e["byessences"].get(str(x["id"]), {}).items():
            if classe not in classes:
                continue
            if len(mods) != 1:
                multiples += 1
                continue
            cibles[classes[classe]] = cle[str(mods[0])]
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
        f"{a.out} : {len(liste)} Essences, {couples} couples (Essence, classe) ; "
        f"{multiples} écartés (plusieurs cibles)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
