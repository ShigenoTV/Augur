"""Tests du générateur des données embarquées par Forge (pytest, sans réseau)."""

from __future__ import annotations

import gzip
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import forge_data as fd


def _mod(
    id_: str, domaine: str = "item", essence: bool = False, poids: int = 1
) -> dict:
    return {
        "id": id_,
        "nom": id_,
        "domaine": domaine,
        "slot": "prefix",
        "groupe": "G",
        "groupes": ["G"],
        "type": "T",
        "niveau": 1,
        "texte": "",
        "spawn": [{"tag": "default", "weight": poids}],
        "stats": [{"id": "x", "min": 1, "max": 2}],
        "essence_seulement": essence,
    }


def test_filtre_et_champs():
    mods = {
        "schema": 1,
        "mods": [
            _mod("B"),
            _mod("A"),
            _mod("Desacralise", domaine="desecrated"),
            _mod("Essence", essence=True),
            _mod("Jamais", poids=0),
        ],
    }
    bases = {
        "schema": 1,
        "bases": [
            {
                "id": "b",
                "nom": "B",
                "classe": "Ring",
                "tags": [],
                "niveau_drop": 5,
                "implicites": [],
            }
        ],
    }
    d = fd.construire(mods, bases, {"pages": {}}, {"version_repoe": "0.5.5.7"})
    assert [m["id"] for m in d["mods"]["mods"]] == ["A", "B"]
    assert "stats" not in d["mods"]["mods"][0]
    assert d["bases"]["bases"] == [
        {"id": "b", "nom": "B", "classe": "Ring", "tags": [], "niveau_drop": 5}
    ]
    assert d["repoe_version"] == "0.5.5.7"


def test_sortie_deterministe(tmp_path: Path):
    d = {"schema": 1, "b": [2, 1], "a": "é"}
    a, b = tmp_path / "a.gz", tmp_path / "b.gz"
    fd.ecrire(d, a)
    fd.ecrire(d, b)
    assert a.read_bytes() == b.read_bytes()
    assert json.loads(gzip.decompress(a.read_bytes())) == d


def test_complement_dans_le_releve():
    mods = {"schema": 1, "mods": [_mod("A")]}
    bases = {"schema": 1, "bases": []}
    complement = {"source": "Craft of Exile", "pages": {"Daggers": {"poids": {"A": 500}}}}
    d = fd.construire(mods, bases, {"pages": {}}, {"version_repoe": "x"}, complement)
    assert d["releve"]["complement"]["pages"]["Daggers"]["poids"] == {"A": 500}
    sans = fd.construire(mods, bases, {"pages": {}}, {"version_repoe": "x"})
    assert "complement" not in sans["releve"]
