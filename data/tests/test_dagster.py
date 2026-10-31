import json

import pytest

dg = pytest.importorskip("dagster")


def test_definitions_load_and_prices_asset_runs_offline(tmp_path, monkeypatch):
    monkeypatch.setenv("AUGUR_RAW_ROOT", str(tmp_path / "raw"))
    monkeypatch.setenv("AUGUR_PRICES_OUT", str(tmp_path / "prices"))
    from augur_data import dagster_defs

    dg.Definitions.validate_loadable(dagster_defs.defs)
    res = dg.materialize([dagster_defs.prices_document], partition_key="Standard")
    assert res.success
    doc = json.loads((tmp_path / "prices" / "Standard.json").read_text())
    assert doc["league"] == "Standard" and doc["unit"] == "exalted"
