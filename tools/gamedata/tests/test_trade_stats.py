"""Tests du relevé des stats du site de trade pour Forge (pytest, sans réseau)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import trade_stats as ts


def test_normaliser_plages_et_signes():
    assert ts.normaliser("+(10-19) to maximum Life") == "# to maximum life"
    assert ts.normaliser("Adds (3-4) to (5-8) Cold damage to Attacks") == "adds # to # cold damage to attacks"
    assert ts.normaliser("(-10--5)% to Chaos Resistance") == "#% to chaos resistance"
    assert ts.normaliser("+# to maximum Life") == "# to maximum life"
    assert ts.normaliser("15% increased Spirit") == "#% increased spirit"


def test_table_et_negate():
    ndjson = [
        json.dumps({"ref": "+# to maximum Life", "matchers": [{"string": "+# to maximum Life"}],
                    "trade": {"ids": {"explicit": ["explicit.stat_3299347043"]}}}),
        json.dumps({"ref": "#% increased Flask Charges used",
                    "matchers": [{"string": "#% reduced Flask Charges used", "negate": True}],
                    "trade": {"ids": {"explicit": ["explicit.stat_644456512"]}}}),
        json.dumps({"ref": "# Crafted Modifiers", "matchers": [{"string": "# Crafted Modifiers"}],
                    "trade": {"ids": {"pseudo": ["pseudo.x"]}}}),
        "",
    ]
    table = ts.table_trade(ndjson)
    assert table["# to maximum life"] == {"ids": ["explicit.stat_3299347043"]}
    assert table["#% reduced flask charges used"]["negate"] is True
    assert "# crafted modifiers" not in table
    sortie = ts.construire({"# to maximum life", "eternal iron hook"}, table)
    assert list(sortie["stats"]) == ["# to maximum life"]
    assert sortie["sans_stat"] == ["eternal iron hook"]
