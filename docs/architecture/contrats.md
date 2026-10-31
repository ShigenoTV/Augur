# Contrats entre modules

**Source de vérité :** `crates/augur-contracts` (Rust). Les types TypeScript sont générés par `ts-rs` dans `packages/contracts/src/generated/` et exposés par le paquet `@augur/contracts`.

```bash
cargo test -p augur-contracts        # régénère les .ts
pnpm --filter @augur/contracts generate   # équivalent depuis le front
```

La CI doit vérifier que `git diff --exit-code packages/contracts` reste vide après génération.

## Règles

- JSON et TypeScript en `camelCase` ; énumérations en `camelCase` (`"rateLimited"`, `"verified"`).
- Pas d'entier 64 bits : durées en millisecondes `u32`, dates en chaînes RFC 3339.
- Un champ optionnel vaut `null`, jamais absent.
- Ajouter un champ est rétrocompatible ; renommer ou supprimer incrémente `CONTRACT_VERSION` (actuellement **1**).
- `poe2-core` ne dépend pas de ce crate (voir [ADR-0001](adr/0001-frontieres-poe2-core.md)).

## Commandes Tauri

| Commande | Arguments | Retour | Notes |
|---|---|---|---|
| `parse_clipboard_item` | `{ text: string }` | `ParsedItem` | Aussi déclenchée en interne sur Ctrl+C, qui émet ensuite `augur://item-copied` |
| `get_overlay_state` | — | `OverlayState` | Au montage de l'overlay |
| `evaluate_item` | `{ item: ParsedItem }` | `ItemDelta` | Objectif < 100 ms ; `support` pilote l'état « vérifié / approximatif / non supporté » |
| `estimate_price` | `{ item: ParsedItem }` | `PriceEstimate` | Peut attendre le budget manager ; l'UI écoute `augur://rate-limit-changed` pendant l'attente |
| `get_rate_limit_status` | — | `RateLimitStatus` | File visible |
| `get_feature_flags` | — | `FeatureFlags` | Kill switch ; une fonctionnalité absente vaut désactivée |

Toute commande rejette avec un `AppError { code, message, retryAfterMs }`. Le `message` est en français et affichable tel quel.

## Événements

| Événement | Charge |
|---|---|
| `augur://item-copied` | `ParsedItem` |
| `augur://zone-changed` | `ZoneChanged` |
| `augur://rate-limit-changed` | `RateLimitStatus` |
| `augur://feature-flags-changed` | `FeatureFlags` |

Les noms sont aussi exportés en constantes : `augur_contracts::{commands, events}` côté Rust, `Commands` et `Events` dans `@augur/contracts`.

## Types

| Type | Rôle |
|---|---|
| `ParsedItem`, `ItemMod`, `ModKind`, `Affix`, `Rarity` | Objet lu au presse-papier |
| `ItemDelta`, `StatDelta`, `ResistanceDelta`, `Element`, `MechanicNote` | Résultat du calcul |
| `SupportLevel` | `verified`, `approximate`, `unsupported` |
| `Confidence` | `confirmed`, `measured`, `assumed` (fiches du Theorycrafter) |
| `PriceEstimate`, `PriceSource` | Prix ; `stale` affiche l'horodatage au lieu de masquer le prix |
| `RateLimitStatus`, `PolicyStatus`, `RuleWindowState` | Budget GGG |
| `OverlayState`, `CharacterSummary`, `CharacterSource` | État de l'overlay |
| `FeatureFlags` | Kill switch |
| `AppError`, `ErrorCode` | Erreurs |

## À venir

- API backend `/v1/…` en OpenAPI (sprint 1, semaine 3), dont `GET /v1/prices/{league}`.
- Format des paquets de données de jeu, aligné sur le manifeste signé du chantier DevOps.
