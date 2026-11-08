import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import coe_controle as k
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
