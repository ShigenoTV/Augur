import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coe_controle as k
import coe_essences as es
import coe_poids as c

# Fixture fictive au format du fichier de données de Craft of Exile.
LANGUE = ["Rings", "Gloves (STR)", "Gloves (BASE)", "Gold Ring", "Brass Gloves"]
COE = {
    "classes": {
        "entries": [
            {"id": 1, "label": 0},
            {"id": 2, "label": 1},
            {"id": 3, "label": 2},
        ]
    },
    "items": {"entries": [{"class": 1, "label": 3}, {"class": 2, "label": 4}]},
    "modgroups": {
        "entries": [
            {"id": 10, "type": 1, "domain": 1, "influence": 6},
            {"id": 11, "type": 2, "domain": 1, "influence": 6},
            {"id": 12, "type": 2, "domain": 1, "influence": 1002},
        ]
    },
    "mods": {
        "entries": [
            {"id": 1, "key": "Life1", "group": 10, "minlvl": 1},
            {"id": 2, "key": "Cast1", "group": 11, "minlvl": 5},
            {"id": 3, "key": "Cast2", "group": 11, "minlvl": 60},
            {"id": 4, "key": "Inconnu", "group": 11, "minlvl": 1},
            {"id": 5, "key": "Influence", "group": 12, "minlvl": 1},
        ]
    },
    "classmods": {
        "1": {"1": 1000, "2": 500, "3": 250, "4": 1, "5": 800},
        "2": {"1": 1000},
        "3": {"1": 1000},
    },
}


def test_page_de():
    assert c.page_de("Rings") == "Rings"
    assert c.page_de("Gloves (STR)") == "Gloves_str"
    assert c.page_de("Body Armours (STR/DEX/INT)") == "Body_Armours_str_dex_int"
    assert c.page_de("Gloves (BASE)") is None
    assert c.page_de("Wands (Sceptre)") is None


def test_completer_ne_garde_que_les_poids_absents_de_poe2db():
    poe2db = {"pages": {"Rings": {"poids": {"Life1": 1000}}}}
    pages, communs = c.completer(COE, LANGUE, poe2db)
    assert communs == 1
    # poids de 1 = non mesuré, ignoré ; page (BASE) ignorée
    assert pages["Rings"]["poids"] == {"Cast1": 500, "Cast2": 250, "Influence": 800}
    assert pages["Gloves_str"]["poids"] == {"Life1": 1000}
    assert "Gloves_base" not in pages


def test_completer_refuse_un_poids_different():
    poe2db = {"pages": {"Rings": {"poids": {"Life1": 999}}}}
    with pytest.raises(ValueError, match="Life1"):
        c.completer(COE, LANGUE, poe2db)


def test_controle_identique_et_ecart():
    ok = {"nom": "Gold Ring", "ilvl": 10, "mods": [["Life1", "Prefix", 1000], ["Cast1", "Suffix", 500]]}
    assert k.controler(COE, LANGUE, [ok]) == []
    # au niveau 60, Cast2 manque côté Forge
    haut = dict(ok, ilvl=60)
    (e,) = k.controler(COE, LANGUE, [haut])
    assert "Cast" in e
    (e,) = k.controler(COE, LANGUE, [dict(ok, nom="Base inconnue")])
    assert "absente" in e


def test_ecartes_par_niveau_minimum():
    coe = {
        "mods": {
            "entries": [
                {"id": 1, "key": "Bas", "group": 1, "minlvl": 10},
                {"id": 2, "key": "Haut", "group": 1, "minlvl": 60},
                {"id": 3, "key": "Seul", "group": 2, "minlvl": 20},
            ]
        },
        "classmods": {"7": {"1": 100, "2": 100, "3": 100}},
    }
    # Bas a un tier accessible plus haut : écarté ; Seul est le meilleur tier de son groupe : gardé
    assert k.ecartes_coe(coe, 7, 80, 35) == {"Bas"}
    # à l'ilvl 50, Haut (niv. 60) est inaccessible : Bas redevient le meilleur tier
    assert k.ecartes_coe(coe, 7, 50, 35) == set()


def test_essences_une_cible_par_classe():
    langue = ["Rings", "Gloves (STR)", "Wands (Fire)", "Essence of the Body", "Essence of Battle", "Runic Alloy"]
    coe = {
        "classes": {
            "entries": [
                {"id": 1, "label": 0, "class": 5},
                {"id": 2, "label": 1, "class": 22},
                {"id": 3, "label": 2, "class": 8},
            ]
        },
        "mods": {"entries": [{"id": 1, "key": "Life1"}, {"id": 2, "key": "Acc1"}, {"id": 3, "key": "Acc2"}]},
        "essences": {
            "entries": [
                {"id": 0, "label": 3, "type": 3},
                {"id": 1, "label": 4, "type": 2},
                {"id": 2, "label": 5, "type": 5},
            ],
            "byessences": {
                "0": {"5": [1], "22": [1], "99": [1]},
                "1": {"8": [2, 3]},
                "2": {"5": [1]},
            },
        },
    }
    liste, multiples = es.essences(coe, langue)
    assert multiples == 1
    assert liste == [
        {
            "id": "essence-of-the-body",
            "nom": "Essence of the Body",
            "type": "perfect",
            "cibles": {"Gloves": "Life1", "Ring": "Life1"},
        }
    ]
    assert es.slug("The Runefather's Alloy") == "the-runefather-s-alloy"
