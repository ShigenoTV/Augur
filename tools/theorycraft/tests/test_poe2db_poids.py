import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import craft_proba as c
import poe2db_poids as p

# Fixture écrite à la main sur le modèle d'une page poe2db (pas de données
# poe2db versées au dépôt : les valeurs sont fictives).
REPOE = {
    "Int1": {
        "name": "of the Pupil",
        "required_level": 1,
        "generation_type": "suffix",
        "groups": ["Intelligence"],
        "spawn_weights": [
            {"tag": "int_armour", "weight": 1},
            {"tag": "default", "weight": 0},
        ],
        "text": "+(5-8) to [Intelligence|Intelligence]",
    },
    # Même nom, même niveau, même famille : seul le texte départage.
    "Jumeau_A": {
        "name": "Twin",
        "required_level": 10,
        "generation_type": "prefix",
        "groups": ["G"],
        "spawn_weights": [{"tag": "int_armour", "weight": 1}],
        "text": "+(1-2) to maximum [Life|Life]",
    },
    "Jumeau_B": {
        "name": "Twin",
        "required_level": 10,
        "generation_type": "prefix",
        "groups": ["G"],
        "spawn_weights": [{"tag": "int_armour", "weight": 1}],
        "text": "+(3-4) to maximum [Mana|Mana]",
    },
    "Recharge4": {
        "name": "of Buffering",
        "required_level": 48,
        "generation_type": "suffix",
        "groups": ["EnergyShieldRegeneration"],
        "spawn_weights": [{"tag": "int_armour", "weight": 1}],
        "text": "(16-19)% increased [EnergyShield|Energy Shield] Recharge Rate",
    },
}


def entree(nom, niveau, genre, familles, tags, texte, poids):
    return {
        "Name": nom,
        "Level": str(niveau),
        "ModGenerationTypeID": genre,
        "ModFamilyList": familles,
        "DropChance": poids,
        "str": texte,
        "spawn_no": tags,
    }


VUE = {
    "opt": {"tags": "int_armour"},
    "normal": [
        entree(
            "of the Pupil",
            1,
            "2",
            ["Intelligence"],
            ["int_armour", "default"],
            '<span class=\'mod-value\'>+(5<span class="ndash">—</span>8)</span> to <a href="Intelligence">Intelligence</a>',
            "1000",
        ),
        entree(
            "Twin",
            10,
            "1",
            ["G"],
            ["int_armour"],
            "+(3—4) to maximum <a>Mana</a>",
            "250",
        ),
        entree(
            "of Buffering",
            48,
            "2",
            ["EnergyShieldRegeneration"],
            ["int_armour"],
            "(16—19)% increased Energy Shield Recharge Rate",
            1,
        ),
        entree("Inconnu au bataillon", 5, "1", ["X"], ["int_armour"], "rien", "500"),
    ],
}
PAGE = "<html><script>new ModsView(" + json.dumps(VUE) + ");</script></html>"


def test_rapprochement_poids_inconnus_et_orphelins():
    res = p.poids_page(PAGE, REPOE)
    assert res["poids"] == {"Int1": 1000, "Jumeau_B": 250}
    assert res["inconnus"] == ["Recharge4"]  # DropChance entier 1 = non mesuré
    assert [o["nom"] for o in res["non_rapproches"]] == ["Inconnu au bataillon"]
    assert res["etiquette"] == "int_armour"


def test_pages_de_mods_sans_doublon():
    html = (
        '<a href="/us/Bows#ModifiersCalc">x</a><a href="/us/Rings#ModifiersCalc">'
        '<a href="/us/Bows#ModifiersCalc"><a href="/us/Bows">'
    )
    assert p.pages_de_mods(html) == ["Bows", "Rings"]


def test_arret_sur_refus_ou_anti_robot():
    with pytest.raises(p.Arret):
        p.verifier_page(429, PAGE, "u")
    with pytest.raises(p.Arret, match="anti-robot"):
        p.verifier_page(200, "<title>Just a moment...</title>", "u")
    with pytest.raises(p.Arret, match="inattendue"):
        p.verifier_page(200, "<html>autre chose</html>", "u")
    p.verifier_page(200, PAGE, "u")


def test_telecharger_utilise_le_cache_et_attend(tmp_path):
    index = '<a href="/us/Bows#ModifiersCalc"><a href="/us/Rings#ModifiersCalc">'
    (tmp_path / "Rings.html").write_text(PAGE)
    lus, pauses = [], []

    def lire(url):
        lus.append(url)
        return index if url.endswith("Modifiers") else PAGE

    p.telecharger(tmp_path, 6, lire=lire, dormir=pauses.append)
    assert lus == [p.BASE_URL + "Modifiers", p.BASE_URL + "Bows"]
    assert pauses == [6, 6]
    assert (tmp_path / "Bows.html").exists()


def test_craft_proba_lit_la_sortie_poe2db(tmp_path):
    f = tmp_path / "Body_Armours_int.json"
    f.write_text(
        json.dumps({"source": "poe2db", "poids": {"Int1": 1000}, "inconnus": []})
    )
    assert c.charger_poids(f) == {"Int1": 1000.0}
    g = tmp_path / "plat.json"
    g.write_text(json.dumps({"Int1": 7}))
    assert c.charger_poids(g) == {"Int1": 7.0}


def test_craft_proba_lit_le_fichier_multi_pages(tmp_path):
    f = tmp_path / "poids-poe2db.json"
    f.write_text(json.dumps({"pages": {"Bows": {"poids": {"A": 5}, "inconnus": []}}}))
    assert c.charger_poids(f, "Bows") == {"A": 5.0}
    with pytest.raises(KeyError):
        c.charger_poids(f)


def test_releve_verse_au_depot_est_coherent():
    """Le relevé versé garde son attribution et ne contient que des poids mesurés."""
    racine = Path(__file__).resolve().parents[1] / "donnees"
    d = json.loads((racine / "poids-poe2db.json").read_text(encoding="utf-8"))
    assert "Krakenbul" in d["source"] and d["licence"] == "CC BY-NC-SA 3.0"
    assert (racine / "LICENCE-poids-poe2db.md").exists()
    for page, contenu in d["pages"].items():
        assert all(isinstance(v, int) and v > 1 for v in contenu["poids"].values()), (
            page
        )
        assert not set(contenu["poids"]) & set(contenu["inconnus"]), page
