"""Faux serveur GGG pour les tests : aucune requête réseau réelle."""

import json
from datetime import UTC, datetime, timedelta

import httpx
import pytest

from augur_data.ggg import ClientIdentity, GGGClient
from augur_data.ratelimit import HeaderDrivenBudget

NOW = datetime(2026, 10, 31, 12, 0, tzinfo=UTC)
LEAGUE = "Rise of the Abyssal"

RL = {
    "X-Rate-Limit-Policy": "test-policy",
    "X-Rate-Limit-Rules": "Ip",
    "X-Rate-Limit-Ip": "100:10:60",
    "X-Rate-Limit-Ip-State": "1:10:0",
}


def cx_body():
    hour = int(NOW.timestamp())

    def m(market, va, vb):
        a, b = market.split("|")
        return {
            "league": LEAGUE,
            "market_id": market,
            "volume_traded": {a: va, b: vb},
            "lowest_ratio": {a: 1, b: vb / va * 0.9},
            "highest_ratio": {a: 1, b: vb / va * 1.1},
        }

    return {
        "next_change_id": hour,
        "markets": [
            m("divine|exalted", 100, 30000),  # 1 divine = 300 ex
            m("chaos|exalted", 1000, 15000),  # 1 chaos = 15 ex
            m("annul|divine", 50, 10),  # 1 annul = 0,2 div = 60 ex (triangulé)
            {
                "league": "Standard",
                "market_id": "divine|exalted",
                "volume_traded": {"divine": 1, "exalted": 999},
            },
            {"league": LEAGUE, "market_id": "malformé"},
        ],
    }


def listing(i, amount, currency="exalted", seller=None, hours_ago=1, stack=1):
    return {
        "id": f"l{i}",
        "listing": {
            "indexed": (NOW - timedelta(hours=hours_ago)).isoformat(),
            "account": {"name": seller or f"vendeur{i}"},
            "price": {"type": "~price", "amount": amount, "currency": currency},
        },
        "item": {"stackSize": stack},
    }


BOOK = [listing(0, 0.01)] + [listing(i, 2 + 0.1 * i) for i in range(1, 12)] + [listing(99, 900)]


class FakeGGG:
    def __init__(self, book=BOOK):
        self.book = {x["id"]: x for x in book}
        self.requests: list[httpx.Request] = []

    def __call__(self, request: httpx.Request) -> httpx.Response:
        self.requests.append(request)
        path = request.url.path
        headers = dict(RL, **{"set-cookie": "POESESSID=piege; Path=/", "x-secret": "ne pas garder"})
        if path.startswith("/currency-exchange/"):
            return httpx.Response(200, json=cx_body(), headers=headers)
        if path.startswith("/api/trade2/search/"):
            return httpx.Response(
                200, json={"id": "abc", "result": list(self.book), "total": len(self.book)}, headers=headers
            )
        if path.startswith("/api/trade2/fetch/"):
            ids = path.rsplit("/", 1)[1].split(",")
            return httpx.Response(200, json={"result": [self.book[i] for i in ids]}, headers=headers)
        return httpx.Response(404)


@pytest.fixture
def fake_ggg():
    return FakeGGG()


@pytest.fixture
def client(fake_ggg, monkeypatch):
    monkeypatch.delenv("AUGUR_INGESTION_KILL_SWITCH", raising=False)
    monkeypatch.delenv("AUGUR_INGESTION_KILL_FILE", raising=False)
    c = GGGClient(
        ClientIdentity("augur-test", "data@augur.invalid"),
        HeaderDrivenBudget(),
        access_token="jeton-de-test",
        transport=httpx.MockTransport(fake_ggg),
    )
    yield c
    c.close()


def dumps(x):
    return json.dumps(x, sort_keys=True)
