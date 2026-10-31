# ADR-0002 : budget manager des rate limits GGG

- **Statut :** proposé, prototype livré (`crates/augur-ratelimit`)
- **Date :** 31 octobre 2026
- **Auteur :** architecte

## Contexte

Les rate limits de GGG sont la ressource la plus rare du studio (plan fondateur, §0.1). Elles sont dynamiques, annoncées par en-têtes, et des dépassements répétés font révoquer l'accès de l'application. Côté appli, le budget est celui du joueur et il est **partagé** avec son navigateur (règle `ip`) et, pour l'OAuth public, avec tous les autres clients publics.

## Décision

Un composant unique, `augur-ratelimit`, utilisé par l'appli et par les workers d'ingestion. **Aucun code n'appelle GGG sans lui.**

```rust
let permit = budget.acquire("trade2-search", Priority::Interactive).await;
let response = http.send(request).await;
let snapshot = parse_policy(|name| response.header(name))?;
permit.complete(snapshot.as_ref(), response.status());
```

### Comportement

1. **Piloté par les en-têtes.** Les fenêtres viennent de `X-Rate-Limit-{règle}` ; aucune limite n'est codée en dur.
2. **Amorçage prudent.** Tant qu'une politique est inconnue, une seule requête à la fois.
3. **Rattachement des endpoints.** L'appelant nomme un endpoint ; dès que `X-Rate-Limit-Policy` est connu, l'endpoint est rattaché à la politique, partagée entre endpoints.
4. **Marge de sécurité** de 10 % par fenêtre (arrondie au supérieur, minimum 1 requête utilisable).
5. **Réconciliation.** Si `X-Rate-Limit-{règle}-State` compte plus de requêtes que nous (navigateur, autre outil), on s'aligne sur le serveur.
6. **Restriction.** Sur une restriction active ou un `Retry-After`, la politique est bloquée pendant toute la durée ; un 429 sans indication bloque pendant la plus longue restriction connue, à défaut 60 s.
7. **Priorités.** Les requêtes interactives (action explicite du joueur) passent devant ; les requêtes de fond ne peuvent pas entamer les 25 % de capacité réservés au joueur.
8. **Jeton abandonné** (erreur réseau, annulation) : la requête reste comptée, car on ne sait pas si le serveur l'a reçue.
9. **Visible.** `BudgetManager::snapshot()` alimente `RateLimitStatus` pour l'UI (file d'attente visible) et le tableau de bord SRE « budgets GGG ».

### Architecture interne

- `headers` : lecture des en-têtes, indépendante du client HTTP.
- `limiter` : machine à états **pure**, l'horloge est injectée ; testable en temps simulé.
- `BudgetManager` (feature `async`, tokio) : partage entre tâches, files de priorité.

## Validation

`crates/augur-ratelimit/tests/saturation.rs` simule un serveur GGG (fenêtres `8:10:60,15:60:120,60:300:1800`, inconnues du limiteur au départ) et une demande d'une recherche toutes les 100 ms pendant une heure :

- **0 réponse 429**, avec plus de 600 requêtes servies (le plafond théorique est ~650 avec la marge) ;
- **0 réponse 429** aussi quand le navigateur du joueur consomme la même règle en parallèle.

C'est le critère de la Definition of Done de l'architecte au sprint 1.

## Suite prévue

- Persistance de l'état entre deux lancements de l'appli (un redémarrage ne doit pas remettre les compteurs à zéro alors que le serveur se souvient).
- Côté backend : un budget manager partagé entre plusieurs workers (état dans Redis) si l'ingestion passe à plusieurs processus.
- Métriques OpenTelemetry : jetons consommés, attentes, 429 reçus.
