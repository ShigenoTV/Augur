#!/usr/bin/env python3
"""Poids de mods de Craft of Exile pour compléter le relevé poe2db.

Craft of Exile (https://www.craftofexile.com/?game=poe2) publie les poids de génération des mods de
PoE 2 dans le fichier de données de son émulateur (`json/poe2/<version>/data.json`, préfixé
`coedata=`) : `classmods` donne, pour chaque classe d'objet, `{id de mod: poids}`, et `mods[].key`
est l'identifiant RePoE du mod. Un poids de 1 y est un marqueur « non mesuré », comme chez poe2db.

Comparaison du 08/11/2026 : les 6 434 poids poe2db sont identiques à ceux de Craft of Exile (même
source, les estimations de Krakenbul). Craft of Exile en connaît 1 311 de plus. Décision de Max
(08/11/2026, carte) : les copier dans le dépôt, avec leur attribution
(`donnees/LICENCE-poids-coe.md`), **seulement pour les mods sans poids chez poe2db**. poe2db reste
la source principale.

    curl -sS -o data.json "https://beta.craftofexile.com/json/poe2/0.5.5.5/data.json"
    curl -sS -o english.json "https://beta.craftofexile.com/json/poe2/0.5.5.5/localization/english.json"
    python3 tools/theorycraft/coe_poids.py --data data.json --langue english.json \\
        --poe2db tools/theorycraft/donnees/poids-poe2db.json \\
        --date 2026-11-08 --out tools/theorycraft/donnees/poids-coe.json

Le script s'arrête si un poids connu des deux sources diffère : les données ne sont plus
cohérentes, il faut regarder avant de compléter.

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

SOURCE = (
    "Craft of Exile (https://www.craftofexile.com/?game=poe2), données de l'émulateur PoE 2 ; "
    "poids estimés par Krakenbul (Prohibited Library)"
)

ARMURES = {
    "Body Armours": "Body_Armours",
    "Boots": "Boots",
    "Gloves": "Gloves",
    "Helmets": "Helmets",
    "Shields": "Shields",
}


def lire_js(chemin: Path):
    """Fichier JSON éventuellement préfixé `nom=` et terminé par `;`."""
    texte = chemin.read_text(encoding="utf-8").strip()
    if not texte.startswith(("{", "[")):
        texte = texte[texte.index("=") + 1 :]
    return json.loads(texte.rstrip(";"))


def page_de(classe: str) -> str | None:
    """Nom de page poe2db d'une classe Craft of Exile ; `None` si elle n'en a pas."""
    if "(" in classe:
        nom, attrs = classe[:-1].split(" (")
        if nom not in ARMURES or attrs == "BASE":
            return None  # armures sans attribut, baguettes et bâtons spécialisés
        return f"{ARMURES[nom]}_{attrs.lower().replace('/', '_')}"
    return classe.replace(" ", "_")


def completer(coe: dict, langue: list, poe2db: dict) -> tuple[dict, int]:
    """Pages `{page: {"poids": {mod: poids}}}` des poids que poe2db n'a pas, et le nombre de poids
    connus des deux sources (tous identiques, sinon `ValueError`)."""
    cle = {str(m["id"]): m["key"] for m in coe["mods"]["entries"]}
    pages: dict[str, dict] = {}
    communs = 0
    for c in coe["classes"]["entries"]:
        page = page_de(langue[c["label"]])
        poids_coe = coe["classmods"].get(str(c["id"]), {})
        if page is None or not poids_coe:
            continue
        connus = poe2db["pages"].get(page, {}).get("poids", {})
        ajout = {}
        for id_coe, w in poids_coe.items():
            mod = cle.get(id_coe)
            if mod is None or w <= 1:
                continue
            if mod in connus:
                if connus[mod] != w:
                    raise ValueError(f"{page} {mod} : poe2db {connus[mod]}, Craft of Exile {w}")
                communs += 1
            else:
                ajout[mod] = w
        if ajout:
            pages[page] = {"poids": dict(sorted(ajout.items()))}
    return dict(sorted(pages.items())), communs


def principal(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--data", type=Path, required=True)
    p.add_argument("--langue", type=Path, required=True)
    p.add_argument("--poe2db", type=Path, required=True)
    p.add_argument("--date", required=True)
    p.add_argument("--version", default="0.5.5.5")
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args(argv)
    poe2db = json.loads(a.poe2db.read_text(encoding="utf-8"))
    pages, communs = completer(lire_js(a.data), lire_js(a.langue), poe2db)
    sortie = {
        "source": SOURCE,
        "licence": "données tierces, voir LICENCE-poids-coe.md",
        "date_releve": a.date,
        "version_donnees": a.version,
        "communs_identiques": communs,
        "pages": pages,
    }
    a.out.write_text(
        json.dumps(sortie, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8"
    )
    total = sum(len(v["poids"]) for v in pages.values())
    print(f"{a.out} : {total} poids ajoutés sur {len(pages)} pages ; {communs} poids communs identiques")
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
