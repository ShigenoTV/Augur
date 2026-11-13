"""Tests de la commande de mise à jour des données (pytest, sans réseau)."""

from __future__ import annotations

import copy
import gzip
import json
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import mise_a_jour as maj
from test_extract_build_data import SKILLS, TREE

REPOE_TEST = HERE.parent / "testdata" / "repoe"
GAME_VERSIONS = (
    'treeVersionList = { "0_1", "0_4", "0_5" }\nlatestTreeVersion = treeVersionList[#treeVersionList]\n'
)


# --- Fonctions pures ----------------------------------------------------------


def test_versions_lues_sur_les_pages() -> None:
    assert maj.version_repoe("<title>RePoE - PoE2 version 0.5.5.8</title>") == "0.5.5.8"
    assert maj.derniere_version_arbre(GAME_VERSIONS) == "0_5"
    assert maj.version_coe('src="json/poe2/0.5.5.5/data.json?v=1"') == "0.5.5.5"
    with pytest.raises(maj.Echec):
        maj.version_repoe("<title>RePoE</title>")
    with pytest.raises(maj.Echec):
        maj.derniere_version_arbre("rien")


def test_differences_et_format() -> None:
    avant = {"a": {"b": 1, "c": [1.0, 2.0]}, "d": "x"}
    apres = {"a": {"b": 2, "c": [1.0, 2.0]}, "e": True}
    assert maj.differences(avant, apres) == [("a.b", 1, 2), ("d", "x", None), ("e", None, True)]
    assert maj.fmt([3.0, 13.5]) == "3–13.5"
    assert (
        maj.fmt([{"tag": "ring", "weight": 1000}, {"tag": "default", "weight": 0}]) == "ring 1000, default 0"
    )
    assert maj.fmt(None) == "—"
    c = maj.comparer({"1": 1, "2": 2, "10": 3}, {"2": 2, "10": 4, "3": 5})
    assert c == {"ajouts": ["3"], "retraits": ["1"], "modifs": ["10"]}


def _etat(**changements) -> dict:
    base = {
        "repoe": "0.5.5.7",
        "arbre_version": "0_5",
        "poids_date": "2026-11-05",
        "complement_date": "2026-11-08",
        "actives": {
            "ArcPlayer": {
                "nom": "Arc",
                "verifiee": True,
                "temps_incantation": 1.1,
                "niveaux": {
                    "19": {"degats": {"foudre": [18.0, 339.0]}},
                    "20": {"degats": {"foudre": [20.0, 386.0]}},
                },
            },
            "FireballPlayer": {"nom": "Fireball", "verifiee": False, "temps_incantation": 1.0, "niveaux": {}},
        },
        "supports": {"S": ["Supported Skills deal 10% more Damage"]},
        "armes": {},
        "passifs": {"200": ["12% increased Lightning Damage"]},
        "mods": {
            "IncreasedLife9": {
                "id": "IncreasedLife9",
                "nom": "Athlete's",
                "slot": "prefix",
                "texte": "+(120-149) to maximum Life",
            },
            "Ancien1": {"id": "Ancien1", "nom": "Old", "slot": "suffix", "texte": "old"},
        },
        "bases": {},
        "essences": {"e": {"id": "e", "nom": "Essence of Life", "cibles": {"Ring": ["Ancien1"]}}},
        "poids": {"IncreasedLife9": {"Rings": 1000}, "Ancien1": {"Rings": 500}},
    }
    etat = copy.deepcopy(base)
    etat.update(changements)
    return etat


def test_rapport_signale_ce_qui_compte() -> None:
    avant = _etat()
    apres = _etat(repoe="1.0.0.1", arbre_version="1_0")
    apres["actives"]["ArcPlayer"]["niveaux"]["20"]["degats"]["foudre"] = [22.0, 400.0]
    apres["actives"]["ArcPlayer"]["temps_incantation"] = 1.0
    apres["actives"]["NouveauPlayer"] = {"nom": "Nouveau", "verifiee": False, "niveaux": {}}
    del apres["actives"]["FireballPlayer"]
    apres["supports"]["S"] = ["Supported Skills deal 15% more Damage"]
    del apres["mods"]["Ancien1"]
    del apres["poids"]["Ancien1"]
    apres["mods"]["Nouveau1"] = {"id": "Nouveau1", "nom": "New", "slot": "prefix", "texte": "new"}

    texte, resume = maj.rapport(
        avant, apres, "2026-12-12", ["outil : ok"], ["crates/poe2-core/data/gemmes.json"]
    )

    assert "| RePoE (sorts, mods, bases) | 0.5.5.7 | 1.0.0.1 |" in texte
    assert "| Sorts et attaques | 2 | 2 | 1 | 1 | 1 |" in texte
    assert "- Nouveau (`NouveauPlayer`)" in texte
    assert "- Fireball (`FireballPlayer`)" in texte
    assert "temps_incantation : 1.1 → 1" in texte
    assert "1 niveau(x) modifié(s), niveau 20 : degats.foudre 20–386 → 22–400" in texte
    assert "retiré : Supported Skills deal 10% more Damage" in texte
    assert "ajouté : Supported Skills deal 15% more Damage" in texte
    attention = " ".join(resume["attention"])
    assert "Sort vérifié en jeu modifié ou retiré (Arc)" in attention
    assert "Nouvelle version d'arbre (0_5 → 1_0)" in attention
    assert "1 mod(s) de Forge sans poids connu" in attention and "`Nouveau1`" in attention
    assert "Mods retirés encore visés par une Essence : Ancien1" in attention
    assert "- `crates/poe2-core/data/gemmes.json`" in texte
    assert resume["comptes"]["mods"] == {"ajouts": 1, "retraits": 1, "modifs": 0}


def test_rapport_sans_changement() -> None:
    texte, resume = maj.rapport(_etat(), _etat(), "2026-11-13", fichiers=[])
    assert resume["attention"] == []
    assert "Aucun : les données sont déjà à jour." in texte
    assert texte.count("Aucun changement.") == 8


def test_longues_listes_tronquees() -> None:
    avant = _etat(passifs={})
    apres = _etat(passifs={str(i): [f"stat {i}"] for i in range(maj.LIMITE + 5)})
    texte, _ = maj.rapport(avant, apres, "2026-11-13")
    assert "- … et 5 autres" in texte


# --- Commande complète, sur un faux dépôt et un faux réseau ---------------------


def _faux_depot(racine: Path) -> None:
    donnees = racine / "tools" / "theorycraft" / "donnees"
    donnees.mkdir(parents=True)
    releve = {
        "date_releve": "2026-11-05",
        "pages": {"Rings": {"poids": {"FireResist5": 1000}, "inconnus": []}},
    }
    (donnees / "poids-poe2db.json").write_text(json.dumps(releve), encoding="utf-8")
    (donnees / "poids-coe.json").write_text(
        json.dumps({"date_releve": "2026-11-08", "pages": {}}), encoding="utf-8"
    )
    (donnees / "essences-coe.json").write_text(json.dumps({"essences": []}), encoding="utf-8")


def _faux_reseau(skills: dict, tree: dict, arbres: str = GAME_VERSIONS, repoe: str = "0.5.5.8"):
    fichiers = {
        maj.REPOE: f"<title>RePoE - PoE2 version {repoe}</title>".encode(),
        maj.REPOE + "skills.json": json.dumps(skills).encode(),
        maj.REPOE + "base_items.json": (REPOE_TEST / "base_items.min.json").read_bytes(),
        maj.REPOE + "base_items.min.json": (REPOE_TEST / "base_items.min.json").read_bytes(),
        maj.REPOE + "mods.min.json": (REPOE_TEST / "mods.min.json").read_bytes(),
        maj.POB + "GameVersions.lua": arbres.encode(),
    }
    demandes = []

    def lire(url: str) -> bytes:
        demandes.append(url)
        if url.startswith(maj.POB + "TreeData/"):
            return json.dumps(tree).encode()
        if url not in fichiers:
            raise maj.Echec(f"téléchargement impossible : {url}")
        return fichiers[url]

    return lire, demandes


def _embarques(racine: Path) -> dict[str, bytes]:
    return {c: (racine / c).read_bytes() for c in maj.EMBARQUES if (racine / c).exists()}


def test_commande_complete(tmp_path: Path) -> None:
    racine = tmp_path / "depot"
    _faux_depot(racine)
    cache = ["--cache", str(tmp_path / "sources"), "--date", "2026-12-12"]

    # Premier passage : données absentes, tout est créé.
    lire, demandes = _faux_reseau(SKILLS, TREE)
    rapport1 = tmp_path / "r1.md"
    assert maj.principal([*cache, "--rapport", str(rapport1)], lire, racine) == 0
    assert maj.POB + "TreeData/0_5/tree.json" in demandes
    forge = json.loads(gzip.decompress((racine / "crates/poe2-core/data/forge.json.gz").read_bytes()))
    assert forge["repoe_version"] == "0.5.5.8"
    assert json.loads((racine / "crates/poe2-core/data/arbre.json").read_text())["version"] == "0_5"
    premier = _embarques(racine)
    assert len(premier) == 3

    # Deuxième passage, mêmes sources : rien ne change, sortie déterministe.
    lire, _ = _faux_reseau(SKILLS, TREE)
    rapport2 = tmp_path / "r2.md"
    assert maj.principal([*cache, "--rapport", str(rapport2)], lire, racine) == 0
    assert _embarques(racine) == premier
    assert "Aucun : les données sont déjà à jour." in rapport2.read_text(encoding="utf-8")

    # Patch : Arc change, nouvelle version d'arbre avec un passif de plus. En essai, rien n'est écrit.
    skills = copy.deepcopy(SKILLS)
    skills["ArcPlayer"]["stat_sets"][0]["per_level"]["20"]["stats"][1] = {"value": 400}
    tree = copy.deepcopy(TREE)
    tree["nodes"]["40"] = {"stats": ["5% increased maximum Life"]}
    lire, demandes = _faux_reseau(skills, tree, GAME_VERSIONS.replace('"0_5" }', '"0_5", "1_0" }'), "1.0.0.1")
    rapport3 = tmp_path / "r3.md"
    assert maj.principal([*cache, "--essai", "--rapport", str(rapport3)], lire, racine) == 0
    assert maj.POB + "TreeData/1_0/tree.json" in demandes
    assert _embarques(racine) == premier
    texte = rapport3.read_text(encoding="utf-8")
    assert "| RePoE (sorts, mods, bases) | 0.5.5.8 | 1.0.0.1 |" in texte
    assert "Sort vérifié en jeu modifié ou retiré (Arc)" in texte
    assert "Nouvelle version d'arbre (0_5 → 1_0)" in texte
    assert "niveau 20 : degats.foudre 20–386 → 20–400" in texte
    assert "- `40` : 5% increased maximum Life" in texte

    # Sans --essai, depuis les sources déjà téléchargées (sans réseau) : les fichiers sont remplacés.
    rapport4 = tmp_path / "r4.md"
    hors_ligne = ["--date", "2026-12-12", "--depuis", str(tmp_path / "sources"), "--rapport", str(rapport4)]
    assert maj.principal(hors_ligne, lambda url: pytest.fail(url), racine) == 0
    assert json.loads((racine / "crates/poe2-core/data/arbre.json").read_text())["version"] == "1_0"
    assert _embarques(racine) != premier


def test_echec_ne_touche_pas_au_depot(tmp_path: Path, capsys) -> None:
    racine = tmp_path / "depot"
    _faux_depot(racine)
    sans_arc = {k: v for k, v in SKILLS.items() if k != "ArcPlayer"}
    lire, _ = _faux_reseau(sans_arc, TREE)
    argv = ["--cache", str(tmp_path / "sources"), "--rapport", str(tmp_path / "r.md")]
    assert maj.principal(argv, lire, racine) == 1
    assert _embarques(racine) == {}
    assert not (tmp_path / "r.md").exists()
    err = capsys.readouterr().err
    assert "sorts et arbre" in err and "Aucun fichier du dépôt n'a été modifié." in err


def test_source_indisponible(tmp_path: Path, capsys) -> None:
    racine = tmp_path / "depot"
    _faux_depot(racine)
    lire, _ = _faux_reseau(SKILLS, TREE)
    argv = ["--coe", "--cache", str(tmp_path / "sources"), "--rapport", str(tmp_path / "r.md")]
    assert maj.principal(argv, lire, racine) == 1
    assert "craftofexile" in capsys.readouterr().err
