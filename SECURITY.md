# Politique de sécurité d'Augur

Augur s'installe sur le PC des joueurs, lit leur presse-papier et détient leurs jetons OAuth GGG. Une vulnérabilité chez nous, c'est un risque direct pour leur compte Path of Exile. Cette politique fixe comment nous suivons et corrigeons les CVE.

## Signaler une vulnérabilité

Ne publiez pas d'issue publique. Utilisez le signalement privé de GitHub : onglet **Security** du dépôt, puis **Report a vulnerability**. Nous accusons réception sous 72 heures.

## Détection

| Source | Outil | Fréquence |
|---|---|---|
| Dépendances Rust (cœur, application, backend) | `cargo audit` (base RustSec) | Chaque PR, chaque push sur `main`, tous les jours |
| Dépendances front (React, Tauri JS) | `pnpm audit` (ou `npm audit` si `package-lock.json`) | Idem |
| Dépendances data (Python) | `pip-audit` (base PyPA / OSV) | Idem |
| Toutes les dépendances et les GitHub Actions | Alertes et PR Dependabot | En continu, PR groupées chaque lundi |
| Runtime embarqué | Bulletins Tauri et WebView2 (Microsoft Edge) | Veille hebdomadaire |

Une nouvelle CVE peut toucher du code qui n'a pas changé : c'est pourquoi l'audit tourne aussi chaque jour, pas seulement sur les PR.

## Délais de correction

Les délais courent à partir de la publication de la CVE ou de son signalement.

| Sévérité (CVSS) | Correctif publié dans |
|---|---|
| Critique (9,0 et plus) | 48 heures, avec mise à jour forcée de l'application |
| Haute (7,0 à 8,9) | 7 jours |
| Moyenne (4,0 à 6,9) | 30 jours |
| Basse | Prochaine version planifiée |

Une dépendance vulnérable sans correctif disponible est remplacée, retirée ou désactivée par le kill switch distant. Une exception temporaire n'est acceptée que si elle est documentée dans `audit.toml` (Rust) ou dans la PR concernée, avec la raison et une date de réexamen.

## Règles de sécurité propres à l'application

- **Mises à jour et paquets de données signés** par les clés du studio : l’updater Tauri refuse toute mise à jour dont la signature ne correspond pas. L’installeur n’a pas de signature Authenticode (décision de Max du 2026-10-31) ; SmartScreen avertit à l’installation.
- **Capabilities Tauri minimales** : aucun plugin `shell`, aucun accès système de fichiers hors des dossiers de l'application et de `Client.txt`.
- **Content Security Policy stricte** dans la WebView : aucun script distant, aucun `eval`.
- Jetons OAuth stockés dans le **Gestionnaire d'identification Windows**, jamais en clair sur le disque ni dans les logs.
- Jamais de `POESESSID` demandé ni stocké.
- Aucun secret dans le dépôt : clés de signature et identifiants OAuth du studio dans les secrets de la CI uniquement.
