# augur-data

Ingestion, nettoyage et indices de prix PoE 2 pour Augur. Conception et règles : [docs/data/pipeline-prix.md](../docs/data/pipeline-prix.md).

## Installation

```bash
cd data
python3.13 -m venv .venv && source .venv/bin/activate
pip install -r requirements-dev.txt -e .
# orchestration (optionnelle) :
pip install -r requirements-pipelines.txt
```

Les fichiers `requirements*.txt` sont générés par `uv pip compile` depuis `pyproject.toml` et audités par `pip-audit` dans la CI de sécurité.

## Tests

```bash
pytest          # aucun appel réseau : un faux serveur GGG simule les réponses
ruff check src tests && ruff format --check src tests
```

## Utilisation

Identité du studio dans l'environnement (jamais dans le dépôt) : `AUGUR_GGG_CLIENT_ID`, `AUGUR_GGG_CONTACT`, `AUGUR_GGG_TOKEN`.

```bash
python -m augur_data collect-cx                                   # une heure d'agrégats de devises
python -m augur_data collect-trade --league "Standard"            # requêtes curées de queries/poe2-s1.json
python -m augur_data build --league "Standard" --out var/prices/Standard.json
dagster dev -m augur_data.dagster_defs                            # orchestration locale
```

Kill switch : `AUGUR_INGESTION_KILL_SWITCH=1`, ou un fichier sentinelle désigné par `AUGUR_INGESTION_KILL_FILE`. Codes de sortie : 3 = kill switch, 4 = rate limit GGG (la collecte s'arrête sans insister).

Le schéma ClickHouse est dans [`sql/clickhouse/`](sql/clickhouse).
