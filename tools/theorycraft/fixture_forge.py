#!/usr/bin/env python3
"""Cas de test croisés pour Forge : `craft_proba.py` (Python) contre le moteur Rust.

Pour une base réelle, écrit un fichier JSON avec :
- les mods de la base au format des tables d'Augur (`import_repoe.import_mods`) ;
- la base (`import_repoe.import_bases`) et la page de poids poe2db réduite à ces mods ;
- des scénarios (mods déjà présents, places libres) et la distribution du prochain mod
  calculée par `craft_proba.distribution_prochain_mod`.

Le test Rust `crates/poe2-core/tests/forge_croise.rs` reconstruit le pool avec
`poe2_core::forge::donnees::construire_pool` et doit retrouver ces probabilités.

Usage :
    python3 tools/theorycraft/fixture_forge.py REPOE_DIR "Nom de base" ILVL SORTIE.json
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ICI = Path(__file__).resolve().parent
sys.path.insert(0, str(ICI))
sys.path.insert(0, str(ICI.parents[0] / "gamedata"))

import craft_proba
import import_repoe

RELEVE = ICI / "donnees" / "poids-poe2db.json"


def page_de(base: dict) -> str:
    """Même correspondance que `page_poe2db` côté Rust (limitée aux classes des cas)."""
    pluriels = {"Body Armour": "Body_Armours", "Ring": "Rings", "Gloves": "Gloves"}
    nom = pluriels[base["classe"]]
    if base["classe"] in ("Body Armour", "Gloves"):
        attr = next(
            t[: -len("_armour")]
            for t in base["tags"]
            if t.endswith("_armour") and t != "body_armour"
        )
        nom += "_" + attr
    return nom


def scenarios(cands: dict, poids: dict) -> list[dict]:
    """Rare vide, un préfixe + un suffixe présents, trois préfixes (préfixes pleins)."""
    tires = sorted(k for k in cands if poids.get(k, 0) > 0)
    pref = [k for k in tires if cands[k]["generation_type"] == "prefix"]
    suff = [k for k in tires if cands[k]["generation_type"] == "suffix"]

    def groupes_distincts(liste, n):
        vus, res = set(), []
        for k in liste:
            g = cands[k]["groups"][0]
            if g not in vus:
                vus.add(g)
                res.append(k)
            if len(res) == n:
                break
        return res

    return [
        {"presents": []},
        {"presents": [pref[0], suff[0]]},
        {"presents": groupes_distincts(pref, 3)},
    ]


def principal(argv: list[str]) -> int:
    repoe, nom_base, ilvl, sortie = argv[0], argv[1], int(argv[2]), Path(argv[3])
    mods_bruts = json.loads(
        craft_proba._fichier_repoe(repoe, "mods").read_text(encoding="utf-8")
    )
    bases_brutes = json.loads(
        craft_proba._fichier_repoe(repoe, "base_items").read_text(encoding="utf-8")
    )
    base = next(
        b for b in import_repoe.import_bases(bases_brutes) if b["nom"] == nom_base
    )
    tags = set(base["tags"])
    # tous niveaux : les tiers se numérotent sur la famille entière
    cands_tous = craft_proba.candidats(mods_bruts, tags, 10_000)
    cands = craft_proba.candidats(mods_bruts, tags, ilvl)

    page_nom = page_de(base)
    releve = json.loads(RELEVE.read_text(encoding="utf-8"))
    page = releve["pages"][page_nom]
    poids = {k: v for k, v in page["poids"].items() if k in cands_tous}
    inconnus = sorted(k for k in page["inconnus"] if k in cands_tous)

    cas = []
    for sc in scenarios(cands, poids):
        presents = sc["presents"]
        pl = 3 - sum(cands[k]["generation_type"] == "prefix" for k in presents)
        sl = 3 - len(presents) + (3 - pl)
        avec_poids = {k: v for k, v in cands.items() if k in poids}
        d = craft_proba.distribution_prochain_mod(
            avec_poids, {k: float(v) for k, v in poids.items()}, presents, pl, sl
        )
        cas.append({"presents": presents, "attendu": dict(sorted(d.items()))})

    mods_table = import_repoe.import_mods(
        {k: v for k, v in mods_bruts.items() if k in cands_tous}
    )
    sortie.write_text(
        json.dumps(
            {
                "note": "Généré par tools/theorycraft/fixture_forge.py ; poids poe2db "
                "(estimations Krakenbul, CC BY-NC-SA 3.0, voir "
                "tools/theorycraft/donnees/LICENCE-poids-poe2db.md).",
                "ilvl": ilvl,
                "base": base,
                "mods": {"schema": import_repoe.SCHEMA_VERSION, "mods": mods_table},
                "releve": {
                    **{k: v for k, v in releve.items() if k != "pages"},
                    "pages": {page_nom: {"poids": poids, "inconnus": inconnus}},
                },
                "cas": cas,
            },
            ensure_ascii=False,
            indent=1,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"{sortie} : {len(mods_table)} mods, {len(poids)} poids, {len(cas)} cas")
    return 0


if __name__ == "__main__":
    raise SystemExit(principal(sys.argv[1:]))
