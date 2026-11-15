"""Tests du relevé des stats du site de trade de PoE 1 pour la Forge PoE 1 (pytest, sans réseau)."""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import trade_stats_poe1 as ts1

LUA = """
return {
	{
		["entries"] = {
			{
				["id"] = "pseudo.pseudo_total_life",
				["text"] = "+# total maximum Life",
				["type"] = "pseudo",
			},
			{
				["id"] = "explicit.stat_3299347043",
				["text"] = "+# to maximum Life",
				["type"] = "explicit",
			},
			{
				["id"] = "explicit.stat_1062208444",
				["text"] = "#% increased Armour (Local)",
				["type"] = "explicit",
			},
			{
				["id"] = "explicit.stat_2866361420",
				["text"] = "#% increased Armour",
				["type"] = "explicit",
			},
			{
				["id"] = "explicit.stat_4237190083",
				["text"] = "#% increased Mana Reservation Efficiency of Skills",
				["type"] = "explicit",
			},
		},
	},
}
"""


def test_lit_les_stats_explicites_et_locales():
    t = ts1.lire_stats_pob(LUA)
    assert t["# to maximum life"] == {"ids": ["explicit.stat_3299347043"]}
    assert t["#% increased armour"] == {
        "ids": ["explicit.stat_1062208444", "explicit.stat_2866361420"]
    }
    assert "# total maximum life" not in t


def test_forme_inverse():
    t = ts1.lire_stats_pob(LUA)
    sortie = ts1.construire(
        {
            "# to maximum life",
            "#% reduced mana reservation efficiency of skills",
            "inconnue",
        },
        t,
    )
    assert (
        sortie["stats"]["#% reduced mana reservation efficiency of skills"]["negate"]
        is True
    )
    assert sortie["sans_stat"] == ["inconnue"]


def test_paquet_et_donnees_du_depot():
    if not ts1.PAQUET.exists():
        return
    t = ts1.lire_stats_pob(ts1.lua_du_paquet(ts1.PAQUET))
    assert t["# to maximum life"]["ids"] == ["explicit.stat_3299347043"]
