import pytest

from augur_data.ratelimit import HeaderDrivenBudget, RateLimitedError, parse_rate_limit_headers

HEADERS = {
    "X-Rate-Limit-Policy": "trade-search-request-limit",
    "X-Rate-Limit-Rules": "Ip,Account",
    "X-Rate-Limit-Ip": "5:10:60,15:60:120",
    "X-Rate-Limit-Ip-State": "1:10:0,1:60:0",
    "X-Rate-Limit-Account": "3:5:60",
    "X-Rate-Limit-Account-State": "1:5:0",
}


class FakeClock:
    def __init__(self):
        self.t = 1000.0
        self.slept = []

    def now(self):
        return self.t

    def sleep(self, s):
        self.slept.append(s)
        self.t += s


def make(margin=1, max_wait=900.0):
    c = FakeClock()
    return HeaderDrivenBudget(margin=margin, clock=c.now, sleep=c.sleep, max_wait=max_wait), c


def test_parse_headers():
    policy, rules, states = parse_rate_limit_headers(HEADERS)
    assert policy == "trade-search-request-limit"
    assert [(r.max_hits, r.period, r.penalty) for r in rules["Ip"]] == [(5, 10, 60), (15, 60, 120)]
    assert rules["Account"][0].max_hits == 3
    assert states["Ip"][0].hits == 1


def test_single_probe_until_policy_is_known():
    b, c = make()
    b.acquire("k")
    assert b.wait_time("k") > 0  # pas de seconde requête à l'aveugle
    b.observe("k", 200, HEADERS)
    assert b.wait_time("k") == 0


def test_respects_tightest_rule_with_margin():
    b, c = make(margin=1)
    b.acquire("k")
    b.observe("k", 200, HEADERS)
    # Account : 3 hits / 5 s, marge 1 → 2 requêtes par fenêtre de 5 s.
    b.acquire("k")
    assert c.slept == []
    b.acquire("k")
    assert c.slept and c.slept[0] == pytest.approx(5.0)


def test_aligns_on_server_state():
    b, c = make(margin=0)
    b.acquire("k")
    hdr = dict(HEADERS, **{"X-Rate-Limit-Account-State": "3:5:0"})
    b.observe("k", 200, hdr)  # le serveur compte 3 hits : la fenêtre est pleine
    assert b.wait_time("k") == pytest.approx(5.0)


def test_429_blocks_with_retry_after_and_gives_up_when_too_long():
    b, c = make(max_wait=30)
    b.acquire("k")
    b.observe("k", 429, dict(HEADERS, **{"Retry-After": "120"}))
    assert b.wait_time("k") == pytest.approx(120)
    with pytest.raises(RateLimitedError):
        b.acquire("k")


def test_active_restriction_in_state_blocks():
    b, c = make()
    b.acquire("k")
    b.observe("k", 200, dict(HEADERS, **{"X-Rate-Limit-Ip-State": "6:10:45,6:60:0"}))
    assert b.wait_time("k") == pytest.approx(45)


def test_queues_are_independent():
    b, c = make()
    b.acquire("search")
    assert b.wait_time("fetch") == 0
