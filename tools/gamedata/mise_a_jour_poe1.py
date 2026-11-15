#!/usr/bin/env python3
"""Mise à jour des données PoE 1 d'Augur en une commande, avec rapport des différences.

Pendant de `mise_a_jour.py` (PoE 2). Enchaîne, dans l'ordre :

1. la Forge PoE 1 : `forge_poe1_data.py` (RePoE PoE 1, https://repoe-fork.github.io/) ;
2. le paquet Path of Building PoE 1 embarqué : `tools/pob/paquet_pob_poe1.py` (`--pob <tag ou
   commit>` ; sans l'option, le paquet n'est pas touché) ;
3. les identifiants de stats du site de trade : `trade_stats_poe1.py` (depuis le paquet PoB) ;
4. un rapport Markdown : versions de RePoE et de PoB avant et après, mods, bases, Essences, fossiles
   et crafts d'établi ajoutés, retirés ou modifiés.

Les fichiers sont régénérés dans un dossier de travail et copiés dans le dépôt seulement si tout
a réussi. Avec `--essai`, seul le rapport est écrit.

Utilisation (depuis la racine du dépôt) :

    python tools/gamedata/mise_a_jour_poe1.py                       # Forge + stats de trade
    python tools/gamedata/mise_a_jour_poe1.py --pob v2.68.0         # + Path of Building
    python tools/gamedata/mise_a_jour_poe1.py --depuis DOSSIER      # RePoE déjà téléchargé
    python tools/gamedata/mise_a_jour_poe1.py --essai               # rapport seulement

Rapport par défaut : `var/mise-a-jour/rapport-poe1-AAAA-MM-JJ.md` (hors dépôt).

Python 3.11+, bibliothèque standard seulement (le paquet PoB demande `git`).
"""

from __future__ import annotations

import argparse
import datetime as dt
import gzip
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ICI = Path(__file__).resolve().parent
RACINE = ICI.parents[1]
sys.path.insert(0, str(ICI))

import forge_poe1_data
import trade_stats_poe1

FORGE = RACINE / "crates" / "poe2-core" / "data" / "forge-poe1.json.gz"
STATS = RACINE / "crates" / "poe2-core" / "data" / "trade-stats-poe1.json"
PAQUET = RACINE / "crates" / "pob-poe1" / "paquet"
LIMITE = 40


def aujourdhui() -> str:
    return dt.datetime.now(dt.UTC).date().isoformat()


def lire_forge(chemin: Path) -> dict:
    if not chemin.exists():
        return {}
    with gzip.open(chemin, "rt", encoding="utf-8") as f:
        return json.load(f)


def version_pob(dossier: Path) -> str:
    v = dossier / "VERSION"
    return v.read_text(encoding="utf-8").strip() if v.exists() else "?"


def index(liste: list, cle) -> dict:
    return {cle(x): x for x in liste}


def comparer_listes(avant: list, apres: list, cle) -> tuple[list, list, list]:
    a, b = index(avant, cle), index(apres, cle)
    ajoutes = sorted(set(b) - set(a))
    retires = sorted(set(a) - set(b))
    modifies = sorted(k for k in set(a) & set(b) if a[k] != b[k])
    return ajoutes, retires, modifies


def _cle(x) -> str:
    if isinstance(x, dict):
        for k in ("id", "nom", "mod"):
            if k in x:
                return str(x[k])
    return json.dumps(x, sort_keys=True, ensure_ascii=False)


def section(titre: str, avant: list, apres: list) -> list[str]:
    ajoutes, retires, modifies = comparer_listes(avant, apres, _cle)
    lignes = [
        f"## {titre}",
        "",
        f"{len(apres)} au total : {len(ajoutes)} ajoutés, {len(retires)} retirés, {len(modifies)} modifiés.",
        "",
    ]
    for nom, liste in (
        ("Ajoutés", ajoutes),
        ("Retirés", retires),
        ("Modifiés", modifies),
    ):
        if liste:
            lignes.append(
                f"**{nom}** : "
                + ", ".join(f"`{x}`" for x in liste[:LIMITE])
                + (f" et {len(liste) - LIMITE} autres" if len(liste) > LIMITE else "")
            )
            lignes.append("")
    return lignes


def rapport(
    avant: dict, apres: dict, pob: tuple[str, str], stats: tuple[dict, dict]
) -> str:
    l = [
        f"# Mise à jour des données PoE 1 ({aujourdhui()})",
        "",
        f"- RePoE PoE 1 : {avant.get('repoe_version', '?')} → {apres.get('repoe_version', '?')}",
        f"- Path of Building PoE 1 : {pob[0]} → {pob[1]}",
        (
            f"- Stats de trade reliées : {len(stats[0].get('stats', {}))} → {len(stats[1].get('stats', {}))}"
            f" ({len(stats[1].get('sans_stat', []))} lignes sans stat)"
        ),
        "",
    ]
    if pob[0] != pob[1]:
        l += [
            "> Nouvelle version de Path of Building : relancer `cargo test -p pob-poe1`, puis vérifier en jeu une comparaison d'objet et Alt+U.",
            "",
        ]
    for titre, cle in (
        ("Mods", "mods"),
        ("Bases", "bases"),
        ("Essences", "essences"),
        ("Fossiles", "fossiles"),
        ("Crafts d'établi", "etabli"),
    ):
        l += section(titre, avant.get(cle, []), apres.get(cle, []))
    return "\n".join(l) + "\n"


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument(
        "--depuis", type=Path, help="dossier des fichiers RePoE PoE 1 déjà téléchargés"
    )
    p.add_argument(
        "--pob", help="version de Path of Building PoE 1 à embarquer (tag ou commit)"
    )
    p.add_argument(
        "--essai", action="store_true", help="écrit le rapport sans toucher au dépôt"
    )
    p.add_argument("--rapport", type=Path)
    a = p.parse_args(argv)

    avant = lire_forge(FORGE)
    stats_avant = (
        json.loads(STATS.read_text(encoding="utf-8")) if STATS.exists() else {}
    )
    pob_avant = version_pob(PAQUET)
    with tempfile.TemporaryDirectory() as tmp:
        travail = Path(tmp)
        forge = travail / "forge-poe1.json.gz"
        args = ["--out", str(forge)] + (["--depuis", str(a.depuis)] if a.depuis else [])
        if forge_poe1_data.main(args) not in (0, None):
            sys.exit("forge_poe1_data.py a échoué : rien n'est écrit")
        paquet = PAQUET
        if a.pob:
            copie = travail / "paquet"
            shutil.copytree(PAQUET, copie)
            # Le script écrit dans le dépôt : on le lance sur une copie du dossier de sortie.
            subprocess.run(
                [
                    sys.executable,
                    str(RACINE / "tools" / "pob" / "paquet_pob_poe1.py"),
                    "--ref",
                    a.pob,
                    "--sortie",
                    str(copie),
                ],
                check=True,
            )
            paquet = copie
        table = trade_stats_poe1.lire_stats_pob(
            trade_stats_poe1.lua_du_paquet(paquet / "pob-poe1.tar.gz")
        )
        stats = trade_stats_poe1.construire(
            trade_stats_poe1.lignes_forge_poe1(forge), table
        )
        apres = lire_forge(forge)
        texte = rapport(
            avant, apres, (pob_avant, version_pob(paquet)), (stats_avant, stats)
        )
        sortie = (
            a.rapport
            or RACINE / "var" / "mise-a-jour" / f"rapport-poe1-{aujourdhui()}.md"
        )
        sortie.parent.mkdir(parents=True, exist_ok=True)
        sortie.write_text(texte, encoding="utf-8")
        if not a.essai:
            shutil.copyfile(forge, FORGE)
            STATS.write_text(
                json.dumps(stats, ensure_ascii=False, indent=1, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            if paquet != PAQUET:
                for f in ("pob-poe1.tar.gz", "VERSION"):
                    shutil.copyfile(paquet / f, PAQUET / f)
    print(f"Rapport : {sortie}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
