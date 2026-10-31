# Format des paquets de données de jeu (v1)

Les tables extraites du client PoE 2 pour chaque patch (mods, stats, bases d'objets,
gemmes, passifs…) sont distribuées à l'application par le CDN `data.<domaine>`.
Outil de référence : [`tools/datapack/augur_datapack.py`](../../tools/datapack/augur_datapack.py).
Vecteur de test pour le vérificateur Rust : [`tools/datapack/testdata/vector/`](../../tools/datapack/testdata/vector/).

## Objectifs

- **Charge serveur quasi nulle** le jour d'un patch : tout est statique et mis en cache au bord.
- **Intégrité et authenticité** : l'application ne charge que des données signées par le studio.
- **Hors ligne** : l'application garde le dernier paquet vérifié et fonctionne sans réseau.
- **Rejouabilité** : un paquet de patch reste disponible indéfiniment (URL immuables).

## Arborescence sur le CDN

```
{game}/channels/{channel}.json               pointeur signé (seul objet modifiable, cache 60 s)
{game}/manifests/{patch}/{sha256}.json       manifeste du paquet (immuable)
{game}/manifests/{patch}/{sha256}.json.sig   signature du manifeste (immuable)
{game}/blobs/{sha256}.{ext}                  fichiers de données adressés par empreinte (immuables)
```

`game` vaut `poe2`, `channel` vaut `stable` ou `beta`. Les blobs inchangés d'un patch à
l'autre gardent la même URL : l'application ne retélécharge que ce qui a changé.

## Pointeur de canal

Une enveloppe JSON qui porte la charge utile et sa signature dans un seul objet, pour
que la bascule soit atomique :

```json
{"alg":"ed25519","key_id":"56475aa75463474c","payload":"<base64>","sig":"<base64>"}
```

`payload` décodé (JSON canonique) :

```json
{"channel":"stable","game":"poe2","manifest":"manifests/0.4.1/<sha256>.json",
 "manifest_sha256":"<sha256>","patch":"0.4.1","published_at":"2026-01-01T00:00:00Z",
 "schema":1,"sequence":100}
```

## Manifeste

```json
{"created_at":"…","files":[{"name":"tables/mods.json","path":"blobs/<sha256>.json",
 "sha256":"<sha256>","size":61}],"game":"poe2","min_app_version":"0.2.0",
 "patch":"0.4.1","schema":1}
```

`min_app_version` est optionnel : une application plus ancienne garde son paquet
actuel et propose la mise à jour de l'application.

## Signatures

- Algorithme **ed25519** sur les octets exacts du fichier (JSON canonique : clés triées,
  séparateurs compacts, UTF-8, saut de ligne final).
- Fichier `.sig` / enveloppe : `{"alg":"ed25519","key_id":…,"sig":…}`.
- `key_id` = 16 premiers caractères hex du SHA-256 de la clé publique brute (32 octets).
- L'application embarque **une liste** de clés publiques de confiance : on peut faire
  tourner la clé en publiant avec la nouvelle tant que les deux sont embarquées.
- Clé distincte de la clé de mise à jour de l'application (Tauri/minisign).

## Algorithme de vérification côté application

1. Télécharger `channels/{channel}.json`, décoder `payload`, vérifier la signature avec
   une clé de confiance dont le `key_id` correspond.
2. Vérifier `schema == 1`, `game` et `channel` attendus, et `sequence` **supérieure ou
   égale** à la dernière séquence acceptée (stockée localement) : anti-rollback.
3. Si `manifest_sha256` est déjà installé, terminer.
4. Télécharger le manifeste, vérifier que son SHA-256 vaut `manifest_sha256` et que son
   nom de fichier est cette empreinte, puis vérifier sa signature `.sig`.
5. Télécharger les blobs absents du cache local, vérifier taille et SHA-256 de chacun.
6. Basculer atomiquement sur le nouveau paquet, mémoriser la séquence. En cas d'échec à
   n'importe quelle étape : garder l'ancien paquet et réessayer plus tard.

## Publication

Workflow GitHub Actions **« Publier un paquet de données »** : il prend l'artefact
produit par le pipeline data, construit et signe le paquet, vérifie la chaîne avec les
clés publiques (pas la clé privée), dépose les objets immuables, puis bascule le canal.
Le jour d'un patch : publier d'abord sur `beta`, vérifier, puis sur `stable`.
