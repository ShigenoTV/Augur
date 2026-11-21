# Architecture d'Augur

| Document | Contenu |
|---|---|
| [api-ggg.md](api-ggg.md) | Cartographie des API GGG pour PoE 2 : endpoints, scopes, OAuth, rate limits, risques |
| [ADR-0001](adr/0001-frontieres-poe2-core.md) | Arborescence du monorepo, frontières de `poe2-core`, partage client / serveur |
| [ADR-0002](adr/0002-budget-manager-rate-limits.md) | Budget manager des rate limits (`crates/augur-ratelimit`) |
| [contrats.md](contrats.md) | Commandes et événements Tauri, DTO partagés (`crates/augur-contracts`) |

Le backend, le CDN de données et l'infrastructure prévus au plan initial ont été retirés (voir [../archive.md](../archive.md)) : l'appli est 100 % locale.

Une décision d'architecture s'écrit en ADR numéroté dans `adr/`, au statut « proposé » jusqu'à sa revue.
