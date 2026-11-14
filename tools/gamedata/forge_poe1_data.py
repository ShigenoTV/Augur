#!/usr/bin/env python3
"""Données embarquées par la Forge PoE 1 (`crates/poe2-core/data/forge-poe1.json.gz`).

Source unique : RePoE pour Path of Exile 1 (https://repoe-fork.github.io/), tables extraites des
fichiers du jeu. Contrairement à PoE 2, les tables PoE 1 portent les **vrais poids de génération**
des mods (`spawn_weights`) : aucun relevé tiers n'est nécessaire.

Le fichier réunit :

- les classes d'équipement craftables et leurs étiquettes d'influence (`item_classes.json`) ;
- les bases publiées de ces classes (`base_items.json`) ;
- les préfixes et suffixes du domaine `item` (`mods.json`) : ceux qui ont un poids quelque part,
  plus les mods garantis par les Essences (poids nul, posés seulement par une Essence) ;
- les Essences (`essences.json`) : mod garanti par classe, niveau d'objet maximal, et si elles
  refont un objet Rare (Screaming et au-dessus) ou seulement un objet Normal.

Seuls les champs lus par `poe2_core::poe1::forge` sont gardés. La sortie est déterministe (JSON
canonique, gzip sans date) : deux exécutions sur les mêmes entrées donnent les mêmes octets.

Utilisation (depuis la racine du dépôt) :

    python tools/gamedata/forge_poe1_data.py                    # télécharge RePoE PoE 1
    python tools/gamedata/forge_poe1_data.py --depuis DOSSIER   # fichiers déjà téléchargés

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import gzip
import io
import json
import sys
import urllib.request
from pathlib import Path

SCHEMA = 1
REPOE = "https://repoe-fork.github.io/"
FICHIERS = ("mods.json", "base_items.json", "essences.json", "item_classes.json", "version.txt")
RACINE = Path(__file__).resolve().parents[2]
SORTIE = RACINE / "crates" / "poe2-core" / "data" / "forge-poe1.json.gz"

# Classes d'équipement que la Forge PoE 1 sait calculer (Rare : 3 préfixes / 3 suffixes, 4 à 6 mods).
# Les joyaux (2/2, 3 ou 4 mods), les flacons et les objets à part (Trinkets, Tinctures…) n'y sont pas.
CLASSES = (
    "Amulet",
    "Ring",
    "Belt",
    "Body Armour",
    "Boots",
    "Gloves",
    "Helmet",
    "Shield",
    "Quiver",
    "Bow",
    "Claw",
    "Dagger",
    "Rune Dagger",
    "One Hand Axe",
    "One Hand Mace",
    "One Hand Sword",
    "Thrusting One Hand Sword",
    "Sceptre",
    "Staff",
    "Warstaff",
    "Two Hand Axe",
    "Two Hand Mace",
    "Two Hand Sword",
    "Wand",
)

# Étiquette d'influence (suffixe de `influence_tags`) -> nom affiché.
INFLUENCES = {
    "shaper": "Shaper",
    "elder": "Elder",
    "crusader": "Crusader",
    "eyrie": "Redeemer",
    "basilisk": "Hunter",
    "adjudicator": "Warlord",
}

# Rang d'une Essence d'après son préfixe de nom (les Essences spéciales n'en ont pas).
RANGS = ("Whispering", "Muttering", "Weeping", "Wailing", "Screaming", "Shrieking", "Deafening")


def telecharger(dossier: Path) -> None:
    dossier.mkdir(parents=True, exist_ok=True)
    for nom in FICHIERS:
        req = urllib.request.Request(REPOE + nom, headers={"User-Agent": "Augur-gamedata"})
        with urllib.request.urlopen(req, timeout=120) as r:
            (dossier / nom).write_bytes(r.read())


def lire_json(dossier: Path, nom: str):
    return json.loads((dossier / nom).read_text(encoding="utf-8"))


def classes(item_classes: dict) -> dict:
    sortie = {}
    for c in CLASSES:
        if c not in item_classes:
            raise ValueError(f"classe absente de item_classes.json : {c}")
        tags = {}
        for t in item_classes[c].get("influence_tags") or []:
            suffixe = t.rsplit("_", 1)[-1]
            if suffixe in INFLUENCES:
                tags[INFLUENCES[suffixe]] = t
        sortie[c] = {"influences": dict(sorted(tags.items()))}
    return sortie


def bases(items: dict, table_mods: dict) -> list[dict]:
    sortie = []
    for bid, b in items.items():
        if (
            b.get("domain") != "item"
            or b.get("release_state") != "released"
            or b.get("item_class") not in CLASSES
            or not b.get("name")
            or b["name"].startswith("[")
        ):
            continue
        sortie.append(
            {
                "id": bid,
                "nom": b["name"],
                "classe": b["item_class"],
                "tags": list(b.get("tags") or []),
                "niveau_drop": int(b.get("drop_level") or 1),
                "implicites": [
                    (table_mods.get(i) or {}).get("text", "").replace("\r", "")
                    for i in b.get("implicits") or []
                    if (table_mods.get(i) or {}).get("text")
                ],
            }
        )
    sortie.sort(key=lambda b: (b["classe"], b["niveau_drop"], b["nom"], b["id"]))
    return sortie


def essences(table: dict, items: dict) -> list[dict]:
    sortie = []
    for eid, e in table.items():
        cibles = {c: m for c, m in (e.get("mods") or {}).items() if m and c in CLASSES}
        if not cibles:
            continue
        nom = e["name"]
        description = ((items.get(eid) or {}).get("properties") or {}).get("description") or ""
        sortie.append(
            {
                "id": eid,
                "nom": nom,
                "rang": next((i + 1 for i, r in enumerate(RANGS) if nom.startswith(r + " ")), 0),
                "ilvl_max": e.get("item_level_restriction"),
                "sur_rare": "rare item" in description,
                "cibles": dict(sorted(cibles.items())),
            }
        )
    sortie.sort(key=lambda e: e["id"])
    return sortie


def mods(table: dict, garantis: set[str]) -> list[dict]:
    sortie = []
    for mid, m in table.items():
        if m.get("domain") != "item" or m.get("generation_type") not in ("prefix", "suffix"):
            continue
        spawn = [[s["tag"], int(s["weight"])] for s in m.get("spawn_weights") or []]
        if mid not in garantis and not any(w > 0 for _, w in spawn):
            continue
        sortie.append(
            {
                "id": mid,
                "nom": m.get("name") or "",
                "slot": m["generation_type"],
                "groupes": list(m.get("groups") or []),
                "type": m.get("type") or "",
                "niveau": int(m.get("required_level") or 1),
                "texte": (m.get("text") or "").replace("\r", ""),
                "spawn": spawn,
                "essence": bool(m.get("is_essence_only")),
            }
        )
    sortie.sort(key=lambda m: m["id"])
    return sortie


def construire(dossier: Path) -> dict:
    table_mods = lire_json(dossier, "mods.json")
    items = lire_json(dossier, "base_items.json")
    ess = essences(lire_json(dossier, "essences.json"), items)
    garantis = {m for e in ess for m in e["cibles"].values()}
    if absents := sorted(garantis - set(table_mods)):
        raise ValueError(f"mods garantis absents de mods.json : {', '.join(absents)}")
    version = (dossier / "version.txt").read_text(encoding="utf-8").strip()
    return {
        "schema": SCHEMA,
        "repoe_version": version,
        "classes": classes(lire_json(dossier, "item_classes.json")),
        "bases": bases(items, table_mods),
        "mods": mods(table_mods, garantis),
        "essences": ess,
    }


def ecrire(donnees: dict, sortie: Path) -> None:
    brut = json.dumps(donnees, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    tampon = io.BytesIO()
    with gzip.GzipFile(fileobj=tampon, mode="wb", compresslevel=9, mtime=0, filename="") as gz:
        gz.write(brut)
    sortie.write_bytes(tampon.getvalue())


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--depuis", type=Path, help="dossier des fichiers RePoE PoE 1 déjà téléchargés")
    p.add_argument("--out", type=Path, default=SORTIE)
    a = p.parse_args(argv)
    if a.depuis:
        dossier = a.depuis
    else:
        dossier = RACINE / "var" / "repoe-poe1"
        telecharger(dossier)
    d = construire(dossier)
    ecrire(d, a.out)
    print(
        f"{a.out} : RePoE PoE 1 {d['repoe_version']}, {len(d['bases'])} bases, "
        f"{len(d['mods'])} mods, {len(d['essences'])} Essences",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
