"""Tests du contrôle des monnaies de Forge contre poe.ninja (pytest, sans réseau)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import controle_monnaies as cm


def _page(lignes: dict[str, float], noms: dict[str, str] | None = None) -> dict:
    noms = noms or {}
    return {
        "core": {"primary": "divine", "items": []},
        "items": [{"id": i, "name": noms.get(i, i)} for i in lignes],
        "lines": [{"id": i, "primaryValue": v} for i, v in lignes.items()],
    }


def _pages(**surcharges: dict) -> dict:
    pages = {c: _page({}) for c in cm.CATEGORIES}
    pages.update(surcharges)
    return pages


SOURCES = {
    "Currency": [("exalt", "exalted"), ("chaos", "chaos"), ("divine", "divine")],
    "Ritual": [("omen_light", "omen-of-light"), ("omen_liege", "omen-of-the-liege")],
}
ESSENCES = {"essence-of-flames": "Essence of Flames", "adaptive-alloy": "Adaptive Alloy"}


def test_table_sources_lue_dans_ninja_rs() -> None:
    sources = cm.sources_forge(cm.NINJA_RS.read_text(encoding="utf-8"))
    assert ("divine", "divine") in sources["Currency"]
    assert ("exalt", "exalted") in sources["Currency"]
    assert ("gnawed_jawbone", "gnawed-jawbone") in sources["Abyss"]
    assert sum(len(v) for v in sources.values()) == 51  # taille de `SOURCES`


def test_essences_lues_dans_forge() -> None:
    essences = cm.essences_forge(cm.FORGE)
    assert "adaptive-alloy" in essences and len(essences) > 50


def test_monnaie_sans_cote_et_cote_nulle() -> None:
    pages = _pages(
        Currency=_page({"exalted": 0.0018, "chaos": 0.0, "divine": 1}),
        Ritual=_page({"omen-of-light": 0.5}),
        Essences=_page({"essence-of-flames": 0.01}),
        Verisium=_page({"adaptive-alloy": 0.2}),
    )
    res = cm.controler(pages, SOURCES, ESSENCES, {})
    assert res["sans_cote"] == [("Currency", "chaos", "chaos"), ("Ritual", "omen_liege", "omen-of-the-liege")]
    assert res["essences_sans_cote"] == []
    assert res["pages_vides"] == ["Abyss"]
    assert cm.nb_points(res) == 3


def test_sans_cote_connus_ranges_a_part() -> None:
    pages = _pages(Currency=_page({"exalted": 1, "chaos": 1, "divine": 1}), Ritual=_page({"omen-of-light": 1}))
    res = cm.controler(pages, SOURCES, ESSENCES, {}, {"omen_liege"})
    assert res["sans_cote"] == []
    assert res["sans_cote_connus"] == [("Ritual", "omen_liege", "omen-of-the-liege")]
    assert "Déjà sans cote" in cm.rapport(res, "Test", "2026-12-11")


def test_nouvel_objet_signale_sauf_deja_vu() -> None:
    pages = _pages(
        Currency=_page({"exalted": 1, "chaos": 1, "divine": 1, "vaal": 1, "orb-of-dominance": 2},
                       {"orb-of-dominance": "Orb of Dominance"}),
        Ritual=_page({"omen-of-light": 1, "omen-of-the-liege": 1}),
        Verisium=_page({"adaptive-alloy": 1, "verisium": 1}),
        Essences=_page({"essence-of-flames": 1, "essence-of-wonder": 1}),
    )
    deja = {"Currency": ["vaal"], "Verisium": ["verisium"]}
    res = cm.controler(pages, SOURCES, ESSENCES, deja)
    assert res["nouveaux"] == [
        ("Currency", "orb-of-dominance", "Orb of Dominance"),
        ("Essences", "essence-of-wonder", "essence-of-wonder"),
    ]
    texte = cm.rapport(res, "Test", "2026-12-11")
    assert "Orb of Dominance" in texte and "Nouveaux objets" in texte


def test_page_vide_debut_de_ligue() -> None:
    res = cm.controler(_pages(), SOURCES, ESSENCES, {})
    assert "Currency" in res["pages_vides"]
    assert "pas encore de prix" in cm.rapport(res, "Nouvelle", "2026-12-11")


def test_ligne_de_commande_depuis_dossier_et_referencer(tmp_path: Path, monkeypatch) -> None:
    dossier = tmp_path / "pages"
    dossier.mkdir()
    pages = _pages(Currency=_page({"exalted": 1, "vaal": 1}))
    for cat, page in pages.items():
        (dossier / f"{cat}.json").write_text(json.dumps(page), encoding="utf-8")
    deja = tmp_path / "deja.json"
    monkeypatch.setattr(cm, "DEJA_VUS", deja)
    rapport = tmp_path / "r.md"
    code = cm.main(["--depuis", str(dossier), "--rapport", str(rapport), "--referencer", "--date", "2026-12-11"])
    assert code == 1  # monnaies de Forge sans cote dans ces pages presque vides
    assert "vaal" in json.loads(deja.read_text(encoding="utf-8"))["categories"]["Currency"]
    # Après --referencer, les mêmes pages ne signalent plus que les pages vides.
    cm.main(["--depuis", str(dossier), "--rapport", str(rapport), "--date", "2026-12-11"])
    texte = rapport.read_text(encoding="utf-8")
    assert "Nouveaux objets cotés inconnus de Forge | 0" in texte
    assert "Monnaies, Omens et os de Forge sans cote | 0" in texte
