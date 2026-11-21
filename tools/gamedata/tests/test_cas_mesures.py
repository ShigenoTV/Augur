"""Tests de la liste des cas golden à re-mesurer (pytest, sans réseau)."""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import cas_mesures as cm

DEPOT = HERE.parents[2]


def _depot(tmp_path: Path) -> Path:
    archetype = tmp_path / "tests" / "golden" / "archetype"
    archetype.mkdir(parents=True)
    (archetype / "arc.yaml").write_text(
        "# commentaire\n"
        "- id: arc-001\n"
        '  patch: "0.5.5e"\n'
        "  confiance: mesuré\n"
        "  personnage:\n"
        "    competence:\n"
        "      patch: pas-un-champ-de-cas\n"
        "- id: lb-001\n"
        '  patch: "0.5.5.5"\n'
        "  confiance: supposé\n",
        encoding="utf-8",
    )
    moteur = tmp_path / "tests" / "golden" / "moteur"
    moteur.mkdir(parents=True)
    (moteur / "defense.yaml").write_text(
        'id: defense-001\npatch: "1.0.0"   # déjà en 1.0\nconfiance: confirmé\n', encoding="utf-8"
    )
    return tmp_path


def test_version_majeure() -> None:
    assert cm.version_majeure("0.5.5e") == (0, 5)
    assert cm.version_majeure("0.5.5.10") == (0, 5)
    assert cm.version_majeure("1.0.0.1") == (1, 0)
    assert cm.version_majeure("?") is None


def test_lecture_des_cas(tmp_path: Path) -> None:
    cas = {c["id"]: c for c in cm.lire_cas(_depot(tmp_path))}
    assert cas["arc-001"]["patch"] == "0.5.5e" and cas["arc-001"]["confiance"] == "mesuré"
    assert cas["lb-001"]["confiance"] == "supposé"
    assert cas["defense-001"] == {
        "id": "defense-001",
        "patch": "1.0.0",
        "confiance": "confirmé",
        "fichier": "tests/golden/moteur/defense.yaml",
    }


def test_a_remesurer_seulement_les_cas_mesures_d_une_version_plus_ancienne(tmp_path: Path) -> None:
    cas = cm.lire_cas(_depot(tmp_path))
    assert cm.a_remesurer(cas, "0.5.5.10") == []
    assert [c["id"] for c in cm.a_remesurer(cas, "1.0.0.1")] == ["arc-001"]
    assert cm.a_remesurer(cas, "?") == []


def test_commande(tmp_path: Path, capsys) -> None:
    racine = _depot(tmp_path)
    assert cm.principal(["--version", "1.0.0.1", "--github"], racine) == 0
    sortie = capsys.readouterr().out
    assert "1 cas mesurés sur une version plus ancienne" in sortie
    assert "::warning file=tests/golden/archetype/arc.yaml,title=Cas à re-mesurer::arc-001" in sortie


def test_vrais_cas_du_depot() -> None:
    cas = cm.lire_cas(DEPOT)
    mesures = [c for c in cas if c["confiance"] in cm.CONFIANCES_MESUREES]
    assert mesures, "aucun cas mesuré trouvé : le format des YAML golden a changé ?"
    assert all(c["patch"] for c in cas), [c["id"] for c in cas if not c["patch"]]
