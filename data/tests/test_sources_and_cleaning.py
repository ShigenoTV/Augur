from datetime import timedelta

import pytest

from augur_data.cleaning import clean_listings, quality_report
from augur_data.currency_index import build_rates
from augur_data.item_index import build_item_prices
from augur_data.sources import currency_exchange, trade_sampler
from augur_data.storage import make_envelope
from conftest import LEAGUE, NOW, cx_body, listing

RATES = {"exalted": 1.0, "divine": 300.0, "chaos": 15.0}


def cx_obs():
    env = make_envelope(
        source="currency_exchange",
        realm="poe2",
        league="_all",
        request={},
        status=200,
        headers={},
        body=cx_body(),
        fetched_at=NOW,
    )
    return currency_exchange.parse(env)


def test_cx_parse_uses_volume_ratio_and_skips_malformed():
    obs = cx_obs()
    assert len(obs) == 4
    div = next(o for o in obs if o["base"] == "divine" and o["league"] == LEAGUE)
    assert div["rate"] == pytest.approx(300)
    assert div["method"] == "volume"
    assert div["observed_at"] == (NOW - timedelta(hours=1)).isoformat()


def test_rates_direct_triangulated_and_league_filtered():
    rates = build_rates(cx_obs(), league=LEAGUE, now=NOW)
    assert rates["exalted"].value == 1
    assert rates["divine"].value == pytest.approx(300)
    assert rates["chaos"].value == pytest.approx(15)
    assert rates["annul"].value == pytest.approx(60)
    assert rates["annul"].method == "triangulated:divine"


def test_rates_exclude_outlier_observation():
    obs = [
        {
            "league": "L",
            "observed_at": NOW.isoformat(),
            "base": "divine",
            "quote": "exalted",
            "rate": r,
            "weight": 10,
        }
        for r in (300, 305, 295, 310, 3)
    ]
    rates = build_rates(obs, league="L", now=NOW)
    assert rates["divine"].excluded == 1
    assert 295 <= rates["divine"].value <= 310


def rows(book):
    env = make_envelope(
        source="trade_sample",
        realm="poe2",
        league=LEAGUE,
        request={"query_id": "q", "category": "fragment", "name": "Q", "search": {}},
        status=200,
        headers={},
        body={"result": book},
        fetched_at=NOW,
    )
    return trade_sampler.parse(env)


def reasons(df):
    return dict(zip(df["listing_id"].to_list(), df["exclusion"].to_list(), strict=True))


def test_decoys_and_outliers_are_flagged():
    book = [listing(0, 0.01)] + [listing(i, 2 + 0.1 * i) for i in range(1, 12)] + [listing(99, 900)]
    df = clean_listings(rows(book), RATES)
    r = reasons(df)
    assert r["l0"] == "leurre_bas"
    assert r["l99"] == "aberrant_haut"
    assert sum(v is None for v in r.values()) == 11


def test_normalization_in_exalts_with_stack_and_unknown_currency():
    book = [listing(1, 1, "divine", stack=10), listing(2, 3, "monnaie-inconnue"), listing(3, None)]
    df = clean_listings(rows(book), RATES)
    r = reasons(df)
    assert df.filter(df["listing_id"] == "l1")["unit_price_ex"][0] == pytest.approx(30)
    assert r["l2"] == "devise_inconnue"
    assert r["l3"] == "prix_absent"


def test_duplicates_relists_and_seller_wall():
    base = [listing(i, 5 + i) for i in range(1, 6)]
    relist = [listing(10, 5, seller="spam"), listing(11, 5, seller="spam")]
    wall = [listing(20 + i, 3 + 0.01 * i, seller="mur") for i in range(8)]
    df = clean_listings(rows(base + relist + wall) + rows([base[0]]), RATES)
    r = df["exclusion"].to_list()
    assert r.count("doublon") == 1
    assert r.count("reannonce") == 1
    assert r.count("mur_vendeur") == 7


def test_seller_names_never_leave_cleaning():
    df = clean_listings(rows([listing(1, 2, seller="CompteJoueur")]), RATES)
    assert "seller" not in df.columns
    assert "CompteJoueur" not in str(df.to_dicts())


def test_item_price_is_median_of_best_listing_per_seller():
    book = [listing(i, 2 + 0.1 * i) for i in range(1, 12)] + [
        listing(50 + i, 1.0 + 0.01 * i, seller="mur") for i in range(12)
    ]
    df = clean_listings(rows(book), RATES)
    [p] = build_item_prices(df, now=NOW)
    # Le mur est réduit à sa meilleure annonce, elle-même trop basse : leurre.
    assert p.sellers == 11
    assert 2.3 <= p.value <= 2.8
    q = quality_report(df)
    assert q["listings"] == 23
    assert q["excluded_by_reason"] == {"mur_vendeur": 11, "leurre_bas": 1}
