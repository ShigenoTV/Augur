# Augur : document fondateur technique

**Auteur :** AI Tech Lead · **Destinataire :** Max (fondateur) · **Version :** 1.1, 31 octobre 2026
**Statut :** proposition à valider en revue d'équipe
**Périmètre :** le studio livre **uniquement un logiciel Windows**. Aucun site web public : l'overlay et le planificateur de builds vivent dans la même application desktop, et nos serveurs ne servent qu'à alimenter cette application en données.

> Ce document fixe la stack, le portefeuille produit initial, la méthode de travail Theorycrafter ↔ Devs et le premier sprint d'un mois. Tout ce qui touche aux API et aux règles de Grinding Gear Games (GGG) doit être revérifié sur la documentation officielle (`pathofexile.com/developer/docs`) au moment de l'implémentation : GGG fait évoluer ses endpoints, ses scopes OAuth et ses limites sans préavis long, et la couverture PoE 2 de certains endpoints reste plus restreinte que celle de PoE 1.

---

## 0. Les contraintes qui dictent tout le reste

Avant de parler de technologies, voici les cinq réalités propres à l'écosystème PoE 2 qui conditionnent chaque choix de ce document.

1. **Les rate limits de GGG sont la ressource la plus rare du studio.** Les endpoints de trade (`/api/trade2/search/{league}` puis `/api/trade2/fetch/{ids}`) et les endpoints OAuth renvoient des politiques dynamiques via les en-têtes `X-Rate-Limit-Policy`, `X-Rate-Limit-Rules`, `X-Rate-Limit-{Rule}` et `X-Rate-Limit-{Rule}-State`, et `Retry-After` en cas de dépassement. Un dépassement entraîne un timeout de plusieurs minutes, et des abus répétés peuvent faire révoquer notre client OAuth. Conséquence : **le rate limiter n'est pas une bibliothèque qu'on ajoute, c'est un composant d'architecture**, piloté par les en-têtes renvoyés (jamais par des valeurs codées en dur).
2. **Les recherches de trade « utilisateur » doivent partir de la machine de l'utilisateur.** Si notre backend proxifie les recherches de 30 000 joueurs, nous concentrons sur une seule IP un budget prévu pour un seul joueur. L'overlay interroge donc le site de trade directement depuis le poste client ; notre backend ne consomme l'API que pour ses propres besoins d'ingestion, avec son propre client OAuth.
3. **Les règles de GGG sur les outils tiers sont non négociables.** Principes à intégrer dès la conception (à relire dans les Terms of Use et les communications officielles) :
   - une action serveur par pression de touche, aucune automatisation du gameplay, des achats ou des whispers ;
   - aucune lecture ou écriture de la mémoire du client, aucune injection, aucune modification des fichiers du jeu ;
   - les sources tolérées en pratique sont le **presse-papier** (Ctrl+C / Ctrl+Alt+C sur un objet), le fichier de log **`Client.txt`**, la **capture d'écran / lecture de pixels**, et les **API officielles**, OAuth en tête ;
   - **ne jamais demander ni stocker le `POESESSID`** d'un utilisateur : c'est un cookie de session équivalent à un mot de passe. Tout accès au compte passe par OAuth (enregistrement de l'application auprès de GGG, scopes `account:profile`, `account:characters`, `account:stashes`, etc.).
   - User-Agent obligatoire et identifiable pour toute requête : `OAuth {clientId}/{version} (contact: {email})`.
4. **Le lancement de ligue est un pic de charge, mais surtout un pic d'incertitude.** Les patch notes arrivent quelques jours avant, les fichiers de données (`.datc64` dans les bundles) changent le jour J, et les schémas peuvent casser. Le trade est instable les premières heures et GGG peut le restreindre. Notre avantage concurrentiel se joue sur ces 72 heures : données à jour en moins de 6 heures après le patch, et infrastructure qui ne tombe pas.
5. **PoE 2 est en évolution rapide.** Les mécaniques (Spirit, sets d'armes, gemmes de soutien limitées à un exemplaire par personnage, meta-gemmes, buildup d'ailments, Heavy Stun, Electrocute, etc.) bougent d'un patch à l'autre. Le moteur de calcul doit donc être **piloté par les données et par les tests**, pas codé en dur.

---

## 1. La stack technique

### 1.1 Vue d'ensemble

```
   ┌────────────────────────────────────────────────────────┐
   │  Application Windows (Tauri 2)                         │
   │   ┌──────────────────┐   ┌──────────────────────────┐  │
   │   │ Overlay en jeu   │   │ Fenêtre planificateur    │  │
   │   │ (Ctrl+C, hotkeys)│   │ (builds, arbre, craft)   │  │
   │   └────────┬─────────┘   └────────────┬─────────────┘  │
   │            └────────────┬─────────────┘                │
   │               ┌─────────▼─────────┐                    │
   │               │ poe2-core (Rust)  │ parseur, calcul,   │
   │               │ natif, en local   │ données du jeu     │
   │               └───────────────────┘                    │
   └──────┬──────────────────────────────────┬──────────────┘
  trade   │ (IP de l'utilisateur)            │ HTTPS : prix, données de patch
          ▼                                  ▼
   pathofexile.com              ┌─────────────────────────────┐
          ▲                     │ Backend privé (Rust, axum)  │
   OAuth  │                     │ Postgres, ClickHouse, Redis,│
   studio │                     │ stockage objet              │
   ┌──────┴───────────┐         │ (aucune interface web)      │
   │ Ingestion (Rust) │────────▶│                             │
   └──────────────────┘         └──────────────▲──────────────┘
                                               │ modèles de prix
                                      ┌────────┴────────┐
                                      │ Data (Python,   │
                                      │ Polars, Dagster)│
                                      └─────────────────┘
```

**Décision structurante : une seule application Windows qui embarque le cœur de calcul Rust en natif.** L'overlay et le planificateur partagent le même processus, le même moteur et le même personnage chargé : un objet affiche forcément le même delta de DPS dans les deux vues. Tout le calcul se fait en local, ce qui donne une latence quasi nulle, un fonctionnement hors ligne et une charge serveur minimale le jour du lancement de ligue.

### 1.2 Le cœur partagé : `poe2-core`

| Composant | Choix | Justification |
|---|---|---|
| Langage | **Rust** (édition 2024) | Performance déterministe, intégration native directe dans Tauri, pas de GC (calculs répétés des milliers de fois pour les recherches d'upgrades, parallélisés avec `rayon` sur les cœurs du PC du joueur). |
| Données du jeu | Pipeline d'extraction des `.datc64` (schéma communautaire type `dat-schema` / poe-dat-viewer) → JSON/MessagePack versionnés par patch | Tables `Mods`, `Stats`, `BaseItemTypes`, `GrantedEffects*`, `PassiveSkills`… Versionner par numéro de patch permet de rejouer un calcul « tel qu'il était en 0.x.y ». |
| Parseur d'objets | Grammaire sur le texte Ctrl+C / Ctrl+Alt+C, mapping texte → `stat_id` via les descriptions de stats extraites du jeu | Le format avancé (Ctrl+Alt+C) expose tiers et préfixes/suffixes : indispensable pour le craft et le pricing. |
| Moteur de calcul | Graphe de modificateurs typés (base, added, increased/reduced, more/less, conversions, overrides), évaluation paresseuse avec **trace de calcul** | La trace (« breakdown ») est l'outil de travail du Theorycrafter (voir section 3). |
| Précision | Arithmétique flottante maîtrisée et règles d'arrondi explicites, documentées par mécanique | Les écarts d'arrondi sont la première source de « votre outil est faux ». |
| Tests | `cargo test` + fixtures YAML « golden » + tests de propriétés (`proptest`) | Le Theorycrafter alimente les fixtures sans écrire de Rust. |

**Rôles concernés :** Architecte (frontières du crate, API publique stable), Devs back (implémentation), Theorycrafter (spécifications et fixtures), Data (export du modèle de mods pour le pricing).

### 1.3 L'application Windows : overlay en jeu

| Sujet | Choix | Justification |
|---|---|---|
| Framework | **Tauri 2** (WebView2, déjà présent sur Windows 10/11) | Binaire de 10 à 20 Mo contre plus de 150 Mo pour Electron, RAM bien plus faible (un joueur PoE 2 a déjà le jeu, OBS et Discord ouverts), backend Rust qui appelle `poe2-core` en natif. |
| UI | **React 19 + TypeScript**, Zustand, TanStack Query, Tailwind avec tokens de design | Écosystème de composants le plus riche, recrutement facile ; une seule base de composants pour l'overlay et le planificateur (tooltip d'objet, affichage des mods). |
| Intégration Windows | Crate `windows` (Win32) pour détecter la fenêtre du jeu, sa position et son focus, gestion du DPI par écran | Le panneau doit se coller au bon écran et disparaître quand le jeu perd le focus. |
| Fenêtrage | Fenêtre transparente, toujours au premier plan, click-through hors zones actives ; jeu en mode « fenêtré sans bordure » requis | Le plein écran exclusif empêche tout overlay propre : à documenter dans l'onboarding. |
| Entrées | Raccourcis globaux (plugin Tauri global-shortcut), lecture presse-papier, tail de `Client.txt` (changements de zone, whispers, niveau) | Uniquement des sources tolérées par GGG. Aucune action envoyée au jeu sans pression de touche explicite, et jamais plus d'une. |
| Trade | Requêtes HTTP depuis le client, rate limiter local qui respecte les en-têtes, file d'attente visible pour l'utilisateur | Le budget de rate limit est celui de l'utilisateur ; on l'affiche au lieu de le cacher. |
| Plateforme | **Windows 10/11 x64 uniquement** | Un seul OS à tester et à supporter. `Client.txt` est détecté automatiquement selon le client (Steam ou standalone). |
| Distribution | Installeur **MSI/NSIS signé** (certificat de signature de code, idéalement EV, pour éviter les alertes SmartScreen), updater Tauri, canal stable + canal bêta, **kill switch** distant par fonctionnalité | Un exécutable non signé déclenche SmartScreen et fait fuir les joueurs. Le jour d'un patch qui casse le format du presse-papier, on désactive la fonctionnalité sans republier. |
| Antivirus | Soumission de chaque version majeure aux éditeurs (Microsoft Defender en priorité) | Les overlays avec raccourcis globaux et lecture du presse-papier sont souvent signalés à tort. |
| C++ ? | Non, sauf besoin ponctuel | Aucune fonctionnalité légitime ne justifie de descendre plus bas que Rust : ce qui l'exigerait (lecture mémoire, hooks) est justement interdit. |

**Rôles concernés :** Devs front/desktop (implémentation), UX/UI (lisibilité en surimpression), Architecte (rate limiter local, kill switch), DevOps (signature, CDN de mises à jour).

### 1.4 L'application Windows : planificateur de builds (type Path of Building)

Le planificateur est une **fenêtre classique de la même application**, ouverte depuis l'icône de la barre des tâches ou un raccourci, et non un site.

| Sujet | Choix | Justification |
|---|---|---|
| Calcul | `poe2-core` en natif, recalculs dans un thread dédié (commandes Tauri asynchrones) | L'interface reste fluide pendant les recalculs, sans le coût du WebAssembly. |
| Arbre passif | Rendu **WebGL via PixiJS** dans la WebView (ou Canvas 2D en repli) | L'arbre de PoE 2 compte des milliers de nœuds ; le DOM/SVG s'écroule au zoom. |
| Persistance | Fichiers de build locaux (SQLite embarqué via `rusqlite` dans `%APPDATA%`), export/import de fichiers | Fonctionne hors ligne, aucune donnée de build ne dépend de nos serveurs. |
| Partage | **Codes de build** copiables (format compact compressé en base64), collables dans Discord ou Reddit | Le partage passe par des codes, comme dans la culture PoE, sans site d'hébergement de builds. |
| Interopérabilité | Import des codes Path of Building PoE 2 (XML compressé et encodé en base64) et import de personnage via OAuth (`account:characters`, flux PKCE avec redirection vers `localhost`) | Personne ne ressaisit un build. L'import est le premier moment de vérité du produit. |
| Vue | Écartée | Une seule stack front pour l'overlay et le planificateur. |

### 1.5 Backend de données et économie (privé, sans interface web)

Le backend existe uniquement pour fournir à l'application ce qu'elle ne peut pas calculer seule : les prix agrégés, les données de jeu de chaque patch et les statistiques communautaires d'Atlas Ledger. Il n'expose aucune page web.

| Couche | Choix | Justification |
|---|---|---|
| Ingestion | Workers **Rust (tokio)**, un « budget manager » central qui distribue les jetons de rate limit par client OAuth | Un seul composant connaît l'état des limites ; aucun worker ne frappe GGG directement. |
| Sources | Échange de devises officiel (endpoint OAuth dédié, agrégats horaires), échantillonnage du trade sur une liste de requêtes curées, flux de stash publics **si et quand disponibles pour PoE 2** | À cartographier en semaine 1 : la couverture PoE 2 des endpoints conditionne la roadmap Data. |
| Bus | **NATS JetStream** (ou Redpanda si le volume l'exige) | Découple l'ingestion du traitement, permet de rejouer un flux après un bug. |
| Stockage transactionnel | **PostgreSQL 17** | Comptes, builds, configuration, métadonnées par ligue. |
| Séries temporelles et analytique | **ClickHouse** | Des millions d'observations de prix par ligue ; agrégations par fenêtres temporelles en millisecondes. |
| Cache | **Redis / Valkey** | Prix courants, réponses chaudes, déduplication. |
| Stockage objet | S3-compatible (R2 / Hetzner Object Storage) | Snapshots bruts immuables par ligue : notre actif data de long terme. |
| API privée | **Rust (axum)**, REST + JSON, ETag, réponses cacheables au CDN, consommée uniquement par l'application (clé d'application + jeton par installation) | 95 % des lectures (prix, données de jeu) sont cacheables 1 à 5 minutes. |
| Données statiques | Paquets de données versionnés par patch, téléchargés par l'application au démarrage depuis le CDN (URL immuables, delta si possible) | Charge serveur quasi nulle le jour du lancement, et l'application reste utilisable hors ligne avec le dernier paquet. |

### 1.6 Data science

- **Python 3.13, Polars, DuckDB** pour l'exploration ; **Dagster** pour orchestrer les pipelines (assets versionnés par ligue).
- **LightGBM / CatBoost** pour la régression de prix d'objets rares (features : base, item level, mods et tiers, nombre d'affixes ouverts, corruption, quality, âge de la ligue) ; modèles de séries temporelles simples (lissage, Prophet en appui) pour la devise.
- **Détection de mispricing :** score d'anomalie (prix demandé / prix prédit, avec intervalle de confiance) et filtres anti « price fixing » (annonces leurres à prix absurde, très fréquentes sur l'échange PoE).
- **MLflow** pour le suivi des modèles, export ONNX si un modèle doit tourner dans l'overlay.
- Garde-fou éthique et légal : nos alertes de « bonnes affaires » ne déclenchent **jamais** d'achat ni de whisper automatique. Elles s'affichent ; l'humain agit.

### 1.7 Infrastructure et SRE

| Sujet | Choix | Justification |
|---|---|---|
| Bord | **Cloudflare** (CDN, WAF, rate limiting de notre propre API) | Absorbe le pic de lancement de ligue et les abus. |
| Calcul | Kubernetes managé (ou k3s sur Hetzner dédié) + autoscaling horizontal sur l'API | Coût maîtrisé hors lancement, élasticité pendant. |
| IaC | **OpenTofu** + Helm/Kustomize, GitOps avec Argo CD | Environnement de pré-lancement reproductible à l'identique. |
| Observabilité | **OpenTelemetry**, Prometheus, Grafana, Loki, Sentry (crashs Rust et WebView de l'application, avec consentement) | Un tableau de bord dédié aux « budgets GGG » : jetons consommés, 429 reçus, `Retry-After` actifs. |
| Tests de charge | **k6**, scénarios rejoués à 10x le trafic de la ligue précédente | Le test de charge est un livrable obligatoire avant chaque lancement. |
| Dégradation contrôlée | Feature flags, mode lecture seule, prix « gelés » avec horodatage visible | Mieux vaut un prix vieux de 10 minutes, affiché comme tel, qu'une erreur 500. |

### 1.8 Synthèse par rôle

- **Architecte :** propriétaire de `poe2-core` (frontières, versionnement), du budget manager de rate limits, du contrat entre l'application et le backend. Il arbitre ce qui est calculé en local et côté serveur (par défaut : en local).
- **Devs front / desktop :** application Tauri (overlay et planificateur), React/TS, intégration Win32, design system, rendu de l'arbre en WebGL, installeur et mises à jour.
- **Devs back :** Rust (axum, tokio), ingestion, parseur d'objets, pipeline des `.datc64`, Postgres et ClickHouse.
- **UX/UI :** Figma avec tokens synchronisés vers Tailwind, bibliothèque de composants « tooltip d'objet » fidèle aux conventions du jeu (couleurs de rareté, mods implicites et explicites, affixes désacralisés ou corrompus).
- **Data :** Python/Polars/Dagster sur ClickHouse, modèles LightGBM, rapports de santé du marché.
- **Theorycrafter :** fixtures YAML, éditeur de cas de test intégré à la version interne de l’application, lecture des traces de calcul. Aucun Rust requis.
- **DevOps/SRE :** Cloudflare, Kubernetes, OpenTofu, OpenTelemetry, k6, runbooks de lancement de ligue, chaîne de build et de signature Windows (runners CI Windows, certificat de signature de code).

---

## 2. Trois produits pour PoE 2

Les critères de sélection : un vrai manque dans l'écosystème actuel (PoE2DB, poe.ninja, Path of Building PoE 2, Exiled Exchange 2, Craft of Exile, Exile UI…), une conformité totale aux règles de GGG, et une réutilisation maximale de `poe2-core`.

### Produit 1 : « Upgrade Copilot », l'overlay qui répond à « est-ce mieux pour MON build ? »

**Le manque.** Les overlays actuels donnent un prix. Path of Building donne un delta de DPS, mais il faut basculer dans une autre application et y coller l'objet à la main. Personne ne répond instantanément, en jeu, à la seule question que le joueur se pose : *cet objet améliore-t-il mon personnage, et de combien par exalt dépensé ?*

**Fonctionnalités.**
- Ctrl+C sur un objet (sol, coffre, trade) : affichage du **delta DPS, EHP et des résistances** par rapport au personnage importé via OAuth, en moins de 100 ms, avec le prix estimé à côté.
- Prise en compte des spécificités PoE 2 : sets d'armes (swap), coût en Spirit, plafonds de résistance, effets sur le buildup d'ailments.
- **Recherche d'upgrades sous budget :** le moteur calcule la valeur marginale de chaque stat pour le build (dérivées numériques de DPS/EHP par stat), puis génère une recherche de trade pondérée (filtres de type « weighted sum » du site de trade) exécutée **depuis le client**.
- Affichage « DPS gagné par exalt » sur les résultats.

**Pourquoi nous.** C'est la vitrine directe de `poe2-core`, le cœur commun de tous nos produits.
**Conformité.** Presse-papier + OAuth + recherche lancée par une action explicite de l'utilisateur.
**Risque principal.** Exactitude du moteur sur la diversité des builds. On mitige en publiant la couverture : chaque archétype est marqué « vérifié », « approximatif » ou « non supporté ».

### Produit 2 : « Forge », un planificateur de craft à espérance de coût

**Le manque.** Les simulateurs de craft calculent des probabilités. Aucun ne répond à la question économique : *quelle stratégie de craft a le meilleur coût espéré pour atteindre CET objectif, aux prix d'AUJOURD'HUI, et vaut-il mieux l'acheter ?*

**Fonctionnalités.**
- Modélisation du pool de mods à partir des poids d'apparition extraits des données du jeu (par base, item level, tags), y compris les mécaniques propres à PoE 2 : Essences, Omens, mods désacralisés, corruption via Vaal Orb, et toute nouvelle mécanique de craft à chaque ligue.
- Simulation de Monte-Carlo (dans `poe2-core`, côté client) des stratégies : chaînes de devises, points d'arrêt, recyclage des échecs.
- **Espérance de coût en exalts** via les prix live de l'échange de devises, distribution des coûts (P50, P90 : « 1 fois sur 10, ce craft vous coûtera plus de X divines »).
- Comparaison avec le prix de marché d'un objet équivalent (modèle de pricing de l'équipe Data) : **« craftez » ou « achetez »**.
- Mode overlay : Ctrl+Alt+C sur la base en cours de craft, le plan affiche la prochaine étape optimale.

**Pourquoi nous.** Ça combine le data-mining (poids), le moteur de simulation et l'économie : une barrière à l'entrée élevée pour les projets bénévoles.
**Risque principal.** Certains poids ou mécaniques ne sont pas dans les données du client, ou changent en cours de ligue. On mitige en étiquetant chaque probabilité par sa source (extraite, mesurée empiriquement, supposée) et en ajoutant une boucle de validation communautaire (résultats de crafts réels déclarés volontairement).

### Produit 3 : « Atlas Ledger », le traqueur de rentabilité des stratégies de farm

**Le manque.** La question « quelle stratégie d'Atlas rapporte le plus par heure en ce moment ? » se règle sur des vidéos et des ressentis. Les données agrégées objectives n'existent pas.

**Fonctionnalités.**
- **Suivi de session automatique et conforme :** tail de `Client.txt` pour les entrées et sorties de zone (cartes, hideout, temps par map), snapshot de la réserve via OAuth (`account:stashes`) en début et en fin de session, diff valorisé aux prix du moment.
- Étiquetage de la stratégie (tablettes de précurseur, mécaniques ciblées, type de Waystone) par l'utilisateur, avec suggestions automatiques.
- Tableau de bord personnel dans l'application : profit horaire, variance, coût des investissements (Waystones, tablettes) ; historique stocké en local.
- **Couche communautaire (opt-in, anonymisée), consultable dans l'application :** profit/heure médian par stratégie et par jour de ligue. C'est un actif de données unique qui se renforce avec le nombre d'utilisateurs.
- **Prévision de marché :** courbes de prix attendues des principales ressources selon l'âge de la ligue (les cycles de lancement se ressemblent d'une ligue à l'autre), pour décider quand vendre.

**Pourquoi nous.** Ça fait travailler l'ingénierie data (modélisation, prévision) et produit des données propriétaires que les concurrents ne peuvent pas copier.
**Risque principal.** Le biais de sélection des utilisateurs qui partagent leurs données. On mitige avec des intervalles de confiance affichés, des seuils minimaux d'échantillon et la pondération par niveau de personnage et par avancement d'Atlas.

### Ordre recommandé

1. **Upgrade Copilot** d'abord : il force la construction de `poe2-core`, du parseur et de l'import OAuth, dont les deux autres produits dépendent.
2. **Atlas Ledger** ensuite : le pipeline data tourne dès le sprint 1 pour accumuler l'historique, le produit arrive quand les données sont là.
3. **Forge** en dernier : c'est le plus exigeant en exactitude, il bénéficie de la maturité du moteur et du pricing.

---

## 3. Méthodologie Theorycrafter ↔ Développeurs

**Principe directeur : le Theorycrafter est propriétaire de la vérité, les développeurs sont propriétaires de l'implémentation, et les tests sont le contrat entre les deux.** Le Theorycrafter ne doit jamais avoir à lire du Rust pour savoir si le logiciel est juste.

### 3.1 Les quatre artefacts du Theorycrafter

1. **Les fiches mécaniques** (Markdown, une par mécanique, versionnées dans le dépôt). Exemple : « Electrocute : buildup, seuil, durée ». Chaque fiche contient :
   - la formule et l'**ordre des opérations** (base → added → conversions → increased/reduced additifs → more/less multiplicatifs → plafonds → arrondis) ;
   - les **règles d'arrondi** à chaque étape (tronqué, arrondi, à quelle étape) ;
   - les interactions connues et les cas limites ;
   - un **niveau de confiance** : `confirmé` (patch notes, déclaration de dev GGG, tooltip du jeu), `mesuré` (test empirique reproductible), `supposé` ;
   - les **sources** : lien vers la patch note, la vidéo ou la capture du test.
2. **Les cas de test « golden »** (YAML, édités via un écran dédié de la version interne de l’application) :
   ```yaml
   id: electrocute-buildup-001
   patch: "0.x.y"
   confiance: mesuré
   personnage: { niveau: 80, classe: ..., import_code: "..." }
   conditions: { cible: "dummy niveau 80", buffs: [] }
   attendu:
     tooltip_dps: 48213      # valeur affichée en jeu
     tolerance_pct: 0.5
   preuve: "lien vers la capture"
   ```
   Chaque cas devient automatiquement un test exécuté en CI. Un cas qui échoue bloque la fusion.
3. **Les tables de correspondance de stats** (CSV/YAML éditables) qui relient chaque `stat_id` du jeu au modificateur interne du moteur. Une nouvelle stat introduite par un patch se règle dans une table, sans attendre un développeur dans la majorité des cas.
4. **Le registre des inconnues** : la liste des mécaniques non élucidées, priorisées par le nombre de builds concernés. C'est la feuille de route du Theorycrafter pour les tests en jeu.

### 3.2 Les outils que les devs construisent pour le Theorycrafter

- **La trace de calcul (« breakdown ») :** pour chaque stat finale, l'arbre complet des contributions avec leur source (nœud passif, mod d'objet, gemme). Le Theorycrafter y trouve l'erreur sans lire le code.
- **Le rapport de diff en CI :** à chaque pull request, un tableau « cas golden : avant / après / attendu » commenté automatiquement sur la PR. Le Theorycrafter relit les chiffres, pas le code.
- **Le diff de données de patch :** au jour du patch, l'extraction des `.datc64` produit la liste des mods, gemmes et passifs modifiés. Chaque entrée devient un ticket pré-rempli, tagué par mécanique, assigné au Theorycrafter pour qualification.
- **Le mode « explication » dans les produits :** la même trace, exposée aux utilisateurs avancés. La communauté devient un relais de détection d'erreurs.

### 3.3 Le cycle de vie d'une mécanique

1. **Spécification** : le Theorycrafter rédige la fiche et au moins trois cas golden (cas nominal, cas limite, interaction).
2. **Revue croisée** de 30 minutes entre Theorycrafter et développeur : le développeur reformule la formule dans ses mots. Les ambiguïtés se découvrent ici, pas en production.
3. **Implémentation** pilotée par les tests : la PR n'est fusionnable que si les cas golden passent.
4. **Validation** : le Theorycrafter vérifie la trace sur un ou deux builds réels importés.
5. **Publication** avec le niveau de confiance affiché dans le produit.
6. **Surveillance** : les signalements utilisateurs (« écart avec le tooltip en jeu ») arrivent dans une file de tri ; un écart reproductible devient un nouveau cas golden **avant** toute correction.

### 3.4 Protocole de mesure en jeu

- Conditions standardisées : personnage de test dédié, cible standardisée, aucun buff temporaire, mêmes réglages.
- Référence prioritaire : les valeurs affichées par le jeu (tooltips de compétence, panneau du personnage), qui sont déterministes, plutôt que des mesures de dégâts au chronomètre.
- Tolérance de comparaison explicite par type de stat (0 pour les entiers affichés, ±0,5 % pour les DPS dérivés).
- Toute mesure est filmée ou capturée et référencée dans le cas golden.

### 3.5 Le rituel du jour de patch (pendant 72 heures)

| Moment | Action | Responsable |
|---|---|---|
| Publication des patch notes | Triage : chaque ligne est classée par impact sur le moteur (aucun, table, formule, nouvelle mécanique) | Theorycrafter + Architecte |
| Patch déployé | Extraction des données, diff, génération des tickets | Data + Back |
| H+2 | Les changements de tables sont appliqués par le Theorycrafter, la CI tourne | Theorycrafter |
| H+6 | Publication des données à jour ; les mécaniques non vérifiées sont signalées comme telles dans l'UI | Tous |
| H+24 à H+72 | Les nouvelles mécaniques sont mesurées en jeu et leurs cas golden ajoutés | Theorycrafter |

---

## 4. Premier sprint : un mois, quatre semaines

**Objectif du sprint :** livrer une **bêta fermée de l'overlay** (Upgrade Copilot V0) capable de lire un objet, d'en estimer le prix et de calculer un delta de DPS sur **un archétype de build restreint et entièrement vérifié**, avec la plateforme data déjà en train d'accumuler l'historique.

**Hors périmètre volontaire :** la fenêtre planificateur complète (arbre passif interactif), Forge, Atlas Ledger côté produit.

**Action administrative immédiate (Max, jour 1) :** demander l'enregistrement de notre application OAuth auprès de GGG en suivant la procédure de la documentation développeur, et vérifier la politique d'usage de la marque (nom du studio et des produits sans confusion possible avec « Path of Exile » ou GGG). Le délai de validation est indépendant de nous ; c'est le chemin critique du sprint.

### Architecte logiciel
- **S1 :** cartographie des endpoints GGG réellement disponibles pour PoE 2 (trade2, échange de devises, personnages, réserves, stash publics) avec scopes, limites observées et risques. Document de décision d'architecture n° 1 : frontières de `poe2-core`, partage client/serveur.
- **S2 :** conception et prototype du **budget manager de rate limits** (lecture des en-têtes, back-off, `Retry-After`, priorités par file).
- **S3 :** spécification du format de données de jeu versionné par patch et du contrat d'API publique (OpenAPI).
- **S4 :** revue de sécurité de l'overlay (aucune donnée sensible persistée, jetons OAuth dans le trousseau système), validation de la bêta.
- **Definition of Done :** les décisions d'architecture sont écrites et acceptées, le budget manager encaisse un test de saturation sans aucun 429 non géré.

### Développeurs back-end
- **S1 :** pipeline d'extraction des `.datc64` → JSON versionné (Mods, Stats, BaseItemTypes, gemmes, passifs), exécuté en CI sur un client à jour.
- **S2 :** **parseur d'objets** Ctrl+C et Ctrl+C avancé dans `poe2-core`, sur au moins 500 objets réels collectés (toutes raretés, uniques, corrompus, désacralisés).
- **S3 :** squelette du moteur de calcul : graphe de modificateurs, trace de calcul, premier archétype (par exemple une compétence d'attaque à l'arc ou un sort élémentaire, choisi avec le Theorycrafter).
- **S4 :** service d'ingestion des prix de devises + API `GET /prices/{league}`, cache CDN.
- **Definition of Done :** parseur ≥ 99 % de réussite sur le corpus, 100 % des cas golden de l'archétype au vert.

### Développeurs front-end / desktop
- **S1 :** coquille Tauri 2 : fenêtre transparente, raccourcis globaux, lecture du presse-papier, détection de la fenêtre du jeu via Win32, tail de `Client.txt` (clients Steam et standalone), icône dans la zone de notification.
- **S2 :** intégration native de `poe2-core` ; affichage de l'objet parsé avec le composant tooltip du design system.
- **S3 :** recherche de prix via le trade **depuis le client** avec rate limiter local et file visible ; import de personnage via OAuth (dès que l'application est validée, sinon via un code Path of Building en repli).
- **S4 :** panneau delta DPS/EHP, installeur signé et updater, kill switch par fonctionnalité, télémétrie d'erreurs (Sentry) avec consentement.
- **Definition of Done :** du Ctrl+C à l'affichage complet en moins de 150 ms hors trade, aucune perte de focus du jeu, installation sans alerte SmartScreen sur un Windows 11 vierge.

### UX/UI Designer
- **S1 :** audit des overlays existants et 8 à 10 entretiens de joueurs (du casual au joueur SSF ou au trader), cartographie des moments d'usage (en map, en hideout, au trade).
- **S2 :** design system « dark » : tokens, typographie lisible sur fond de jeu, couleurs de rareté et de mods fidèles aux conventions du jeu, contrastes vérifiés sur des captures réelles de zones claires et sombres.
- **S3 :** maquettes de l'overlay : tooltip enrichi, panneau de delta, états de chargement et de rate limit, états « non supporté » et « approximatif ».
- **S4 :** tests d'utilisabilité sur la bêta (5 joueurs minimum), règle des 2 secondes : l'information clé se lit en moins de 2 secondes en plein combat.
- **Definition of Done :** composants livrés dans Figma et implémentés, recommandations issues des tests priorisées.

### Data Scientist / Ingénieur Data
- **S1 :** collecteur d'échantillons de trade sur une liste curée de requêtes (devises, fragments, bases recherchées), en passant par le budget manager. Stockage brut dans l'object storage, aucun retraitement destructif.
- **S2 :** schéma ClickHouse, pipelines Dagster, nettoyage (détection des annonces leurres, déduplication, normalisation en exalts).
- **S3 :** indice de prix des devises robuste (médiane pondérée, exclusion des valeurs aberrantes) exposé par l'API back.
- **S4 :** premier modèle de pricing de rares sur une catégorie (par exemple un emplacement d'équipement), avec métrique de base (MAPE) et analyse des erreurs.
- **Definition of Done :** l'historique s'accumule sans interruption, le tableau de bord de qualité des données est en place.

### Theorycrafter / Analyste de jeu
- **S1 :** choix de l'archétype du MVP avec l'équipe ; fiches mécaniques de base : ordre des opérations des dégâts, résistances et pénétration, armure, évasion, Energy Shield, calcul de l'EHP.
- **S2 :** **40 cas golden minimum** pour l'archétype (personnage de test dédié, preuves capturées), registre des inconnues initial.
- **S3 :** tables de correspondance de stats pour les mods de l'archétype ; validation des traces de calcul avec les devs back.
- **S4 :** validation de la bêta sur 5 builds réels de la communauté ; rédaction du **runbook du jour de patch** côté théorie.
- **Definition of Done :** 100 % des cas golden validés par preuve, niveau de confiance attribué à chaque mécanique de l'archétype.

### Ingénieur DevOps / SRE
- **S1 :** dépôts, CI (Rust, Tauri sur runners Windows, Python), achat du certificat de signature de code, environnements dev, staging et production en OpenTofu.
- **S2 :** cluster, Postgres, ClickHouse, Redis, NATS déployés ; Cloudflare devant tout ; secrets gérés (aucun jeton GGG dans le code).
- **S3 :** observabilité complète, dont le **tableau de bord des budgets GGG** et des alertes sur les 429 ; pipeline de signature et de publication de l'overlay.
- **S4 :** premier test de charge k6 (hypothèse : 20 000 utilisateurs actifs, 10x les requêtes de prix en une heure) ; brouillon du **runbook de lancement de ligue** (gel des déploiements, montée en charge préventive, mode dégradé, astreinte).
- **Definition of Done :** reconstruction de staging en une commande, test de charge passé ou goulots documentés avec un plan.

### Jalons et rituels du sprint

| Semaine | Jalon de fin de semaine |
|---|---|
| S1 | Cartographie des API, données de jeu extraites, coquille d'overlay qui lit le presse-papier |
| S2 | Parseur validé sur le corpus, infra de production en place, design system V1 |
| S3 | Premier delta DPS juste sur l'archétype, prix des devises en API |
| S4 | Bêta fermée à 50 joueurs, test de charge, runbooks rédigés |

- Démo chaque vendredi sur un **build réel en jeu**, pas sur des maquettes.
- Revue « exactitude » hebdomadaire de 45 minutes : Theorycrafter, devs back et Data, autour du rapport de diff des cas golden.
- Un seul tableau de priorités, arbitré par Max ; l'architecte arbitre la technique.

---

## 5. Risques majeurs et parades

| Risque | Impact | Parade |
|---|---|---|
| Application OAuth non validée à temps | Pas d'import de personnage | Repli sur l'import de code Path of Building ; demande faite dès le jour 1 |
| Changement des endpoints ou des limites de GGG | Ingestion cassée | Budget manager piloté par les en-têtes, alertes, kill switch |
| Mise à jour des règles d'outils tiers | Fonctionnalité interdite | Veille sur les annonces GGG, aucune fonctionnalité en zone grise, kill switch distant |
| Écarts de calcul publics | Perte de confiance | Niveaux de confiance affichés, trace exposée, cas golden obligatoires |
| Pic du lancement de ligue | Indisponibilité | CDN, données statiques immuables, mode dégradé, test de charge à 10x |
| Format du presse-papier modifié par un patch | Overlay inutilisable | Corpus de régression, mise à jour du parseur en moins de 6 heures, kill switch |
| Faux positifs antivirus / SmartScreen | Installations abandonnées | Signature de code, réputation construite dès la bêta, soumission aux éditeurs d'antivirus |

---

## 6. Ce que j'attends de toi, Max, cette semaine

1. Valider ou amender la stack (en particulier le choix Rust + Tauri, structurant pour le recrutement), et lancer l'achat d'un certificat de signature de code Windows (délai de vérification d'identité de plusieurs jours).
2. Valider l'ordre des produits (Upgrade Copilot → Atlas Ledger → Forge).
3. Lancer la demande d'enregistrement OAuth auprès de GGG et la vérification de la marque.
4. Fixer la date du prochain lancement de ligue comme jalon de référence : le sprint 2 et la suite s'organisent à rebours depuis cette date.
