"""Tests de l'outil de paquets de données (pytest)."""

from __future__ import annotations

import base64
import json
import shutil
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import augur_datapack as dp  # noqa: E402

SRC = HERE.parent / "testdata" / "src"
VECTOR = HERE.parent / "testdata" / "vector"
# Graine fixe, UNIQUEMENT pour les vecteurs de test (jamais en production).
TEST_SEED = base64.b64encode(bytes(range(32))).decode()
FIXED_TIME = "2026-01-01T00:00:00Z"


@pytest.fixture()
def priv():
    return dp.load_private_key(TEST_SEED)


def _publish(out: Path, priv, channel="stable", sequence=100):
    res = dp.build_pack(SRC, out, "poe2", "0.4.1", priv, created_at=FIXED_TIME)
    dp.write_channel(out, "poe2", channel, res.manifest_key, priv,
                     sequence=sequence, published_at=FIXED_TIME)
    return res


def test_chaine_complete_valide(tmp_path, priv):
    res = _publish(tmp_path, priv)
    assert res.files == 3
    pointer = dp.verify_channel(dp.Source(str(tmp_path)), "poe2", "stable", [priv.public_key()])
    assert pointer["patch"] == "0.4.1"


def test_vecteur_de_reference_stable(tmp_path, priv):
    """Le vecteur commité doit être reproductible octet pour octet.

    Il sert de test d'interopérabilité au vérificateur Rust de l'application.
    Régénérer avec : python tests/test_datapack.py --regen
    """
    _publish(tmp_path, priv)
    produced = sorted(p.relative_to(tmp_path).as_posix() for p in tmp_path.rglob("*") if p.is_file())
    expected = sorted(p.relative_to(VECTOR).as_posix() for p in VECTOR.rglob("*") if p.is_file()
                      if p.name != "README.md")
    assert produced == expected
    for rel in produced:
        assert (tmp_path / rel).read_bytes() == (VECTOR / rel).read_bytes(), rel


def test_blob_altere_rejete(tmp_path, priv):
    _publish(tmp_path, priv)
    blob = next((tmp_path / "poe2" / "blobs").iterdir())
    blob.write_bytes(blob.read_bytes() + b" ")
    with pytest.raises(dp.DataPackError, match="blob corrompu"):
        dp.verify_channel(dp.Source(str(tmp_path)), "poe2", "stable", [priv.public_key()])


def test_manifeste_altere_rejete(tmp_path, priv):
    res = _publish(tmp_path, priv)
    path = tmp_path / "poe2" / res.manifest_key
    path.write_bytes(path.read_bytes().replace(b"0.4.1", b"0.4.2"))
    with pytest.raises(dp.DataPackError, match="empreinte"):
        dp.verify_channel(dp.Source(str(tmp_path)), "poe2", "stable", [priv.public_key()])


def test_canal_altere_rejete(tmp_path, priv):
    _publish(tmp_path, priv)
    path = tmp_path / "poe2" / "channels" / "stable.json"
    env = json.loads(path.read_bytes())
    pointer = json.loads(base64.b64decode(env["payload"]))
    pointer["sequence"] = 999
    env["payload"] = base64.b64encode(dp.canonical_json(pointer)).decode()
    path.write_bytes(dp.canonical_json(env))
    with pytest.raises(dp.DataPackError, match="signature invalide"):
        dp.verify_channel(dp.Source(str(tmp_path)), "poe2", "stable", [priv.public_key()])


def test_cle_inconnue_rejetee(tmp_path, priv):
    _publish(tmp_path, priv)
    autre = dp.Ed25519PrivateKey.generate().public_key()
    with pytest.raises(dp.DataPackError, match="non reconnue"):
        dp.verify_channel(dp.Source(str(tmp_path)), "poe2", "stable", [autre])


def test_rotation_de_cle_acceptee(tmp_path, priv):
    _publish(tmp_path, priv)
    autre = dp.Ed25519PrivateKey.generate().public_key()
    dp.verify_channel(dp.Source(str(tmp_path)), "poe2", "stable", [autre, priv.public_key()])


def test_rollback_rejete(tmp_path, priv):
    _publish(tmp_path, priv, sequence=100)
    with pytest.raises(dp.DataPackError, match="rollback"):
        dp.verify_channel(dp.Source(str(tmp_path)), "poe2", "stable",
                          [priv.public_key()], min_sequence=101)


def test_canal_inconnu_refuse(tmp_path, priv):
    res = dp.build_pack(SRC, tmp_path, "poe2", "0.4.1", priv)
    with pytest.raises(dp.DataPackError, match="canal inconnu"):
        dp.write_channel(tmp_path, "poe2", "prod", res.manifest_key, priv)


@pytest.mark.parametrize("patch", ["", "latest", "0.4", "../0.4.1"])
def test_patch_invalide_refuse(tmp_path, priv, patch):
    with pytest.raises(dp.DataPackError, match="patch invalide"):
        dp.build_pack(SRC, tmp_path, "poe2", patch, priv)


def test_nom_de_fichier_refuse(tmp_path, priv):
    src = tmp_path / "src"
    src.mkdir()
    (src / "Mods.JSON").write_text("{}")
    with pytest.raises(dp.DataPackError, match="nom de fichier refusé"):
        dp.build_pack(src, tmp_path / "out", "poe2", "0.4.1", priv)


def test_cli_keygen_et_build(tmp_path, monkeypatch, capsys):
    assert dp.main(["keygen"]) == 0
    lines = [l for l in capsys.readouterr().out.splitlines() if not l.startswith("#")]
    seed, pub = lines[0], lines[1]
    monkeypatch.setenv("AUGUR_DATA_SIGNING_KEY", seed)
    assert dp.main(["build", "--src", str(SRC), "--out", str(tmp_path), "--patch", "0.4.1"]) == 0
    manifest = json.loads(capsys.readouterr().out)["manifest"]
    assert dp.main(["channel", "--out", str(tmp_path), "--channel", "beta", "--manifest", manifest]) == 0
    assert dp.main(["verify", "--root", str(tmp_path), "--channel", "beta", "--pubkey", pub]) == 0


def test_cli_sans_cle_echoue_proprement(tmp_path, monkeypatch, capsys):
    monkeypatch.delenv("AUGUR_DATA_SIGNING_KEY", raising=False)
    assert dp.main(["build", "--src", str(SRC), "--out", str(tmp_path), "--patch", "0.4.1"]) == 1
    assert "AUGUR_DATA_SIGNING_KEY" in capsys.readouterr().err


if __name__ == "__main__" and "--regen" in sys.argv:
    shutil.rmtree(VECTOR / "poe2", ignore_errors=True)
    _publish(VECTOR, dp.load_private_key(TEST_SEED))
    print("vecteur régénéré dans", VECTOR)
