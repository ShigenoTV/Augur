# Cartographie des API GGG pour Path of Exile 2

**Statut :** relevé du 31 octobre 2026 sur la documentation officielle (`pathofexile.com/developer/docs`, pages *Overview*, *Authorization* et *Reference*). GGG modifie endpoints, scopes et limites sans long préavis : **toute valeur ici se revérifie avant implémentation**, et le code ne doit jamais coder une limite en dur (voir [ADR-0002](adr/0002-budget-manager-rate-limits.md)).

## 1. Ce qui conditionne la roadmap

| # | Constat | Conséquence pour Augur |
|---|---|---|
| 1 | **GGG n'accepte pas de nouvelles applications OAuth pour l'instant** (« We are currently unable to process new applications »). | Le chemin critique du sprint (import de personnage OAuth) est bloqué tant que GGG n'ouvre pas les inscriptions. Le repli prévu au plan (import de code Path of Building PoE 2) devient le **chemin principal** du sprint 1. |
| 2 | Un exécutable **doit** utiliser un client OAuth **public** : PKCE obligatoire, redirection locale (`http://127.0.0.1:<port>/…`), jetons d'accès de 10 h, de rafraîchissement de 7 jours, **aucun scope `service:*`**, et rate limits **partagées avec tous les autres clients publics**. L'utilisateur voit un avertissement « authenticité non vérifiable ». | L'appli ne peut accéder qu'aux scopes `account:*`. Tout ce qui relève de `service:*` (ligues, ladders, échange de devises authentifié) se fait côté backend, avec un client **confidentiel** distinct. Le joueur devra se reconnecter au moins une fois par semaine. |
| 3 | **Réserves de compte (`/stash`) : PoE 1 uniquement.** `poe2` n'est pas un realm accepté. | Atlas Ledger ne peut pas faire de snapshot de réserve sur PoE 2 aujourd'hui. Le suivi de session doit reposer sur `Client.txt` et une saisie ou un Ctrl+C des butins ; à réévaluer à chaque annonce de GGG. |
| 4 | **Public Stash API : PoE 1 uniquement.** | Pas de flux de stash publics pour PoE 2 : les prix d'objets viennent de l'échantillonnage du site de trade et de l'échange de devises. |
| 5 | **Échange de devises : public et disponible pour `poe2`**, agrégats horaires historiques. | Source de prix de devises fiable et légère pour le backend dès le sprint 1. |
| 6 | **Personnages : disponibles pour `poe2`** (`skills`, `passives.specialisations`, `passives.quest_stats` propres à PoE 2). | L'import OAuth fonctionnera dès qu'une application sera validée. |
| 7 | Le site de trade (`/api/trade2/…`) **n'est pas une API documentée**. | Il est utilisé par tous les overlays du marché, mais son format peut changer sans préavis : il passe derrière le kill switch et le budget manager, et ses réponses sont validées par schéma. |

## 2. Endpoints officiels

Base : `https://api.pathofexile.com`. User-Agent obligatoire : `OAuth {clientId}/{version} (contact: {contact})`.

| Domaine | Méthode et chemin | Scope | PoE 2 | Côté Augur |
|---|---|---|---|---|
| Profil | `GET /profile` | `account:profile` | oui (sans realm) | Appli (client public) |
| Filtres d'objets | `GET/POST /item-filter[/<id>]` | `account:item_filter` | oui (`realm: poe2`) | Hors périmètre |
| Ligues | `GET /league`, `GET /league/<league>` | `service:leagues` | oui | **Backend** (client confidentiel), redistribué à l'appli |
| Ladder | `GET /league/<league>/ladder` | `service:leagues:ladder` | oui (top 1 000, sans `sort`) | Backend, plus tard (stats de meta) |
| Event ladder | `GET /league/<league>/event-ladder` | `service:leagues:ladder` | **non** | — |
| PvP | `GET /pvp-match…` | `service:pvp_matches*` | **non** | — |
| Ligues du compte | `GET /account/leagues[/<realm>]` | `account:leagues` | **non** | — |
| Personnages | `GET /character[/<realm>]`, `GET /character[/<realm>]/<name>` | `account:characters` | oui (`/character/poe2/…`) | Appli : import de personnage |
| Réserves | `GET /stash[/<realm>]/<league>[/…]` | `account:stashes` | **non** | Bloque le snapshot d'Atlas Ledger |
| Comptes de ligue | `GET /league-account[/<realm>]/<league>` | `account:league_accounts` | **non** | — |
| Réserves de guilde | `GET /guild[/<realm>]/stash/<league>…` | `account:guild:stashes` (sur demande) | **non** | — |
| Stash publics | `GET /public-stash-tabs[/<realm>]` | `service:psapi` | **non** | — |
| Échange de devises | `GET /currency-exchange[/<realm>][/<id>]` (la doc l'affiche aussi sous un hôte CDN public) | `service:cxapi`, et accès public | oui (`/poe2`) | **Backend** : ingestion horaire |

### Échange de devises : détails utiles à l'équipe Data

- Réponse : `next_change_id` (timestamp Unix tronqué à l'heure) et `markets[]` avec `league`, `market_id`, `market_pair`, `volume_traded`, `lowest_stock`, `highest_stock`, `lowest_ratio`, `highest_ratio`.
- Sans `id`, on reçoit la première heure d'historique ; on avance avec `next_change_id`.
- Fin du flux : `next_change_id` égal à l'`id` envoyé ; attendre la prochaine heure pleine.
- Données **purement historiques** : l'heure en cours n'est jamais disponible, et l'historique ancien peut être purgé. On archive donc chaque réponse brute dans le stockage objet.
- **À vérifier sur la doc au moment de coder :** l'hôte exact (la page de référence affiche un hôte CDN public en plus de l'API OAuth) et la politique de rate limit associée.

## 3. OAuth

| | Client public (appli Windows) | Client confidentiel (backend) |
|---|---|---|
| Flux | Authorization Code + PKCE (S256, vérificateur ≥ 32 octets d'entropie) | Authorization Code, Client Credentials, Refresh |
| Redirection | Locale, ex. `http://127.0.0.1:8080/callback` | HTTPS sur un domaine que nous contrôlons |
| Scopes | `account:*` uniquement | `account:*` et `service:*` |
| Jeton d'accès | 10 h | 28 jours (Client Credentials : sans expiration fixe, révocable) |
| Jeton de rafraîchissement | 7 jours, non prolongeable | 90 jours |
| Rate limits | Partagées entre tous les clients publics | Généralement propres au client |

Points d'implémentation :

- Code d'autorisation valable **30 secondes** : l'échange doit suivre immédiatement la redirection.
- Un jeton de rafraîchissement utilisé expire aussitôt, et le nouveau **hérite de l'échéance d'origine** : au-delà de 7 jours, l'utilisateur repasse par le consentement.
- Jetons stockés dans le trousseau Windows (Credential Manager), jamais en clair dans `%APPDATA%`.
- `oauth:revoke` permet de révoquer à la déconnexion.
- **Jamais de `POESESSID`**, sous aucune forme.

## 4. Rate limits

En-têtes renvoyés :

| En-tête | Contenu |
|---|---|
| `X-Rate-Limit-Policy` | Nom de la politique ; plusieurs endpoints peuvent la partager |
| `X-Rate-Limit-Rules` | Règles applicables, séparées par des virgules (`ip`, `account`, `client`) |
| `X-Rate-Limit-{règle}` | Fenêtres `hits_max:période_s:restriction_s`, séparées par des virgules |
| `X-Rate-Limit-{règle}-State` | Fenêtres `hits:période_s:restriction_active_s` |
| `Retry-After` | Secondes à attendre, sur un 429 |

Règles de conduite tirées de la doc :

- Les limites sont **dynamiques** et peuvent changer à tout moment.
- Dépasser souvent les limites **fait révoquer l'accès de l'application**.
- Les requêtes invalides (tout 4xx, en particulier 401, 403, 429) sont elles aussi limitées : on ne réessaie jamais aveuglément un 4xx.

Valeurs observées publiquement sur la recherche de trade (indicatives, **ne pas coder en dur**) : règle `ip` à `8:10:60,15:60:120,60:300:1800`. Le test de saturation du budget manager s'en sert comme scénario.

## 5. Site de trade (`/api/trade2`)

Non documenté officiellement ; utilisé par le site de trade et tous les overlays de prix.

- `POST /api/trade2/search/poe2/{league}` renvoie un identifiant de requête et une liste d'identifiants de résultats.
- `GET /api/trade2/fetch/{ids}?query={id}` renvoie le détail, par lots de 10.
- Politiques de rate limit distinctes pour la recherche et le fetch, règle `ip` principalement : **le budget est celui du joueur**, partagé avec son navigateur.

Décisions :

1. Les recherches « utilisateur » partent du poste du joueur (plan fondateur, §0.2).
2. Chaque recherche est déclenchée par une action explicite (Ctrl+C, bouton) : une action, une recherche.
3. Le backend n'utilise le trade que pour son échantillonnage curé, à faible débit, via son propre budget manager.
4. Les réponses sont validées par schéma ; un échec de validation coupe la fonctionnalité via le kill switch au lieu d'afficher un prix faux.

## 6. Sources locales tolérées

| Source | Usage | Règle |
|---|---|---|
| Presse-papier (Ctrl+C, Ctrl+Alt+C) | Parseur d'objets | Lecture seule, sur action du joueur |
| `Client.txt` | Zones, niveau, whispers | Lecture seule ; l'utilisateur est informé de ce qu'on en fait |
| Capture d'écran | Non utilisé au sprint 1 | — |
| Mémoire du jeu, injection, fichiers du jeu | **Interdit** | La doc de GGG annonce la fermeture immédiate du compte pour les exécutables qui interagissent avec le jeu ou ses fichiers |

Autres règles de la doc officielle à respecter : macros déclenchées manuellement, une action affectant le jeu par invocation, et **mention visible** qu'Augur n'est ni affilié à GGG ni approuvé par eux.

## 7. Risques ouverts

| Risque | Parade |
|---|---|
| Inscriptions OAuth fermées durablement | Import PoB en chemin principal ; demande déposée dès réouverture |
| Rate limits partagées entre clients publics | Budget manager prudent, file visible, aucune requête de fond via OAuth |
| Format de `/api/trade2` modifié | Validation par schéma, kill switch, corpus de réponses enregistrées |
| Ouverture des réserves PoE 2 plus tard | Atlas Ledger conçu pour brancher la source quand elle existera |
