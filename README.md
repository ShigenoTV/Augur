# Augur

Augur est une application **Windows** d'outils tiers pour **Path of Exile 2**. Elle réunit dans un seul logiciel un overlay en jeu et un planificateur de builds, autour d'un moteur de calcul local.

> Projet en phase de fondation. Aucun code n'est encore publié.

## Modules prévus

| Module | Rôle |
|---|---|
| **Augur Copilot** | Overlay en jeu : sur Ctrl+C, prix estimé et gain de DPS / EHP de l'objet pour ton personnage, et recherche d'upgrades sous budget. |
| **Augur Ledger** | Suivi de la rentabilité réelle des stratégies de farm d'Atlas, à partir de `Client.txt` et des réserves (OAuth). |
| **Augur Forge** | Planificateur de craft à coût espéré : « crafter ou acheter » aux prix du marché du moment. |

## Principes non négociables

- **Windows 10/11 uniquement**, pas de site web.
- **Conformité aux règles de GGG** sur les outils tiers : une action serveur par pression de touche, aucune automatisation, aucune lecture de la mémoire du jeu, aucune modification de ses fichiers.
- **Jamais de `POESESSID`** : l'accès au compte passe uniquement par l'OAuth officiel.
- Les recherches de trade partent du poste du joueur et respectent les rate limits annoncés par les en-têtes de l'API.

## Stack prévue

- Cœur de calcul et parseur d'objets : **Rust** (`poe2-core`)
- Application : **Tauri 2 + React / TypeScript**
- Backend privé de données : Rust (axum), PostgreSQL, ClickHouse, Redis, NATS
- Data : Python, Polars, Dagster

## Documentation

- [Document fondateur](docs/plan-fondateur.md) : stack, produits, méthode Theorycrafter ↔ devs, premier sprint.
- [Politique de sécurité](SECURITY.md) : signalement, suivi des CVE et délais de correction.
- [Design system « dark »](docs/design/design-system.md) et [spec des écrans de l'overlay](docs/design/overlay-spec.md).
- [Application Windows](apps/desktop/README.md) : coquille Tauri 2, overlay, mises à jour.
- Bêta fermée : [installer Augur](docs/beta/guide-installation.md), [l'utiliser et signaler un bug](docs/beta/guide-utilisation.md).
- [Architecture](docs/architecture/README.md) : API GGG, décisions d'architecture, contrats entre modules.

## Données de tiers

- Poids de mods PoE 2 : estimations de Krakenbul (Prohibited Library), publiées par [PoE2DB](https://poe2db.tw/us/weightings), sous licence **CC BY-NC-SA 3.0** (pas d'usage commercial). Fichier et attribution : [tools/theorycraft/donnees/](tools/theorycraft/donnees/LICENCE-poids-poe2db.md). Le relevé est embarqué dans l'appli pour Forge ([crates/poe2-core/data/](crates/poe2-core/data/README.md)), et des extraits servent de cas de test ([crates/poe2-core/tests/donnees/forge/](crates/poe2-core/tests/donnees/forge/README.md)), sous la même licence.
- Poids complémentaires : pour les mods que PoE2DB n'a pas mesurés, poids publiés par [Craft of Exile](https://www.craftofexile.com/?game=poe2) (mêmes estimations de Krakenbul), copiés avec leur attribution : [tools/theorycraft/donnees/LICENCE-poids-coe.md](tools/theorycraft/donnees/LICENCE-poids-coe.md).

---

Augur n'est ni affilié à Grinding Gear Games ni approuvé par eux. Path of Exile est une marque de Grinding Gear Games.
