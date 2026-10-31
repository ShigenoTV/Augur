import httpx
import pytest

from augur_data.ggg import ClientIdentity, GGGClient, KillSwitchError
from augur_data.ratelimit import HeaderDrivenBudget, RateLimitedError


def test_user_agent_and_auth(client, fake_ggg):
    client.request(
        "GET", "https://api.pathofexile.com/currency-exchange/poe2", budget_key="cx", authenticated=True
    )
    req = fake_ggg.requests[0]
    assert req.headers["user-agent"] == "OAuth augur-test/0.1.0 (contact: data@augur.invalid)"
    assert req.headers["authorization"] == "Bearer jeton-de-test"


def test_never_sends_cookies_and_keeps_only_safe_headers(client, fake_ggg):
    r1 = client.request(
        "POST", "https://www.pathofexile.com/api/trade2/search/poe2/X", budget_key="s", json={}
    )
    client.request("POST", "https://www.pathofexile.com/api/trade2/search/poe2/X", budget_key="s", json={})
    assert "cookie" not in fake_ggg.requests[1].headers  # le POESESSID renvoyé n'est jamais réémis
    assert set(r1.headers) == {
        "x-rate-limit-policy",
        "x-rate-limit-rules",
        "x-rate-limit-ip",
        "x-rate-limit-ip-state",
    }


def test_unauthenticated_call_has_no_token(client, fake_ggg):
    client.request("GET", "https://www.pathofexile.com/api/trade2/fetch/l1", budget_key="f")
    assert "authorization" not in fake_ggg.requests[0].headers


def test_kill_switch(client, fake_ggg, monkeypatch, tmp_path):
    monkeypatch.setenv("AUGUR_INGESTION_KILL_SWITCH", "1")
    with pytest.raises(KillSwitchError):
        client.request("GET", "https://api.pathofexile.com/x", budget_key="k")
    monkeypatch.delenv("AUGUR_INGESTION_KILL_SWITCH")
    flag = tmp_path / "stop"
    flag.touch()
    monkeypatch.setenv("AUGUR_INGESTION_KILL_FILE", str(flag))
    with pytest.raises(KillSwitchError):
        client.request("GET", "https://api.pathofexile.com/x", budget_key="k")
    assert fake_ggg.requests == []


def test_429_raises_and_blocks_queue():
    budget = HeaderDrivenBudget()
    transport = httpx.MockTransport(lambda r: httpx.Response(429, headers={"Retry-After": "300"}))
    with (
        GGGClient(ClientIdentity("a", "b"), budget, transport=transport) as c,
        pytest.raises(RateLimitedError),
    ):
        c.request("GET", "https://www.pathofexile.com/x", budget_key="k")
    assert budget.wait_time("k") > 290
