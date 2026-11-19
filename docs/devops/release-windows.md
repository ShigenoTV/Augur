# Build, signature et release Windows

Ce document décrit la chaîne qui va du code à l'installation chez un joueur, et ce qu'il
reste à configurer. Tout est prêt dans le dépôt ; il manque uniquement des comptes et
des secrets, listés plus bas.

## Vue d'ensemble

```
PR / main ──► CI ─────────────► build Windows non signé (artefact 14 j, testeurs internes)

tag vX.Y.Z[-beta.N] ──► Release Windows
                         ├─ vérifie tag = version de l'application
                         ├─ tauri build (NSIS + MSI)
                         │    ├─ signature Authenticode optionnelle (désactivée, voir plus bas)
                         │    └─ chaque installeur signé pour l'updater (.sig minisign)
                         ├─ latest.json, SHA256SUMS
                         ├─ fichiers attachés à la release GitHub (immuables : on incrémente)
                         └─ release GitHub en brouillon

« Promouvoir une release » (approbation de Max) ──► stable : release « latest » / bêta : pré-release canal-beta
                                                     └─ les applications installées se mettent à jour
```

Deux principes : **rien n'atteint les joueurs sans une promotion approuvée**, et **une
version publiée ne se réécrit jamais** (on incrémente).

## Workflows

| Fichier | Déclencheur | Rôle |
|---|---|---|
| `.github/workflows/ci.yml` | PR, push sur `main` | Rust (fmt, clippy, tests), build Windows non signé, outillage Python (`tools/`) et scripts de release. |
| `.github/workflows/release-windows.yml` | tag `v*.*.*` ou manuel | Build, artefacts de mise à jour signés, release GitHub en brouillon. |
| `.github/workflows/promote-release.yml` | manuel | Bascule d'un canal vers une version déposée, avec garde-fous (pas de bêta en stable, pas de retour en arrière). |
| `.github/workflows/mise-a-jour-donnees.yml` | manuel | Régénère les données de jeu (RePoE, arbre PoB, Forge) et rend un rapport des différences. |

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
via le builder du plugin `tauri-plugin-updater` (`src/updates.rs`). La CI force `createUpdaterArtifacts` et
injecte la commande de signature, l'application n'a donc rien d'autre à prévoir.

## Hébergement des mises à jour (GitHub Releases)

Les mises à jour sont servies par les releases GitHub du dépôt (public) :

- la release attache les installeurs, leurs `.sig` et `latest.json` à la release brouillon ;
- **canal stable** : l'application interroge
  `https://github.com/ShigenoTV/Augur/releases/latest/download/latest.json`.
  « Promouvoir une release » (canal `stable`) publie la release brouillon et la marque
  « latest » ;
- **canal bêta** (option « Rejoindre la bêta » dans Réglages) : l'application interroge
  `https://github.com/ShigenoTV/Augur/releases/download/canal-beta/latest.json`, puis le
  stable en secours. `canal-beta` est une **pré-release fixe** qui ne porte que ce
  manifeste. « Promouvoir une release » (canal `beta`) publie la release `vX.Y.Z-beta.N`
  en pré-release (jamais « latest » : le canal stable ne la voit pas) et copie son
  `latest.json` sur `canal-beta`. Une promotion stable y copie aussi le sien quand il est
  plus récent, pour que les testeurs reçoivent les versions stables.
- Les bêtas ne sont construites qu'en NSIS : WiX (MSI) refuse une version `X.Y.Z-beta.N`.

## Publier une bêta (sans toucher au stable)

1. Sur une branche (pas besoin de passer par `main`), monter la version à `X.Y.Z-beta.N`
   (`apps/desktop/package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `Cargo.lock`).
   `X.Y.Z` doit être **supérieur** à la version stable en place.
2. Actions > « Release Windows » > Run workflow **sur cette branche**, tag `vX.Y.Z-beta.N`.
3. Actions > « Promouvoir une release », version `X.Y.Z-beta.N`, canal `beta`.
   Le run vérifie à la fin que le canal stable annonce toujours une version stable.
4. Les testeurs ayant coché « Rejoindre la bêta » voient la version au prochain contrôle
   (démarrage, toutes les 6 h, ou « Rechercher des mises à jour »).

Pour sortir de la bêta : décocher l'option. L'application reste sur la bêta installée
jusqu'à la version stable suivante (`X.Y.Z` est supérieur à `X.Y.Z-beta.N`).

## Signature Authenticode : pas de certificat (décision du 2026-10-31)

Max a décidé de **ne pas acheter de certificat de signature de code vérifié**. Le
pipeline publie donc des installeurs **non signés Authenticode** (mode par défaut
`AUGUR_SIGNING_MODE=none`, un avertissement est affiché dans le run).

Ce qui reste signé, gratuitement, avec nos propres clés :

- **les mises à jour** (clé minisign de l'updater Tauri) : une application installée
  n'accepte qu'une mise à jour signée par le studio ;
- l'intégrité des installeurs est publiée dans `SHA256SUMS.txt` sur chaque release.

Conséquences à connaître :

- À l'installation, Windows SmartScreen affiche « Windows a protégé votre ordinateur » ;
  le joueur doit cliquer sur **Informations complémentaires > Exécuter quand même**.
  Prévoir ce geste dans le guide d'installation des testeurs. Les mises à jour
  automatiques suivantes ne déclenchent pas cet écran.
- La Definition of Done du sprint 1 « installation sans alerte SmartScreen » n'est pas
  atteignable sans certificat et doit être amendée.
- Les antivirus signalent plus souvent les binaires non signés : soumettre chaque
  version à Microsoft Defender (gratuit, lien plus bas) avant de la promouvoir.

Si la décision change, le pipeline sait déjà signer : `AUGUR_SIGNING_MODE=azure`
(Azure Artifact Signing) ou `signtool` (certificat sur HSM cloud), avec les secrets
listés ci-dessous. Rien d'autre à modifier.

## Configuration GitHub (Settings du dépôt)

**Environnements** (Settings > Environments) :

- `release` : clé de l'updater (et secrets Authenticode si un jour utilisés). Restreindre aux tags `v*`.
- `production` : **Required reviewers = Max**. Protège la promotion des versions.

**Secrets**

| Nom | Environnement | Contenu |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | release | Clé privée minisign de l'updater. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | release | Son mot de passe. |
| `AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET` | release (optionnel) | Principal de service autorisé à signer (rôle *Trusted Signing Certificate Profile Signer*, renommé « Artifact Signing » selon la date). |
| `AUGUR_SIGNING_THUMBPRINT` | release (optionnel) | Empreinte du certificat (mode `signtool` seulement). |

**Variables**

| Nom | Exemple |
|---|---|
| `AUGUR_SIGNING_MODE` | `none` (défaut) ; `azure` ou `signtool` si un certificat est pris plus tard |
| `AZURE_SIGNING_ENDPOINT` | `https://weu.codesigning.azure.net` |
| `AZURE_SIGNING_ACCOUNT`, `AZURE_SIGNING_PROFILE` | noms choisis dans Azure |

## Générer les clés (sur le poste de Max, jamais dans un chat ni un dépôt)

```sh
# Clé de l'updater (minisign) : la publique va dans tauri.conf.json
pnpm --dir apps/desktop tauri signer generate -w ~/.tauri/augur-updater.key
```

Conserver une copie hors ligne de la clé privée (gestionnaire de mots de passe +
sauvegarde chiffrée). **Perdre la clé de l'updater oblige chaque joueur à réinstaller
l'application à la main.**

## Publier une version

0. Une seule fois : remplacer dans `tauri.conf.json` la clé publique provisoire de l'updater
   et le domaine `updates.augur.invalid` (constante `STABLE_ENDPOINT` de `src/updates.rs`).
   La release refuse de construire tant que ce n'est pas fait, car une version publiée
   avec une clé factice ne pourrait plus jamais se mettre à jour.
1. Monter la version dans l'application, fusionner sur `main`.
2. `git tag v0.1.0-beta.1 && git push origin v0.1.0-beta.1`.
3. Vérifier le run « Release Windows » et tester l'installeur de la release brouillon sur
   un Windows 11 vierge (SmartScreen, antivirus, mise à jour depuis la version précédente).
4. Lancer « Promouvoir une release » (version, canal) et approuver.
5. Chaque version (installeurs non signés) : soumettre les installeurs à Microsoft Defender
   (https://www.microsoft.com/wdsi/filesubmission) avant la promotion en stable.

## Retour arrière

Les versions publiées restent sur GitHub Releases. Pour revenir en arrière, publier une version
corrective **supérieure** (ex. 0.3.1 qui reprend le code de 0.3.0) : l'updater refuse
les versions inférieures, et le workflow de promotion aussi. Pour une fonctionnalité
cassée par un patch du jeu, utiliser le kill switch distant plutôt qu'une release.
