"""Tests du texte des mods reconstruit depuis les traductions de stats (pytest, sans réseau)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import import_repoe as ir
import traduction_stats as ts

REPOE = HERE.parent / "testdata" / "repoe"


@pytest.fixture(scope="module")
def traducteur() -> ts.Traducteur:
    return ts.Traducteur(json.loads((REPOE / "stat_descriptions.min.json").read_text(encoding="utf-8")))


def _texte(traducteur, *stats) -> str | None:
    t = traducteur.texte([{"id": i, "min": a, "max": b} for i, a, b in stats])
    return None if t is None else ir.clean_text(t)


def test_plages_signe_et_balises(traducteur) -> None:
    assert _texte(traducteur, ("base_maximum_life", 120, 149)) == "+(120-149) to maximum Life"
    assert _texte(traducteur, ("base_fire_damage_resistance_%", 21, 25)) == "+(21-25)% to Fire Resistance"
    assert _texte(traducteur, ("maximum_life_+%", 10, 10)) == "10% increased maximum Life"


def test_deux_stats_une_ligne(traducteur) -> None:
    texte = _texte(
        traducteur, ("attack_minimum_added_cold_damage", 1, 1), ("attack_maximum_added_cold_damage", 2, 3)
    )
    assert texte == "Adds 1 to (2-3) Cold damage to Attacks"


def test_variante_negative_garde_l_ordre_d_origine(traducteur) -> None:
    # Comme l'ancien champ `text` de RePoE : -10 à -8 devient « (10-8)% reduced ».
    assert _texte(traducteur, ("charm_charges_used_+%", -10, -8)) == "(10-8)% reduced Charm Charges used"


def test_conversion_par_minute(traducteur) -> None:
    texte = _texte(traducteur, ("life_regeneration_rate_per_minute_%", 30, 60))
    assert texte == "Regenerate (0.5-1)% of maximum Life per second"


def test_une_ligne_par_stat_dans_l_ordre_du_mod(traducteur) -> None:
    texte = _texte(traducteur, ("maximum_life_+%", 5, 5), ("base_maximum_life", 10, 20))
    assert texte == "5% increased maximum Life\n+(10-20) to maximum Life"


def test_stat_inconnue(traducteur) -> None:
    assert _texte(traducteur, ("stat_inexistante", 1, 2)) is None


def test_transformation_inconnue() -> None:
    table = [{"ids": ["x"], "English": [{"condition": [{}], "format": ["#"], "index_handlers": [["passive_hash"]], "string": "Allocates {0}"}]}]
    assert ts.Traducteur(table).texte([{"id": "x", "min": 1, "max": 1}]) is None


def test_import_sans_texte_repoe(traducteur) -> None:
    mods = json.loads((REPOE / "mods.min.json").read_text(encoding="utf-8"))
    bases = json.loads((REPOE / "base_items.min.json").read_text(encoding="utf-8"))
    avec = {m["id"]: m["texte"] for m in ir.import_mods(mods)}
    for m in mods.values():
        m["text"] = None
    # Sans traductions, l'import refuse au lieu de produire des mods muets.
    with pytest.raises(ir.RepoeError, match="--traductions"):
        ir.build_tables(mods, bases, "0.5.5.10")
    # Avec, le texte reconstruit est celui que donnait RePoE.
    reconstruit = {m["id"]: m["texte"] for m in ir.import_mods(mods, traducteur)}
    assert reconstruit == avec
    source = json.loads(ir.build_tables(mods, bases, "0.5.5.10", traducteur)["tables/source.json"])
    assert source["comptes"]["mods_sans_texte"] == 0
