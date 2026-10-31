-- Augur : schéma ClickHouse des prix (sprint 1, S2 Data).
--
-- Principes :
-- * le stockage brut (object storage) reste la source de vérité ; ces tables
--   sont reconstructibles en rejouant les snapshots (colonne snapshot_key) ;
-- * aucune TTL : l'historique par ligue est l'actif long terme du studio ;
-- * partition par ligue et par mois : une ligue se purge ou se rejoue d'un bloc ;
-- * aucun nom de compte : les vendeurs sont des empreintes (seller_id).

CREATE DATABASE IF NOT EXISTS augur;

-- Registre des snapshots bruts ingérés (idempotence du chargement).
CREATE TABLE IF NOT EXISTS augur.raw_snapshots
(
    snapshot_key      String,
    sha256            FixedString(64),
    source            LowCardinality(String),   -- currency_exchange | trade_sample
    realm             LowCardinality(String),   -- poe2
    league            LowCardinality(String),
    fetched_at        DateTime64(3, 'UTC'),
    collector_version LowCardinality(String),
    http_status       UInt16,
    loaded_at         DateTime64(3, 'UTC') DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(loaded_at)
ORDER BY (source, realm, league, snapshot_key);

-- Observations horaires de l'échange de devises officiel (une ligne par paire et par heure).
CREATE TABLE IF NOT EXISTS augur.currency_pair_observations
(
    realm         LowCardinality(String),
    league        LowCardinality(String),
    observed_at   DateTime('UTC'),
    base          LowCardinality(String),
    quote         LowCardinality(String),
    rate          Float64,                     -- 1 base = rate quote
    weight        Float64,                     -- volume échangé côté base
    method        LowCardinality(String),      -- volume | ratio_mid
    snapshot_key  String
)
ENGINE = ReplacingMergeTree
PARTITION BY (league, toYYYYMM(observed_at))
ORDER BY (league, base, quote, observed_at);

-- Annonces de trade échantillonnées, après nettoyage. Les annonces exclues
-- sont conservées avec leur motif, pour auditer le nettoyage.
CREATE TABLE IF NOT EXISTS augur.trade_listings
(
    realm          LowCardinality(String),
    league         LowCardinality(String),
    query_id       LowCardinality(String),
    category       LowCardinality(String),
    listing_id     String,
    seller_id      Nullable(FixedString(16)),
    indexed_at     Nullable(DateTime('UTC')),
    observed_at    DateTime64(3, 'UTC'),
    price_type     LowCardinality(Nullable(String)),
    amount         Nullable(Float64),
    currency       LowCardinality(Nullable(String)),
    stack_size     UInt32,
    unit_price_ex  Nullable(Float64),
    exclusion      LowCardinality(Nullable(String)), -- null = retenue
    snapshot_key   String
)
ENGINE = ReplacingMergeTree
PARTITION BY (league, toYYYYMM(observed_at))
ORDER BY (league, query_id, observed_at, listing_id);

-- Prix publiés (un point par génération du document GET /prices/{league}).
CREATE TABLE IF NOT EXISTS augur.price_index
(
    realm         LowCardinality(String),
    league        LowCardinality(String),
    item_id       LowCardinality(String),      -- {catégorie}/{identifiant}, cf. contrat v1
    computed_at   DateTime('UTC'),
    price_ex      Float64,
    low_ex        Float64,
    high_ex       Float64,
    sample_size   UInt32,
    confidence    LowCardinality(String),
    method        LowCardinality(String),
    source        LowCardinality(String),
    etag          String
)
ENGINE = ReplacingMergeTree
PARTITION BY (league, toYYYYMM(computed_at))
ORDER BY (league, item_id, computed_at);

-- Tableau de bord qualité : taux d'exclusion par requête et par heure.
CREATE VIEW IF NOT EXISTS augur.v_trade_quality_hourly AS
SELECT
    league,
    query_id,
    toStartOfHour(observed_at)                 AS hour,
    count()                                    AS listings,
    countIf(exclusion IS NULL)                 AS kept,
    countIf(exclusion = 'leurre_bas')          AS decoys_low,
    countIf(exclusion = 'aberrant_haut')       AS outliers_high,
    countIf(exclusion = 'mur_vendeur')         AS seller_walls,
    countIf(exclusion IN ('doublon', 'reannonce')) AS duplicates,
    countIf(exclusion = 'devise_inconnue')     AS unknown_currency
FROM augur.trade_listings
GROUP BY league, query_id, hour;
