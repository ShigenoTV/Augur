# Build, signature et release Windows

Ce document décrit la chaîne qui va du code à l'installation chez un joueur, et ce qu'il
reste à configurer. Tout est prêt dans le dépôt ; il manque uniquement des comptes et
des secrets, listés à la fin.

## Vue d'ensemble

```
PR / main ──► CI ─────────────► build Windows non signé (artefact 14 j, testeurs internes)

tag vX.Y.Z[-beta.N] ──► Release Windows
                         ├─ vérifie tag = version de l'application
                         ├─ tauri build (NSIS + MSI)
                         │    ├─ chaque binaire signé Authenticode (sign-windows.ps1)
                         │    └─ chaque installeur signé pour l'updater (.sig minisign)
                         ├─ contrôle des signatures, latest.json, SHA256SUMS
                         ├─ dépôt IMMUABLE sur updates.<domaine>/releases/X.Y.Z/
                         └─ release GitHub en brouillon

« Promouvoir une release » (approbation de Max) ──► channels/{beta|stable}/latest.json
                                                     └─ les applications installées se mettent à jour
```

Deux principes : **rien n'atteint les joueurs sans une promotion approuvée**, et **une
version publiée ne se réécrit jamais** (on incrémente).

## Workflows

| Fichier | Déclencheur | Rôle |
|---|---|---|
| `.github/workflows/ci.yml` | PR, push sur `main` | Rust (fmt, clippy, tests), build Windows non signé, Python, OpenTofu, outillage. Chaque job s'active dès que son dossier existe. |
| `.github/workflows/release-windows.yml` | tag `v*.*.*` ou manuel | Build signé, artefacts de mise à jour, dépôt sur le CDN, release brouillon. |
| `.github/workflows/promote-release.yml` | manuel | Bascule d'un canal vers une version déposée, avec garde-fous (pas de bêta en stable, pas de retour en arrière). |
| `.github/workflows/publish-data-pack.yml` | manuel | Publication d'un paquet de données de jeu signé (voir `paquets-de-donnees.md`). |

L'audit des dépendances (CVE) est dans `audit-securite.yml` (PR sécurité), pas ici.

## Canaux et versions

- `vX.Y.Z` → canal **stable** ; `vX.Y.Z-beta.N` → canal **bêta**. Tout autre tag est refusé.
- La version du tag doit être égale à celle de `apps/desktop/src-tauri/tauri.conf.json`
  (ou du `package.json` / `Cargo.toml` qu'il référence).
- L'updater Tauri n'installe qu'une version **supérieure** à celle en place.

## Configuration attendue dans l'application (thread front/desktop)

Dans `apps/desktop/src-tauri/tauri.conf.json` :

```json
{
  "bundle": { "createUpdaterArtifacts": true },
  "plugins": {
    "updater": {
      "pubkey": "<CLÉ PUBLIQUE minisign générée par Max>",
      "endpoints": ["https://updates.<domaine>/channels/stable/latest.json"],
      "windows": { "installMode": "passive" }
    }
  }
}
```

Le canal bêta s'obtient en remplaçant l'endpoint au runtime (option « Rejoindre la bêta »)
via le builder du plugin `tauri-plugin-updater`. La CI force `createUpdaterArtifacts` et
injecte la commande de signature, l'application n'a donc rien d'autre à prévoir.

## Signature Authenticode : quel produit acheter

Depuis juin 2023, les clés des certificats de signature de code doivent être stockées
sur un HSM : on ne peut plus mettre un fichier `.pfx` dans les secrets de la CI. Et depuis
2024, Microsoft ne donne plus de réputation SmartScreen immédiate aux certificats EV :
EV et OV construisent leur réputation de la même façon, par le volume d'installations.

Le pipeline accepte deux modes (variable `AUGUR_SIGNING_MODE`) :

| Mode | Produit | Coût indicatif | Remarques |
|---|---|---|---|
| `azure` (recommandé) | **Azure Artifact Signing** (ex-« Trusted Signing ») | ~10 $/mois | Signature dans le cloud, aucun matériel, intégration Tauri documentée. Éligibilité selon le pays et le statut (entreprise ou particulier) : **à vérifier** sur la page Microsoft au moment de la souscription. |
| `signtool` | Certificat OV/EV d'une AC avec HSM cloud (DigiCert KeyLocker, SSL.com eSigner…) | 300 à 700 €/an | Ajouter l'étape d'installation du KSP du fournisseur dans `release-windows.yml` (emplacement marqué). |
| `none` | Aucun | 0 | Uniquement pour tester le pipeline (`allow_unsigned`) ; rien n'est déposé sur le CDN. |

La validation d'identité prend plusieurs jours dans les deux cas : c'est le chemin
critique pour la Definition of Done « installation sans alerte SmartScreen ».

## Configuration GitHub (Settings du dépôt)

**Environnements** (Settings > Environments) :

- `release` : secrets de signature. Restreindre aux tags `v*`.
- `production` : **Required reviewers = Max**. Protège la promotion des versions.
- `data-production` : Required reviewers = Max (ou le responsable data). Protège la publication des données.

**Secrets**

| Nom | Environnement | Contenu |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | release | Clé privée minisign de l'updater. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | release | Son mot de passe. |
| `AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET` | release | Principal de service autorisé à signer (rôle *Trusted Signing Certificate Profile Signer*, renommé « Artifact Signing » selon la date). |
| `AUGUR_SIGNING_THUMBPRINT` | release | Empreinte du certificat (mode `signtool` seulement). |
| `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY` | release, production, data-production | Jeton R2 limité aux buckets. |
| `CLOUDFLARE_PURGE_TOKEN` | production, data-production | Jeton limité à la purge de cache. |
| `AUGUR_DATA_SIGNING_KEY` | data-production | Clé privée ed25519 des paquets de données. |

**Variables**

| Nom | Exemple |
|---|---|
| `AUGUR_SIGNING_MODE` | `azure` |
| `AZURE_SIGNING_ENDPOINT` | `https://weu.codesigning.azure.net` |
| `AZURE_SIGNING_ACCOUNT`, `AZURE_SIGNING_PROFILE` | noms choisis dans Azure |
| `AUGUR_UPDATES_BASE_URL` | `https://updates.<domaine>` (sortie OpenTofu) |
| `AUGUR_DATA_BASE_URL` | `https://data.<domaine>` (sortie OpenTofu) |
| `R2_ACCOUNT_ID` | identifiant du compte Cloudflare |
| `R2_UPDATES_BUCKET`, `R2_DATA_BUCKET` | `augur-updates`, `augur-data` |
| `CLOUDFLARE_ZONE_ID` | zone du domaine |
| `AUGUR_DATA_PUBLIC_KEYS` | clé(s) publique(s) ed25519 des données, séparées par des virgules |

## Générer les clés (sur le poste de Max, jamais dans un chat ni un dépôt)

```sh
# Clé de l'updater (minisign) : la publique va dans tauri.conf.json
pnpm --dir apps/desktop tauri signer generate -w ~/.tauri/augur-updater.key

# Clé des paquets de données (ed25519) : la publique va dans l'application
pip install cryptography
python tools/datapack/augur_datapack.py keygen
```

Conserver une copie hors ligne des deux clés privées (gestionnaire de mots de passe +
sauvegarde chiffrée). **Perdre la clé de l'updater oblige chaque joueur à réinstaller
l'application à la main.**

## Publier une version

1. Monter la version dans l'application, fusionner sur `main`.
2. `git tag v0.1.0-beta.1 && git push origin v0.1.0-beta.1`.
3. Vérifier le run « Release Windows » et tester l'installeur de la release brouillon sur
   un Windows 11 vierge (SmartScreen, antivirus, mise à jour depuis la version précédente).
4. Lancer « Promouvoir une release » (version, canal) et approuver.
5. Versions majeures : soumettre les installeurs à Microsoft Defender
   (https://www.microsoft.com/wdsi/filesubmission) avant la promotion en stable.

## Retour arrière

Les versions déposées restent sur le CDN. Pour revenir en arrière, publier une version
corrective **supérieure** (ex. 0.3.1 qui reprend le code de 0.3.0) : l'updater refuse
les versions inférieures, et le workflow de promotion aussi. Pour une fonctionnalité
cassée par un patch du jeu, utiliser le kill switch distant plutôt qu'une release.
