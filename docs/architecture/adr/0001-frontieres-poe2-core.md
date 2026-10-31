# ADR-0001 : frontières de `poe2-core` et partage client / serveur

- **Statut :** proposé (à accepter en revue d'équipe)
- **Date :** 31 octobre 2026
- **Auteur :** architecte

## Contexte

Le plan fondateur fait de `poe2-core` le cœur commun des trois modules (Copilot, Ledger, Forge), embarqué en natif dans l'appli Windows et réutilisé par le backend (pricing, ingestion). Plusieurs chantiers démarrent en parallèle et dépendent de ses frontières : moteur de calcul, overlay, données de prix, theorycraft, DevOps.

## Décision

### 1. Arborescence du monorepo

```
Cargo.toml               workspace (members = ["crates/*", "apps/desktop/src-tauri"]), Cargo.lock unique à la racine
crates/
  poe2-core/             moteur : parseur, calcul, modèle de données du jeu (pur)
  augur-contracts/       DTO partagés appli ↔ interface ↔ backend (serde + ts-rs)
  augur-ratelimit/       budget manager des rate limits GGG
apps/
  desktop/               appli Tauri 2 (src-tauri en Rust + front React)
packages/
  ui/                    @augur/ui : design system et vues React
  contracts/             @augur/contracts : types TS générés depuis augur-contracts
tests/golden/            cas golden YAML du Theorycrafter
docs/architecture/       ADR, carte des API, contrats
```

Gestionnaire JS : **pnpm** (workspaces `apps/*`, `packages/*`).

### 2. `poe2-core` est une bibliothèque pure

- **Aucune E/S** : ni réseau, ni fichier, ni horloge, ni variable d'environnement. Les données du jeu lui sont **passées** sous forme d'une structure déjà chargée (paquet versionné par patch) ; c'est l'appli ou le backend qui lit le disque ou le CDN.
- **Aucune dépendance** à Tauri, tokio, à un client HTTP ni à `augur-contracts`. Il expose ses propres types de domaine ; la conversion vers les DTO se fait dans `apps/desktop/src-tauri` (et dans le backend).
- **Déterministe** : même entrée et même paquet de données donnent le même résultat et la même trace de calcul, sur Windows comme sur le Linux de la CI.
- **Synchrone** : le parallélisme (`rayon`) est permis pour la recherche d'upgrades, l'asynchrone appartient à l'appelant.
- **Versionné par patch** : chaque résultat porte la version des données utilisée (`data_patch`).

Conséquence : `poe2-core` se teste entièrement en CI Linux avec `cargo test`, les cas golden s'exécutent sans lancer l'appli, et le backend peut le réutiliser sans tirer de dépendance desktop.

### 3. Ce qui tourne où

| Fonction | Où | Pourquoi |
|---|---|---|
| Parsing d'objet, delta DPS/EHP, simulation de craft | **Appli, en local** | Latence < 100 ms, hors ligne, aucune charge serveur le jour d'un lancement |
| Recherche de trade du joueur | **Appli** (IP du joueur) | Le budget de rate limit est celui du joueur |
| Import de personnage | **Appli** (OAuth public, PKCE) ou code PoB | Les jetons du joueur ne quittent jamais son poste |
| Ligues, échange de devises, échantillonnage du trade | **Backend** (OAuth confidentiel) | Scopes `service:*` interdits aux clients publics |
| Modèles de prix, indices de devises | **Backend**, servis par API cacheable | Calcul sur l'historique agrégé |
| Paquets de données de jeu | **CDN**, URL immuables par patch, manifeste signé | Hors ligne avec le dernier paquet, charge serveur nulle |
| Kill switch | **Manifeste distant signé**, lu par l'appli | Couper une fonctionnalité sans republier |

Règle par défaut : **ce qui peut se calculer en local se calcule en local.**

### 4. Contrats entre modules

- Appli ↔ interface : commandes et événements Tauri typés par `augur-contracts`, voir [contrats.md](../contrats.md).
- Appli ↔ backend : API REST versionnée (`/v1/…`), spécifiée en OpenAPI au sprint 1, semaine 3.
- Tout appel vers GGG passe par `augur-ratelimit`, voir [ADR-0002](0002-budget-manager-rate-limits.md).

## Alternatives écartées

- **`poe2-core` qui lit lui-même ses fichiers de données** : plus simple au début, mais rend le crate impossible à réutiliser côté backend et ses tests dépendants du disque.
- **Types de contrat définis côté TypeScript** : deux sources de vérité ; on garde Rust comme source et on génère le TS.
- **Calcul côté serveur** : contraire au plan (latence, hors ligne, pic de lancement).

## Conséquences

- Le chantier moteur fournit une API publique de domaine ; le chantier overlay écrit la couche de conversion vers les DTO.
- Une dépendance d'E/S ajoutée à `poe2-core` est un refus en revue.
- Toute évolution incompatible d'un DTO incrémente `CONTRACT_VERSION`.
