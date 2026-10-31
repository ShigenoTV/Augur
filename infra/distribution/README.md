# Infra de distribution (OpenTofu)

Crée, pour un environnement donné, les deux buckets Cloudflare R2 d'Augur et leurs
sous-domaines publics, plus la règle de cache.

| Ressource | Production | Staging |
|---|---|---|
| Mises à jour de l'application | `augur-updates` → `updates.<domaine>` | `augur-updates-staging` → `updates-staging.<domaine>` |
| Paquets de données de jeu | `augur-data` → `data.<domaine>` | `augur-data-staging` → `data-staging.<domaine>` |

## Prérequis (Max)

1. Un compte Cloudflare avec R2 activé et le domaine d'Augur ajouté comme zone.
2. Un jeton API Cloudflare pour OpenTofu (permissions : *Workers R2 Storage: Edit*,
   *Zone: Cache Rules: Edit*, *Zone: DNS: Edit* sur la zone d'Augur), exporté en
   `CLOUDFLARE_API_TOKEN` sur le poste qui applique.
3. Renseigner `account_id`, `zone_id` et `domain` dans `envs/*.tfvars`.

## Appliquer

```sh
cd infra/distribution
tofu init
tofu workspace new staging    # une fois
tofu workspace select staging
tofu plan  -var-file=envs/staging.tfvars
tofu apply -var-file=envs/staging.tfvars
```

Même chose avec `production`. Les sorties donnent les valeurs à reporter dans les
variables GitHub (voir `docs/devops/release-windows.md`).

L'état est local tant que le backend distant n'est pas activé dans `versions.tf`
(bucket R2 `augur-tofu-state`, créé à la main une fois). À faire avant que
quelqu'un d'autre que Max applique l'infra.

## Jetons pour la CI

Créer dans le tableau de bord R2 (*Manage R2 API Tokens*) un jeton **Object Read & Write**
limité aux buckets de l'environnement, et le stocker dans les secrets GitHub
`R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY`. Pour la purge de cache, un jeton
Cloudflare séparé limité à *Zone: Cache Purge* (`CLOUDFLARE_PURGE_TOKEN`).
