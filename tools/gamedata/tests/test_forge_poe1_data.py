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
        "generation_weights": [{"tag": "twostonering", "weight": 50}],
        "is_essence_only": essence,
        "implicit_tags": ["resource", "life"],
    }


def _sources(tmp_path: Path) -> Path:
    mods = {
        "IncreasedLife1": _mod(),
        "Jamais": _mod(poids=0),
        "LifeEssence7": _mod(poids=0, essence=True),
        "Monstre": _mod(domaine="monster"),
        "JunMasterVeiledLife": _mod(domaine="unveiled"),
        "Implicite": {**_mod(generation="unique"), "text": "+(12-16)% to Fire and Lightning Resistances"},
        "EinharMasterIncreasedLife3": {
            **_mod(),
            "domain": "crafted",
            "spawn_weights": [],
            "stats": [{"id": "base_maximum_life", "min": 41, "max": 55}],
        },
    }
    bench = [
        {"actions": {"add_explicit_mod": "EinharMasterIncreasedLife3"}, "item_classes": ["Ring", "Flask"],
         "cost": {"Metadata/Items/Currency/CurrencyUpgradeToRare": 2}},
        {"actions": {"add_explicit_mod": None, "change_socket_count": 3}, "item_classes": ["Ring"], "cost": {}},
    ]
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
        "Metadata/Items/Currency/CurrencyUpgradeToRare": {"domain": "undefined", "name": "Regal Orb"},
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
    vide = {"added_mods": [], "forced_mods": [], "allowed_tags": [], "forbidden_tags": [], "rolls_lucky": False,
            "mirrors": False, "corrupted_essence_chance": 0, "positive_mod_weights": [], "negative_mod_weights": []}
    fossiles = {
        "Metadata/Items/Currency/CurrencyDelveCraftingLife": {
            **vide, "name": "Pristine Fossil",
            "positive_mod_weights": [{"tag": "life", "weight": 1000}],
            "negative_mod_weights": [{"tag": "defences", "weight": 0}],
        },
        "Metadata/Items/Currency/CurrencyDelveCraftingRandom": {
            **vide, "name": "Tangled Fossil", "added_mods": ["X"], "corrupted_essence_chance": 10,
        },
        "Metadata/Items/Currency/CurrencyDelveCraftingQuality": {
            **vide, "name": "Opulent Fossil", "positive_mod_weights": [{"tag": "drop", "weight": 1000}],
        },
        "Metadata/Items/Currency/CurrencyDelveCraftingAbyss": {
            **vide, "name": "Hollow Fossil", "forced_mods": ["DelveAbyssJewelSocket1"],
            "allowed_tags": ["weapon", "helmet"],
        },
        "Metadata/Items/Currency/CurrencyDelveCraftingLuckyModRolls": {
            **vide, "name": "Sanctified Fossil", "rolls_lucky": True,
        },
        "Metadata/Items/Currency/RandomFossilOutcome1": {**vide, "name": "Random"},
    }
    mods["DelveAbyssJewelSocket1"] = {**_mod(generation="suffix", domaine="delve"), "groups": ["AbyssJewelSocket"],
                                      "type": "AbyssJewelSocket", "text": "Has 1 Abyssal Socket"}
    for fid in list(fossiles)[:5]:
        items[fid] = {"domain": "undefined", "release_state": "released", "name": fossiles[fid]["name"]}
    classes = {c: {"influence_tags": []} for c in fd.CLASSES}
    classes["Ring"] = {"influence_tags": ["ring_shaper", "ring_basilisk", "ring_inconnu"]}
    for nom, contenu in {
        "mods.json": mods,
        "base_items.json": items,
        "essences.json": essences,
        "item_classes.json": classes,
        "crafting_bench_options.json": bench,
        "fossils.json": fossiles,
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
    # sans poids ni Essence ni établi : écarté ; le mod `delve` du Hollow Fossil est gardé
    # le mod voilé est gardé, avec ses poids au dévoilement
    assert ids == [
        "DelveAbyssJewelSocket1", "EinharMasterIncreasedLife3", "IncreasedLife1", "JunMasterVeiledLife", "LifeEssence7",
    ]
    assert d["mods"][0]["fossile"] and not d["mods"][2]["fossile"]
    voile = d["mods"].pop(3)
    assert voile["voile"] and voile["spawn"] == [["ring", 1000], ["default", 0]]
    assert not any(m["voile"] for m in d["mods"])
    d["mods"].pop(0)
    craft = d["mods"][0]
    assert craft["artisanal"] and craft["spawn"] == [] and craft["valeur"] == [41, 55]
    assert d["etabli"] == [{"mod": "EinharMasterIncreasedLife3", "classes": ["Ring"], "cout": {"Regal Orb": 2}}]
    assert d["mods"][1]["texte"] == "+(3-9) to maximum Life"
    assert d["mods"][1]["spawn"] == [["ring", 1000], ["default", 0]]
    assert d["mods"][1]["generation"] == [["twostonering", 50]]
    (e,) = d["essences"]
    assert e["rang"] == 7 and e["sur_rare"] and e["cibles"] == {"Ring": "LifeEssence7"}
    assert d["mods"][1]["tags"] == ["life", "resource"]
    # Fossiles à poids, Hollow (mod forcé) et Sanctified (chanceux) ; ni Tangled, ni Opulent, ni les issues.
    vide = {"ajoutes": [], "forces": [], "permis": [], "chanceux": False}
    assert d["fossiles"] == [
        {**vide, "id": "Metadata/Items/Currency/CurrencyDelveCraftingAbyss", "nom": "Hollow Fossil", "poids": [],
         "forces": ["DelveAbyssJewelSocket1"], "permis": ["helmet", "weapon"]},
        {**vide, "id": "Metadata/Items/Currency/CurrencyDelveCraftingLife", "nom": "Pristine Fossil",
         "poids": [["defences", 0], ["life", 1000]]},
        {**vide, "id": "Metadata/Items/Currency/CurrencyDelveCraftingLuckyModRolls", "nom": "Sanctified Fossil",
         "poids": [], "chanceux": True},
    ]


def test_sortie_deterministe(tmp_path):
    src = _sources(tmp_path)
    a, b = tmp_path / "a.json.gz", tmp_path / "b.json.gz"
    fd.ecrire(fd.construire(src), a)
    fd.ecrire(fd.construire(src), b)
    assert a.read_bytes() == b.read_bytes()
    assert json.loads(gzip.decompress(a.read_bytes()))["schema"] == fd.SCHEMA
