"""Tests de l'extraction des données de comparaison (pytest, sans réseau)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import extract_build_data as eb

SKILLS = {
    "LightningBoltPlayer": {
        "cast_time": 850,
        "is_support": False,
        "stat_sets": [
            {
                "static": {
                    "crit_chance": 1000,
                    "stats": [
                        {"id": "spell_minimum_base_lightning_damage", "type": "float"},
                        {"id": "spell_maximum_base_lightning_damage", "type": "float"},
                    ],
                },
                "per_level": {
                    "20": {"stats": [{"value": 30}, {"value": 565}]},
                    "2": {"stats": [{"value": 2}, {"value": 9}]},
                },
            }
        ],
    },
    "ArcPlayer": {
        "cast_time": 1100,
        "is_support": False,
        "stat_sets": [
            {
                "static": {
                    "crit_chance": 900,
                    "quality_stats": [{"stats": {"number_of_chains": 100}}],
                    "stats": [
                        {"id": "spell_minimum_base_lightning_damage", "type": "float"},
                        {"id": "spell_maximum_base_lightning_damage", "type": "float"},
                        {"id": "arc_damage_+%_final_from_infusion_consumption", "value": 200},
                        {"id": "arc_chains_gained_from_infusion_consumption", "value": 6},
                        {"id": "number_of_chains", "type": "additional"},
                    ],
                },
                "per_level": {
                    "20": {"stats": [{"value": 20}, {"value": 386}, None, None, {"value": 9}]}
                },
            }
        ],
    },
    "FireballPlayer": {
        "cast_time": 1000,
        "is_support": False,
        "active_skill": {"display_name": "Fireball", "types": ["Spell", "Projectile", "Fire"]},
        "stat_sets": [
            {
                "static": {
                    "crit_chance": 900,
                    "stats": [
                        {"id": "spell_minimum_base_fire_damage", "type": "float"},
                        {"id": "spell_maximum_base_fire_damage", "type": "float"},
                    ],
                },
                "per_level": {"1": {"stats": [{"value": 4}, {"value": 7}]}},
            }
        ],
    },
    # Écartés : canalisé (sans temps d'incantation), sort sans dégâts, interne.
    "IncinerateChannelPlayer": {
        "cast_time": 0,
        "active_skill": {"display_name": "Burn", "types": ["Spell", "Fire"]},
        "stat_sets": [],
    },
    "BlinkPlayer": {
        "cast_time": 500,
        "active_skill": {"display_name": "Blink", "types": ["Spell"]},
        "stat_sets": [{"static": {"stats": []}, "per_level": {"1": {"stats": []}}}],
    },
    "GSShrineFirePlayer": {
        "cast_time": 500,
        "active_skill": {"display_name": "", "types": ["Spell"]},
        "stat_sets": [],
    },
    "LightningArrowPlayer": {
        "cast_time": 1000,
        "is_support": False,
        "static": {"attack_speed_multiplier": -10},
        "active_skill": {"display_name": "Lightning Arrow", "types": ["Attack", "Projectile", "Lightning"]},
        "stat_sets": [
            {
                "static": {
                    "stats": [
                        {
                            "id": "active_skill_base_physical_damage_%_to_convert_to_lightning",
                            "type": "constant",
                            "value": 80,
                        }
                    ]
                },
                "per_level": {"1": {}, "20": {"damage_multiplier": 250}},
            }
        ],
    },
    "SupportZenithPlayerTwo": {
        "is_support": True,
        "stat_sets": [
            {
                "static": {
                    "stat_text": {
                        "x": "Supported [Spell|Spells] deal 30% more Damage while you are above 90% of your Maximum Mana"
                    }
                },
                "per_level": {},
            }
        ],
    },
}

TREE = {
    "nodes": {
        "10": {"name": "Start"},
        "200": {"stats": ["12% increased [Lightning] Damage"]},
        "30": {"stats": ["3% increased Cast Speed"]},
    }
}


def test_extraction(tmp_path: Path) -> None:
    skills = tmp_path / "skills.json"
    tree = tmp_path / "tree.json"
    skills.write_text(json.dumps(SKILLS), encoding="utf-8")
    tree.write_text(json.dumps(TREE), encoding="utf-8")
    out = tmp_path / "out"
    args = ["--skills", str(skills), "--tree", str(tree), "--tree-version", "0_5", "--out", str(out)]
    assert eb.main(args) == 0
    g = json.loads((out / "gemmes.json").read_text(encoding="utf-8"))
    lb = g["actives"]["LightningBoltPlayer"]
    assert lb["temps_incantation"] == 0.85 and lb["critique_base"] == 10
    assert lb["niveaux"]["20"] == {"degats": {"foudre": [30.0, 565.0]}}
    assert lb["verifiee"] is True
    arc = g["actives"]["ArcPlayer"]
    assert arc["niveaux"]["20"] == {"degats": {"foudre": [20.0, 386.0]}, "chaines": 9.0}
    assert arc["chaines_par_qualite"] == 0.1
    assert arc["infusion"] == {"element": "foudre", "degats_more": 200, "chaines": 6}
    assert sorted(g["actives"]) == ["ArcPlayer", "FireballPlayer", "LightningArrowPlayer", "LightningBoltPlayer"]
    la = g["actives"]["LightningArrowPlayer"]
    assert la["genre"] == "attaque" and la["vitesse_attaque"] == 90
    assert la["niveaux"]["1"]["multiplicateur"] == 100 and la["niveaux"]["20"]["multiplicateur"] == 250
    assert la["conversion"] == {"physique": {"foudre": 80.0}}
    fb = g["actives"]["FireballPlayer"]
    assert fb["verifiee"] is False and fb["elements"] == ["feu"]
    assert fb["niveaux"]["1"] == {"degats": {"feu": [4.0, 7.0]}}
    assert g["supports"]["SupportZenithPlayerTwo"] == [
        "Supported Spells deal 30% more Damage while you are above 90% of your Maximum Mana"
    ]
    a = json.loads((out / "arbre.json").read_text(encoding="utf-8"))
    assert a["version"] == "0_5"
    assert sorted(a["passifs"]) == ["200", "30"]  # nœuds sans stats écartés
    assert a["passifs"]["200"] == ["12% increased Lightning Damage"]
    # Sortie déterministe.
    first = (out / "gemmes.json").read_bytes()
    assert eb.main(args) == 0
    assert (out / "gemmes.json").read_bytes() == first


def test_armes() -> None:
    bases = {
        "Metadata/Items/Weapons/TwoHandWeapons/Bows/FourBow6": {
            "name": "Gemini Bow",
            "item_class": "Bow",
            "properties": {
                "attack_time": 870,
                "critical_strike_chance": 500,
                "physical_damage_min": 39,
                "physical_damage_max": 72,
            },
        },
        "Metadata/Items/Rings/Ring1": {"name": "Iron Ring", "item_class": "Ring", "properties": {}},
    }
    assert eb.armes(bases) == {
        "Gemini Bow": {"classe": "Bows", "physique": [39.0, 72.0], "temps_attaque": 0.87, "critique": 5.0}
    }


def test_competence_absente(tmp_path: Path) -> None:
    skills = tmp_path / "skills.json"
    tree = tmp_path / "tree.json"
    skills.write_text("{}", encoding="utf-8")
    tree.write_text(json.dumps(TREE), encoding="utf-8")
    args = ["--skills", str(skills), "--tree", str(tree), "--tree-version", "0_5", "--out", str(tmp_path)]
    assert eb.main(args) == 1
