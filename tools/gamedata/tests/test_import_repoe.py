"""Tests de l'importeur RePoE (pytest, sans réseau)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent.parent / "datapack"))

import import_repoe as ir

REPOE = HERE.parent / "testdata" / "repoe"


def _load(name: str) -> dict:
    return json.loads((REPOE / name).read_text(encoding="utf-8"))


def _tables() -> dict[str, bytes]:
    return ir.build_tables(
        _load("mods.min.json"), _load("base_items.min.json"), "4.5.5.2"
    )


def test_filtre_des_mods():
    ids = [m["id"] for m in ir.import_mods(_load("mods.min.json"))]
    # Triés ; monstres, uniques et mods sans stat exclus.
    assert ids == ["DesecratedFire1", "EssenceLife1", "FireResist5", "IncreasedLife9"]


def test_champs_d_un_mod():
    mods = {m["id"]: m for m in ir.import_mods(_load("mods.min.json"))}
    vie = mods["IncreasedLife9"]
    assert vie["texte"] == "+(120-149) to maximum Life"
    assert vie["stats"] == [{"id": "base_maximum_life", "min": 120, "max": 149}]
    assert (vie["slot"], vie["groupe"], vie["niveau"]) == (
        "prefix",
        "IncreasedLife",
        65,
    )
    assert vie["tirable"] and not vie["essence_seulement"]
    assert mods["FireResist5"]["texte"] == "+(21-25)% to Fire Resistance"
    # Gardés pour le parseur, mais jamais tirés au hasard.
    assert (
        mods["EssenceLife1"]["essence_seulement"]
        and not mods["EssenceLife1"]["tirable"]
    )
    assert not mods["DesecratedFire1"]["tirable"]


def test_bases():
    bases = ir.import_bases(_load("base_items.min.json"))
    assert [b["nom"] for b in bases] == ["Ornate Cuffs", "Iron Ring"]
    gants = bases[0]
    assert gants["exigences"] == {
        "niveau": 59,
        "force": 39,
        "dexterite": 0,
        "intelligence": 39,
    }
    assert bases[1]["implicites"] == ["AddedPhysicalDamageImplicitRing1"]
    assert bases[1]["exigences"]["niveau"] == 0


def test_sortie_deterministe_et_canonique():
    a, b = _tables(), _tables()
    assert a == b
    assert sorted(a) == ["tables/bases.json", "tables/mods.json", "tables/source.json"]
    for data in a.values():
        assert data.endswith(b"\n")
        assert data == ir.canonical_json(json.loads(data))
    source = json.loads(a["tables/source.json"])
    assert source["comptes"] == {
        "mods": 4,
        "mods_tirables": 2,
        "groupes": 2,
        "bases": 2,
    }
    assert source["version_repoe"] == "4.5.5.2"


def test_format_change_refuse():
    with pytest.raises(ir.RepoeError):
        ir.import_mods([])
    casse = _load("mods.min.json")
    del casse["IncreasedLife9"]["required_level"]
    with pytest.raises(ir.RepoeError, match="required_level"):
        ir.import_mods(casse)
    with pytest.raises(ir.RepoeError):
        ir.build_tables({}, _load("base_items.min.json"), None)


def test_cli_et_paquet(tmp_path):
    out = tmp_path / "tables"
    rc = ir.main(
        [
            "--mods", str(REPOE / "mods.min.json"),
            "--bases", str(REPOE / "base_items.min.json"),
            "--out", str(out),
        ]
    )  # fmt: skip
    assert rc == 0
    assert (out / "tables" / "mods.json").read_bytes() == _tables()["tables/mods.json"]
    # La sortie est un dossier accepté tel quel par l'outil de paquets.
    dp = pytest.importorskip("augur_datapack", reason="tools/datapack absent du chemin")
    priv = dp.load_private_key("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=")
    res = dp.build_pack(out, tmp_path / "pack", "poe2", "0.4.1", priv)
    assert res is not None


def test_cli_erreur(tmp_path, capsys):
    rc = ir.main(
        [
            "--mods",
            str(tmp_path / "absent.json"),
            "--bases",
            "x",
            "--out",
            str(tmp_path),
        ]
    )
    assert rc == 1
    assert "erreur" in capsys.readouterr().err
