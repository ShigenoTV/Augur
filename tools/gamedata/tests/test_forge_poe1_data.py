"""Tests du générateur des données de la Forge PoE 1 (pytest, sans réseau)."""

from __future__ import annotations

import gzip
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import forge_poe1_data as fd


def _mod(generation: str = "prefix", poids: int = 1000, essence: bool = False, domaine: str = "item") -> dict:
    return {
        "domain": domaine,
        "generation_type": generation,
        "name": "Hale",
        "groups": ["IncreasedLife"],
        "type": "IncreasedLife",
        "required_level": 1,
        "text": "+(3-9) to maximum Life\r",
        "spawn_weights": [{"tag": "ring", "weight": poids}, {"tag": "default", "weight": 0}],
        "is_essence_only": essence,
    }


def _sources(tmp_path: Path) -> Path:
    mods = {
        "IncreasedLife1": _mod(),
        "Jamais": _mod(poids=0),
        "LifeEssence7": _mod(poids=0, essence=True),
        "Monstre": _mod(domaine="monster"),
        "Implicite": {**_mod(generation="unique"), "text": "+(12-16)% to Fire and Lightning Resistances"},
    }
    items = {
        "Metadata/Items/Rings/Ring12": {
            "domain": "item",
            "release_state": "released",
            "item_class": "Ring",
            "name": "Two-Stone Ring",
            "tags": ["twostonering", "ring", "default"],
            "drop_level": 20,
            "implicits": ["Implicite"],
        },
        "Metadata/Items/Rings/Cache": {
            "domain": "item",
            "release_state": "unreleased",
            "item_class": "Ring",
            "name": "Cachée",
            "tags": ["ring"],
            "drop_level": 1,
        },
        "Metadata/Items/Currency/CurrencyEssenceGreed7": {
            "domain": "undefined",
            "release_state": "released",
            "item_class": "StackableCurrency",
            "name": "Deafening Essence of Greed",
            "properties": {"description": "Upgrades a normal item to rare or reforges a rare item, guaranteeing one property"},
        },
    }
    essences = {
        "Metadata/Items/Currency/CurrencyEssenceGreed7": {
            "name": "Deafening Essence of Greed",
            "item_level_restriction": None,
            "mods": {"Ring": "LifeEssence7", "Flask": "Inconnu", "Amulet": None},
        },
        "Metadata/Items/Currency/CurrencyCorruptMonolith": {"name": "Remnant of Corruption", "mods": {"Ring": None}},
    }
    classes = {c: {"influence_tags": []} for c in fd.CLASSES}
    classes["Ring"] = {"influence_tags": ["ring_shaper", "ring_basilisk", "ring_inconnu"]}
    for nom, contenu in {
        "mods.json": mods,
        "base_items.json": items,
        "essences.json": essences,
        "item_classes.json": classes,
    }.items():
        (tmp_path / nom).write_text(json.dumps(contenu), encoding="utf-8")
    (tmp_path / "version.txt").write_text("3.29.3.6\n", encoding="utf-8")
    return tmp_path


def test_construit_les_donnees(tmp_path):
    d = fd.construire(_sources(tmp_path))
    assert d["repoe_version"] == "3.29.3.6"
    assert [b["nom"] for b in d["bases"]] == ["Two-Stone Ring"]
    assert d["bases"][0]["implicites"] == ["+(12-16)% to Fire and Lightning Resistances"]
    assert d["classes"]["Ring"]["influences"] == {"Hunter": "ring_basilisk", "Shaper": "ring_shaper"}
    ids = [m["id"] for m in d["mods"]]
    assert ids == ["IncreasedLife1", "LifeEssence7"]  # sans poids ni Essence : écarté
    assert d["mods"][0]["texte"] == "+(3-9) to maximum Life"
    assert d["mods"][0]["spawn"] == [["ring", 1000], ["default", 0]]
    (e,) = d["essences"]
    assert e["rang"] == 7 and e["sur_rare"] and e["cibles"] == {"Ring": "LifeEssence7"}


def test_sortie_deterministe(tmp_path):
    src = _sources(tmp_path)
    a, b = tmp_path / "a.json.gz", tmp_path / "b.json.gz"
    fd.ecrire(fd.construire(src), a)
    fd.ecrire(fd.construire(src), b)
    assert a.read_bytes() == b.read_bytes()
    assert json.loads(gzip.decompress(a.read_bytes()))["schema"] == fd.SCHEMA
