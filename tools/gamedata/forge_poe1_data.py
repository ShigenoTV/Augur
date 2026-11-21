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
  refont un objet Rare (Screaming et au-dessus) ou seulement un objet Normal ;
- les crafts d'établi qui ajoutent un mod (`crafting_bench_options.json`) : mod artisanal (domaine
  `crafted`, gardé dans les mods avec `artisanal`), classes autorisées et coût par nom de monnaie ;
- les fossiles (`fossils.json`) : multiplicateurs en pourcentage par étiquette de mod (0 = étiquette
  bloquée), plus Faceted (mods ajoutés), Hollow (mod forcé) et Sanctified (tirage chanceux), avec les
  mods `delve` qu'ils ajoutent ou forcent (`fossile`). Écartés : voir `fossiles()` ;
- les mods voilés (domaine `unveiled`, `voile`) : ceux qu'un dévoilement peut proposer après un
  Veiled Chaos Orb ou un Veiled Exalted Orb, avec leurs poids au dévoilement.

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
FICHIERS = (
    "mods.json",
    "base_items.json",
    "essences.json",
    "item_classes.json",
    "crafting_bench_options.json",
    "fossils.json",
    "version.txt",
)
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


def etabli(options: list, items: dict) -> list[dict]:
    sortie = []
    for o in options:
        mod = (o.get("actions") or {}).get("add_explicit_mod")
        classes = sorted(c for c in o.get("item_classes") or [] if c in CLASSES)
        if not mod or not classes:
            continue
        cout = {}
        for cid, n in (o.get("cost") or {}).items():
            nom = (items.get(cid) or {}).get("name")
            if not nom:
                raise ValueError(f"monnaie inconnue dans un craft d'établi : {cid}")
            cout[nom] = int(n)
        sortie.append({"mod": mod, "classes": classes, "cout": dict(sorted(cout.items()))})
    sortie.sort(key=lambda e: (e["mod"], e["classes"]))
    return sortie


def fossiles(table: dict, items: dict) -> list[dict]:
    """Fossiles gardés : ceux qui changent les poids par étiquette, plus trois fossiles à effet spécial
    que la Forge sait calculer (règles de l'émulateur Craft of Exile) :

    - Faceted : mods ajoutés au tirage (`ajoutes`, domaine `delve`), en plus de ses poids ;
    - Hollow : mod forcé (`forces`, Abyssal Socket), seulement sur les bases portant une étiquette
      de `permis` ;
    - Sanctified : tirage « chanceux » (`chanceux`) : poids de chaque mod × (1 + (niveau − 40) / 100).

    Écartés : Tangled (effets au hasard, Essence corrompue 1 fois sur 10), Glyphic (Essence corrompue,
    objet corrompu), Fractured (copie fracturée), Bloodstained (implicite corrompu), Gilded (prix de
    revente), Opulent (étiquette `drop`, qui annule aussi les mods sans étiquette).
    """
    sortie = []
    for fid, f in table.items():
        if (
            "DelveCrafting" not in fid
            or (items.get(fid) or {}).get("release_state") != "released"
            or f.get("forbidden_tags")
            or f.get("mirrors")
            or f.get("corrupted_essence_chance")
            or f.get("sell_price_mods")
        ):
            continue
        poids = [[x["tag"], int(x["weight"])] for x in f.get("positive_mod_weights") or []]
        poids += [[x["tag"], int(x["weight"])] for x in f.get("negative_mod_weights") or []]
        ajoutes = sorted(f.get("added_mods") or [])
        forces = sorted(f.get("forced_mods") or [])
        chanceux = bool(f.get("rolls_lucky"))
        if not (poids or forces or chanceux):
            continue
        # Opulent (étiquette `drop`) annule aussi les mods sans étiquette : effet à part, laissé de côté.
        if any(t == "drop" for t, _ in poids):
            continue
        sortie.append(
            {
                "id": fid,
                "nom": f["name"],
                "poids": sorted(poids),
                "ajoutes": ajoutes,
                "forces": forces,
                "permis": sorted(f.get("allowed_tags") or []),
                "chanceux": chanceux,
            }
        )
    sortie.sort(key=lambda f: f["nom"])
    return sortie


def mods(
    table: dict,
    garantis: set[str],
    artisanaux: set[str] = frozenset(),
    de_fossiles: set[str] = frozenset(),
) -> list[dict]:
    sortie = []
    for mid, m in table.items():
        if m.get("generation_type") not in ("prefix", "suffix"):
            continue
        # mods de PoE Royale (mode à part, retiré) : listés dans le domaine `item` mais jamais tirés en jeu
        if mid.rstrip("_").endswith("Royale"):
            continue
        artisanal = mid in artisanaux
        # mods du domaine `delve`, tirés ou forcés seulement par un fossile (Faceted, Hollow)
        fossile = mid in de_fossiles
        # mods voilés : proposés au dévoilement (Veiled Chaos Orb, Veiled Exalted Orb)
        voile = m.get("domain") == "unveiled"
        if m.get("domain") != "item" and not artisanal and not fossile and not voile:
            continue
        spawn = [[s["tag"], int(s["weight"])] for s in m.get("spawn_weights") or []]
        if artisanal:
            spawn = []
        elif mid not in garantis and not any(w > 0 for _, w in spawn):
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
                # Multiplicateurs de génération (en %, première étiquette reconnue) : mods d'attaque
                # à 50 % sur les dagues runiques, bâtons, sceptres et baguettes de sort…
                "generation": [
                    [g["tag"], int(g["weight"])] for g in m.get("generation_weights") or []
                ],
                "essence": bool(m.get("is_essence_only")),
                "artisanal": artisanal,
                "fossile": fossile,
                "voile": voile,
                # Étiquettes du mod : metamods « Cannot roll Attack/Caster Modifiers » et fossiles.
                "tags": sorted(m.get("implicit_tags") or []),
                # Première stat (min, max) : sert à classer un mod d'Essence ou d'établi parmi les tiers.
                "valeur": next(([int(x["min"]), int(x["max"])] for x in m.get("stats") or []), None),
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
    crafts = etabli(lire_json(dossier, "crafting_bench_options.json"), items)
    artisanaux = {c["mod"] for c in crafts}
    if absents := sorted(artisanaux - set(table_mods)):
        raise ValueError(f"mods d'établi absents de mods.json : {', '.join(absents)}")
    fos = fossiles(lire_json(dossier, "fossils.json"), items)
    de_fossiles = {m for f in fos for m in f["ajoutes"] + f["forces"]}
    if absents := sorted(de_fossiles - set(table_mods)):
        raise ValueError(f"mods de fossiles absents de mods.json : {', '.join(absents)}")
    version = (dossier / "version.txt").read_text(encoding="utf-8").strip()
    return {
        "schema": SCHEMA,
        "repoe_version": version,
        "classes": classes(lire_json(dossier, "item_classes.json")),
        "bases": bases(items, table_mods),
        "mods": mods(table_mods, garantis, artisanaux, de_fossiles),
        "essences": ess,
        "etabli": crafts,
        "fossiles": fos,
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
        f"{len(d['mods'])} mods, {len(d['essences'])} Essences, {len(d['etabli'])} crafts d'établi, "
        f"{len(d['fossiles'])} fossiles",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
