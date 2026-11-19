# Éléments archivés (ménage du 19/11/2026)

Le plan initial prévoyait un backend de données et un CDN de paquets de données. L'application
est restée **100 % locale** (Tauri 2, Windows) : les données de jeu sont embarquées dans
`crates/poe2-core/data/`, les prix viennent de poe.ninja et du trade depuis le poste du joueur,
les mises à jour passent par GitHub Releases. Ces éléments n'étaient branchés à rien et ont été
retirés plutôt que maintenus.

Tout reste récupérable dans l'historique git. Dernier commit qui les contient : `174e17c`
(Version 0.1.30). Pour en récupérer un : `git checkout 174e17c -- <chemin>`.

## Retiré

| Chemin | Ce que c'était | Pourquoi ce n'était pas utilisé |
|---|---|---|
| `data/` | Pipeline de prix Python (Polars, Dagster, schéma ClickHouse, échantillonnage trade, indices de devises) et ses tests | Aucun serveur ne le fait tourner ; l'appli ne lit aucune API de prix maison |
| `contracts/prices.v1.schema.json` (+ exemple) | Contrat JSON de l'API `GET /v1/prices` du backend | Seul `data/` l'utilisait |
| `tools/datapack/` | Construction, signature ed25519 et vérification des paquets de données pour le CDN | L'appli ne télécharge aucun paquet : les données sont compilées dedans |
| `.github/workflows/publish-data-pack.yml` | Publication d'un paquet sur Cloudflare R2 | Jamais lancé ; variables et secrets R2 jamais définis |
| `infra/distribution/` | OpenTofu des buckets R2 et domaines `updates.` / `data.` | Pas de domaine ni de compte Cloudflare |
| `docs/devops/paquets-de-donnees.md`, `docs/data/pipeline-prix.md` | Documentation des deux éléments ci-dessus | |
| Étapes « CDN R2 » de `release-windows.yml` et `promote-release.yml` (dépôt sur R2, bascule de canal, purge du cache) | Hébergement des mises à jour sur Cloudflare | Variables `R2_*` jamais définies : les mises à jour passent par GitHub Releases |
| Jobs CI « Python (ruff, pytest) » et « OpenTofu (fmt, validate) », étape « Tests des paquets de données » | Ne testaient que `data/`, `infra/` et `tools/datapack/` | |

## Gardé

- `tools/gamedata/` : commande « mise à jour des données » (RePoE, arbre PoB, Forge, stats de trade, PoE 1), reconstruction du texte des mods, workflow `mise-a-jour-donnees.yml`, veille RePoE.
- `tools/theorycraft/` (poids poe2db / Craft of Exile, calculateur de craft, cas golden) et `tools/pob/` (paquet Path of Building PoE 1).
- `scripts/release/`, `release-windows.yml`, `promote-release.yml`, `audit-securite.yml` : seules les étapes « CDN R2 » des deux workflows de release sont retirées. Elles ne s'exécutaient que si les variables `R2_*` étaient définies, ce qui n'a jamais été le cas : le chemin suivi par chaque release (GitHub Releases) est inchangé.
- `crates/augur-contracts` et `crates/augur-ratelimit` : utilisés par l'appli (commandes Tauri, limites du trade).
