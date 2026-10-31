# Pipeline de prix d'Augur (sprint 1, Data)

**Statut :** première version, à relire en revue d'équipe · **Code :** [`data/`](../../data) · **Contrat :** [`contracts/prices.v1.schema.json`](../../contracts/prices.v1.schema.json)

Ce document couvre les tâches Data du sprint 1 du [plan fondateur](../plan-fondateur.md) : collecteur d'échantillons (S1), schéma ClickHouse et nettoyage (S2), indice de prix des devises exposé à l'application (S3). Le modèle de pricing des objets rares (S4) n'est pas encore commencé : il lui faut plusieurs semaines d'historique réel.

## Vue d'ensemble

```
 API GGG ──▶ budget manager ──▶ collecteurs ──▶ stockage brut immuable ──▶ parse ──▶ nettoyage ──▶ indices ──▶ GET /prices/{league}
            (en-têtes X-Rate-   (cx officiel,    (object storage,          (lignes)  (leurres,     (devises,   (contrat v1, ETag,
             Limit-*, 429)       trade curé)      .json.gz, écriture        doublons,  exalts)      requêtes    mode dégradé)
                                                  unique)                                           curées)
```

| Étape | Module | Ce qu'il garantit |
|---|---|---|
| Budget | `ratelimit.py` | Aucune valeur codée en dur : une seule requête « sonde » tant que la politique est inconnue, puis respect de chaque règle `X-Rate-Limit-*` avec une marge d'un hit, alignement sur l'état vu par GGG, blocage sur 429 / `Retry-After` / restriction active. Même contrat (`acquire(clé, priorité)` puis `observe`) que le budget manager Rust du chantier Architecture, qui le remplacera. |
| Client | `ggg.py` | User-Agent `OAuth {clientId}/{version} (contact: {email})`, jeton lu dans l'environnement, aucun cookie émis (donc jamais de `POESESSID`), kill switch vérifié avant chaque requête, arrêt net au premier 429. |
| Sources | `sources/` | `currency_exchange` : API officielle d'échange de devises (agrégats horaires). `trade_sampler` : liste curée de requêtes (`queries/poe2-s1.json`), tri par prix croissant, 20 annonces max par requête (1 search + 2 fetch). |
| Brut | `storage.py` | Chaque réponse est écrite telle quelle, une seule fois, avec son contexte (source, ligue, requête, en-têtes de rate limit). Tout le reste se recalcule à partir de là. |
| Nettoyage | `cleaning.py` | Normalisation en exalts, doublons, réannonces, leurres bas, aberrants hauts, murs de vendeur. On marque au lieu de supprimer, pour auditer. Les noms de compte sont remplacés par une empreinte. |
| Indices | `currency_index.py`, `item_index.py` | Médiane pondérée (volume, fraîcheur), exclusion par z-score modifié (MAD, seuil 3,5), triangulation via divine ou chaos, fourchette 25-75 %. |
| Contrat | `contract.py` | Document v1 validé par JSON Schema, ETag stable tant que les prix ne bougent pas. |
| Orchestration | `dagster_defs.py` | Devises toutes les heures, trade et prix toutes les 30 minutes, partitions par ligue. |

## Le format servi à l'application

Exemple complet : [`contracts/exemples/prices.v1.json`](../../contracts/exemples/prices.v1.json).

- **Tout est en exalts.** L'application convertit en divines à l'affichage avec `currency/divine`.
- **Chaque prix porte sa fourchette, sa taille d'échantillon et un niveau de confiance** (`high`, `medium`, `low`). L'overlay doit afficher « approximatif » en `low` plutôt qu'un chiffre précis.
- **Mode dégradé** (plan, section 1.7) : `fresh_until` dit jusqu'à quand le prix est considéré frais ; au-delà l'application l'affiche avec son horodatage. `frozen: true` signale des prix gelés volontairement.
- **Identifiants stables** `{catégorie}/{id}` : `currency/divine`, `fragment/breach-splinter`, `base/stellar-amulet-ilvl82-normal`.
- Une évolution incompatible crée `prices.v2.schema.json` ; v1 reste servi le temps que les anciennes versions de l'application disparaissent.

## Règles de nettoyage

| Motif | Règle |
|---|---|
| `prix_absent` | Pas de montant positif ou pas de devise. |
| `devise_inconnue` | Devise absente de l'indice des devises de la ligue. |
| `doublon` | Même annonce vue plusieurs fois pour une requête : on garde l'observation la plus récente. |
| `reannonce` | Même vendeur, même requête, même prix unitaire. |
| `mur_vendeur` | Un vendeur détient au moins 50 % d'un carnet d'au moins 6 annonces : seule sa meilleure annonce reste. |
| `leurre_bas` / `aberrant_haut` | z-score modifié du log du prix inférieur à -3,5 ou supérieur à 3,5, calculé sur une annonce par vendeur, à partir de 5 vendeurs distincts. Carnet parfaitement serré : tolérance de ±50 % autour de la médiane. |

Le taux d'exclusion par motif part dans `quality` du document et dans la vue ClickHouse `v_trade_quality_hourly` (tableau de bord qualité, Definition of Done du sprint).

## Points ouverts et risques

1. **Accès à l'API officielle.** D'après la cartographie du chantier Architecture, `service:cxapi` exige un client OAuth confidentiel (`client_credentials`) et GGG n'accepte pas de nouvelles applications en ce moment. Tant que l'enregistrement n'aboutit pas, la source devises ne peut pas tourner en production. Le format de réponse codé ici (`markets`, `volume_traded`, `lowest_ratio`, `highest_ratio`, `next_change_id`) est à confirmer sur la documentation officielle ; le parseur ignore proprement ce qu'il ne comprend pas.
2. **Échantillonnage du trade depuis le backend.** Les endpoints `trade2` ne font pas partie de l'API OAuth documentée. **Décision de Max (31 octobre 2026) : autorisé en volume minimal**, soit les requêtes curées de `queries/poe2-s1.json` toutes les 30 minutes, User-Agent identifiable, arrêt au premier 429. Toute hausse du nombre de requêtes ou de la cadence repasse par sa validation.
3. **Identifiants de devises.** Le code suppose que l'échange de devises et le trade utilisent les mêmes identifiants (`exalted`, `divine`, `chaos`…). À vérifier lors de la cartographie S1.
4. **Stockage objet.** Seul le backend disque local existe ; le backend S3 (R2 ou Hetzner) se branche sur le même contrat `RawStore` quand le chantier DevOps aura provisionné le bucket.
5. **Bascule vers le budget manager central.** Le collecteur Python utilise une implémentation locale du budget. En production, toutes les requêtes du studio doivent partager un seul budget : il faudra exposer le budget manager Rust au collecteur (service local ou bibliothèque), ou réécrire les collecteurs en Rust comme le prévoit le plan.
