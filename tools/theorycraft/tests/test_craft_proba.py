import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import craft_proba as c


def mod(groupe, genre, niveau=1, tags=("int_armour",), poids=1):
    return {
        "domain": "item",
        "generation_type": genre,
        "groups": [groupe],
        "required_level": niveau,
        "is_essence_only": False,
        "spawn_weights": [{"tag": t, "weight": poids} for t in tags]
        + [{"tag": "default", "weight": 0}],
    }


MODS = {
    "ES1": mod("ES", "prefix"),
    "VIE1": mod("VIE", "prefix"),
    "FEU1": mod("FEU", "suffix"),
    "FEU2": mod("FEU", "suffix", niveau=40),
    "FORCE1": mod("FORCE", "suffix", tags=("str_armour",)),
}
POIDS = {"ES1": 1000, "VIE1": 500, "FEU1": 1000, "FEU2": 500}


def test_eligibilite_tag_et_niveau():
    assert set(c.candidats(MODS, {"int_armour"}, 30)) == {"ES1", "VIE1", "FEU1"}
    assert set(c.candidats(MODS, {"int_armour"}, 40)) == {"ES1", "VIE1", "FEU1", "FEU2"}
    assert set(c.candidats(MODS, {"str_armour"}, 40)) == {"FORCE1"}


def test_distribution_somme_un_et_proportions():
    cands = c.candidats(MODS, {"int_armour"}, 30)
    d = c.distribution_prochain_mod(cands, POIDS)
    assert sum(d.values()) == pytest.approx(1)
    assert d["ES1"] == pytest.approx(1000 / 2500)
    assert c.proba_cible(d, ["ES1", "FEU1"]) == pytest.approx(0.8)


def test_exclusion_de_groupe_et_emplacements():
    cands = c.candidats(MODS, {"int_armour"}, 40)
    d = c.distribution_prochain_mod(cands, POIDS, presents=["FEU1"])
    assert "FEU2" not in d and "FEU1" not in d
    d = c.distribution_prochain_mod(cands, POIDS, prefixes_libres=0)
    assert set(d) == {"FEU1", "FEU2"}


def test_poids_manquant_leve_une_erreur():
    cands = c.candidats(MODS, {"int_armour"}, 30)
    with pytest.raises(KeyError):
        c.distribution_prochain_mod(cands, {"ES1": 1})


def test_esperance_geometrique():
    assert c.esperance_essais(0.25) == 4
    assert c.esperance_essais(0) == float("inf")


def test_monte_carlo_accord_avec_le_calcul_exact():
    cands = c.candidats(MODS, {"int_armour"}, 30)
    exact = c.proba_cible(c.distribution_prochain_mod(cands, POIDS), ["ES1"])
    mc = c.monte_carlo_ajouts(cands, POIDS, ["ES"], 1, essais=20000)
    assert mc == pytest.approx(exact, abs=0.015)
