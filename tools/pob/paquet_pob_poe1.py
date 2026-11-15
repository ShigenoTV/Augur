"""Paquet Path of Building (PoE 1) embarqué dans Augur.

Augur fait tourner le moteur de calcul de Path of Building Community (PoE 1,
licence MIT) sans fenêtre, pour dire si un objet copié en jeu est mieux que celui
porté. Ce script prépare le paquet allégé que la crate `pob-poe1` embarque :

- le code Lua de PoB (`src/`) et ses bibliothèques Lua (`runtime/lua/`) ;
- les données du jeu (`src/Data/`), sans les tables des Timeless Jewels (170 Mo,
  téléchargées à part par PoB ; seuls leurs index `.lua` sont gardés) ;
- seulement l'arbre passif le plus récent (`tree.lua`, `sprites.lua`) : un build
  sur un arbre plus ancien est recalé sur le dernier par Augur ;
- la licence MIT de PoB, livrée avec le paquet.

Usage : python tools/pob/paquet_pob_poe1.py [--ref <tag ou commit>]

La version par défaut est un commit de la branche `dev` : la dernière version
publiée (2.67.2, août 2026) ne connaît pas encore l'arbre de la 3.30.
Sortie : crates/pob-poe1/paquet/pob-poe1.tar.gz et crates/pob-poe1/paquet/VERSION
"""

from __future__ import annotations

import argparse
import gzip
import io
import re
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

DEPOT = "https://github.com/PathOfBuildingCommunity/PathOfBuilding.git"
# Branche dev du 14/11/2026 (arbre 3.30 Covens of Umbra).
REF_PAR_DEFAUT = "303c104ac4dbc116b99f36c87e6a4ad27640d719"
RACINE = Path(__file__).resolve().parents[2]
SORTIE = RACINE / "crates" / "pob-poe1" / "paquet"

# Dossiers de src/ inutiles sans fenêtre (images, outils d'export des données).
EXCLUS = {"Export", "Assets", "TreeData"}


def git(*args: str, cwd: Path | None = None) -> str:
    return subprocess.run(
        ["git", *args], cwd=cwd, check=True, capture_output=True, text=True
    ).stdout


def dernier_arbre(src: Path) -> str:
    texte = (src / "GameVersions.lua").read_text(encoding="utf-8")
    liste = re.search(r"treeVersionList\s*=\s*\{(.*?)\}", texte, re.DOTALL)
    if not liste:
        sys.exit("treeVersionList introuvable dans GameVersions.lua")
    versions = re.findall(r'"([^"]+)"', liste.group(1))
    return versions[-1]


def fichiers(depot: Path) -> list[tuple[Path, str]]:
    """(chemin sur disque, chemin dans le paquet), triés pour un paquet reproductible."""
    src = depot / "src"
    arbre = dernier_arbre(src)
    choisis: list[tuple[Path, str]] = []
    for f in sorted(src.rglob("*")):
        if not f.is_file():
            continue
        rel = f.relative_to(src)
        haut = rel.parts[0]
        if haut in EXCLUS:
            continue
        if rel.parts[:2] == ("Data", "TimelessJewelData") and f.suffix != ".lua":
            continue
        choisis.append((f, f"src/{rel.as_posix()}"))
    for nom in ("tree.lua", "sprites.lua"):
        f = src / "TreeData" / arbre / nom
        if f.exists():
            choisis.append((f, f"src/TreeData/{arbre}/{nom}"))
    # Fichiers d'arbre lus quel que soit l'arbre du build.
    for f in sorted((src / "TreeData" / "legion").glob("*.lua")):
        choisis.append((f, f"src/TreeData/legion/{f.name}"))
    choisis.append(
        (src / "TreeData" / "3_19" / "Assets.lua", "src/TreeData/3_19/Assets.lua")
    )
    for f in sorted((depot / "runtime" / "lua").rglob("*")):
        if f.is_file():
            choisis.append(
                (
                    f,
                    f"runtime/lua/{f.relative_to(depot / 'runtime' / 'lua').as_posix()}",
                )
            )
    choisis.append((depot / "LICENSE.md", "LICENSE.md"))
    choisis.append((depot / "manifest.xml", "manifest.xml"))
    return choisis


def paquet(depot: Path) -> bytes:
    brut = io.BytesIO()
    with tarfile.open(fileobj=brut, mode="w", format=tarfile.PAX_FORMAT) as tar:
        for chemin, nom in fichiers(depot):
            info = tarfile.TarInfo(nom)
            donnees = chemin.read_bytes()
            info.size = len(donnees)
            info.mtime = 0
            info.mode = 0o644
            tar.addfile(info, io.BytesIO(donnees))
    return gzip.compress(brut.getvalue(), compresslevel=9, mtime=0)


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument(
        "--ref",
        default=REF_PAR_DEFAUT,
        help="version de Path of Building (tag ou commit git)",
    )
    p.add_argument(
        "--depot",
        type=Path,
        help="clone local de PoB déjà à la bonne version (sinon clone)",
    )
    args = p.parse_args()
    with tempfile.TemporaryDirectory() as tmp:
        depot = args.depot
        if depot is None:
            depot = Path(tmp) / "pob"
            depot.mkdir()
            git("init", "-q", cwd=depot)
            git("remote", "add", "origin", DEPOT, cwd=depot)
            git("config", "core.sparseCheckout", "true", cwd=depot)
            git(
                "sparse-checkout",
                "set",
                "--no-cone",
                "/src/*",
                "!/src/Export/*",
                "!/src/Assets/*",
                "!/src/TreeData/*",
                "/src/TreeData/legion/*",
                "/src/TreeData/3_19/Assets.lua",
                "/src/TreeData/*/tree.lua",
                "/src/TreeData/*/sprites.lua",
                "!/src/Data/TimelessJewelData/*",
                "/src/Data/TimelessJewelData/*.lua",
                "/runtime/lua/*",
                "/LICENSE.md",
                "/manifest.xml",
                cwd=depot,
            )
            git(
                "fetch",
                "-q",
                "--depth",
                "1",
                "--filter=blob:none",
                "origin",
                args.ref,
                cwd=depot,
            )
            git("checkout", "-q", "FETCH_HEAD", cwd=depot)
        contenu = paquet(depot)
        commit = git("rev-parse", "HEAD", cwd=depot).strip()
        version = re.search(
            r'Version number="([^"]+)"',
            (depot / "manifest.xml").read_text(encoding="utf-8"),
        )
        arbre = dernier_arbre(depot / "src")
    SORTIE.mkdir(parents=True, exist_ok=True)
    (SORTIE / "pob-poe1.tar.gz").write_bytes(contenu)
    etiquette = f"{version.group(1) if version else '?'}+{commit[:7]} (arbre {arbre.replace('_', '.')})"
    (SORTIE / "VERSION").write_text(etiquette + "\n", encoding="utf-8")
    print(
        f"pob-poe1.tar.gz : {len(contenu) / 1e6:.1f} Mo, Path of Building {etiquette}"
    )


if __name__ == "__main__":
    main()
