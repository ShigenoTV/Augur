import pytest

from augur_data.storage import LocalRawStore, make_envelope, read_snapshots, write_snapshot


def test_write_once_and_roundtrip(tmp_path):
    store = LocalRawStore(tmp_path)
    env = make_envelope(
        source="trade_sample",
        realm="poe2",
        league="Rise of the Abyssal",
        request={"q": 1},
        status=200,
        headers={},
        body={"result": []},
    )
    snap = write_snapshot(store, env)
    assert snap.key.startswith("raw/trade_sample/poe2/Rise_of_the_Abyssal/")
    with pytest.raises(FileExistsError):
        write_snapshot(store, env)  # un snapshot ne s'écrase jamais
    [back] = list(read_snapshots(store, "raw/trade_sample/"))
    assert back.sha256 == snap.sha256
    assert back.envelope["response"]["body"] == {"result": []}
    assert back.envelope["_key"] == snap.key
