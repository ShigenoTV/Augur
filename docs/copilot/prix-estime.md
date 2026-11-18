# Prix estimé dans l'overlay (PoE 2)

Demande de Max du 18/11/2026 (check-list d'idées du 11/11, « prix estimé »).

## Ce que l'overlay affiche

| Objet copié | Prix | Comment |
| --- | --- | --- |
| Monnaies, essences, Omens, runes, Soul Cores, catalyseurs, fragments, gemmes brutes, gemmes de lignée, Idols… | automatique | cote du marché (poe.ninja), en Exalted ou en Divine dès 1 Divine ; pour une pile, prix de la pile et prix à l'unité |
| Uniques et rares identifiés | sur raccourci (**Alt+D**, modifiable dans Réglages) | cote du site de trade, lancée depuis le PC du joueur |
| Magiques, normaux, rares non identifiés | aucun | Alt+U ouvre toujours la recherche d'upgrades |

Le prix ne dépend pas du personnage importé : une monnaie n'affiche que son prix, et un unique
ou un rare reste cotable sans personnage. Aucune source n'est nommée dans l'appli (règle de Max).

## Cote du marché (poe.ninja)

- `crates/poe2-core/src/cote.rs` choisit les pages « échange » à lire d'après le nom de l'objet,
  puis cherche l'objet par son nom (`items[].name`, et « Nom (Level N) » pour les gemmes brutes et
  les Flux). poe.ninja PoE 2 n'a pas de page d'uniques (404, vérifié le 18/11/2026).
- `apps/desktop/src-tauri/src/poe2_price.rs` télécharge une page à la demande et la garde
  30 minutes : relire des objets de la même famille ne refait aucune requête.

## Cote trade (uniques et rares)

Décision de Max du 18/11/2026 : « faire comme Awakened PoE Trade / Exiled Exchange 2 ».

- Un appui sur le raccourci = une cote = deux requêtes : `POST /api/trade2/search/poe2/<ligue>`
  puis `GET /api/trade2/fetch/<10 ids>?query=<id>` (les 10 annonces en ligne les moins chères).
- Les requêtes passent par le budget manager (`crates/augur-ratelimit`), qui suit les en-têtes
  `X-Rate-Limit-*` de GGG. Si le créneau n'arrive pas en 8 secondes, ou au premier 429, Augur
  s'arrête et affiche l'attente demandée par GGG. Aucune requête n'est faite sans appui du joueur.
- Recherche : l'unique par son nom et sa base ; le rare par sa base et ses mods explicites à au
  moins 80 % de leur valeur (même règle que la recherche de l'objet cible). Les mods que le trade
  ne sait pas chercher sont comptés et signalés sous l'overlay.
- Fourchette : bas, médiane, haut des annonces lues, la moins chère écartée dès 5 annonces (prix
  d'appel). Les devises sont converties avec la cote du marché.
- Sous l'overlay, « N annonces comparables en ligne : voir sur le trade » ouvre les résultats.

La règle des 4 requêtes / 30 min (`docs/data/pipeline-prix.md`) reste celle de l'échantillonnage
depuis le backend ; elle ne s'applique pas à ces cotes faites à la demande du joueur.

## À vérifier en jeu

Le site de trade refuse les requêtes venant du cloud : la cote trade n'a pas pu être essayée avant
le premier test de Max (Alt+D sur un unique, puis sur un rare).
