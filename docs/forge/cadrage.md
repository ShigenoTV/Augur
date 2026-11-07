# Forge : cadrage

**Statut :** démarrage du module, 7 novembre 2026 · **Code :** [`crates/poe2-core/src/forge`](../../crates/poe2-core/src/forge)

Forge est le deuxième produit du [plan fondateur](../plan-fondateur.md) (section 2, produit 2) : pour un objectif de
craft donné, **quelle stratégie a le meilleur coût espéré, et vaut-il mieux acheter l'objet ?**

## Ce qu'on reprend

| Source | Reprise | Où |
|---|---|---|
| poe2-craft-assistant, `craft-core` (moteur exact : tirage pondéré, monnaies, Omens, Essences, Désécration, objectif, Monte-Carlo) | **oui, tel quel**, avec ses tests | `poe2-core::forge` (PR 1) |
| poe2-craft-assistant, `craft-solver` (reverse-crafting : MDP sur un état abstrait, coût espéré, P90, vérification Monte-Carlo sur le moteur exact) | **oui** | `poe2-core::forge` (PR 3) |
| Tables RePoE d'Augur (`tools/gamedata/import_repoe.py`, PR #29) : mods par palier, groupes d'exclusion, niveaux, bases | oui | éligibilité d'un mod sur une base |
| Poids poe2db (estimations Krakenbul, CC BY-NC-SA 3.0, PR #40) : 59 pages, 6 434 poids, 1 961 mods « inconnus » | oui, **avec l'attribution** | poids des tirages |
| Calculateur `tools/theorycraft/craft_proba.py` (PR #39) | sert de **référence** : le moteur Rust doit redonner ses probabilités | tests croisés (PR 2) |
| Parseur Ctrl+Alt+C d'Augur (`poe2-core::parser`) | oui | lire la base en cours de craft (PR 6) |
| Jeu de données de poe2-craft-assistant (`data/poe2/dataset.json`) | **non** : ses poids viennent de Craft of Exile, données tierces non redistribuables | — |
| Prix poe.ninja de poe2-craft-assistant (`craft-api`, `prices.rs`) | **oui** (accord de Max le 07/11), extraction et règles de cadence reprises | `forge::ninja` |
| Interface React Flow de poe2-craft-assistant | non : l'interface d'Augur a son propre style (React 19 + Tailwind) | — |

Le code repris garde ses noms (`AffixPool`, `Currency`, `Goal`…) pour rester comparable à l'original ; seuls changent
les chemins de modules, rand 0.9 (déjà dans le `Cargo.lock` d'Augur) au lieu de 0.8, et les retouches demandées par
clippy en édition 2024.

## Règles de données

- **Jamais de poids inventé.** Un mod sans poids mesuré (« inconnu » chez poe2db, ou base sans page de poids) n'est pas
  remplacé par une valeur par défaut : la probabilité qui en dépend est marquée **indisponible**, avec la liste des mods
  en cause. (Même règle que `craft_proba.py`.)
- **Chaque probabilité porte sa source** (plan fondateur, risque principal) : `extraite` (fichiers du jeu, RePoE),
  `estimée` (poe2db / Krakenbul), `supposée` (règle de mécanique non mesurée).
- Bases sans poids chez poe2db au 05/11 : griffes, dagues, épées et haches à une et deux mains, fléaux, pièges, joyaux,
  charmes, flacons, armures de corps force/dextérité/intelligence. Forge les affiche « non supportées » tant qu'aucune
  source n'existe.
- Page poe2db d'une base : classe au pluriel (`Rings`, `Quarterstaves` pour les bâtons de guerre…), suivie des
  attributs pour les armures (`Body_Armours_int`, `Shields_str_dex`). Pas de page pour les armures force /
  dextérité / intelligence hors armures de corps, ni pour les quelques bases d'armure sans attribut.
- Limites du pool actuel : pas de mods désacralisés (aucun poids chez poe2db, la Désécration n'est donc pas
  calculable) ; plafond de la base fixé à 3 préfixes / 3 suffixes (les implicites « +1 Prefix Modifier allowed »
  ne sont pas encore extraits des tables) ; joyaux non traités (pas de bases de joyaux dans les tables).
- L'attribution CC BY-NC-SA des poids suit les données partout où elles vont (paquet de données, appli, écran « À
  propos »). Le dépôt reste sans licence.

## Prix

Le coût espéré se compte en Exalted Orbs. Aucune source officielle n'est ouverte (GGG n'accorde pas de client OAuth
pour l'instant, voir [api-ggg.md](../architecture/api-ggg.md)). **Décision de Max (7 novembre 2026) :** poe.ninja
**en plus** des prix saisis. Chaque prix effectif vient, dans l'ordre :

1. d'une **correction** saisie par le joueur ;
2. du dernier **relevé poe.ninja** (API économique publique PoE 2, catégories `Currency` et `Ritual`), converti en
   Exalted. Le relevé est déclenché par le joueur, au plus une fois toutes les 5 minutes (10 après un échec), avec un
   User-Agent qui identifie Augur. Un prix absent d'un relevé garde sa dernière cote (même ligue) ;
3. d'une valeur **indicative** (relevé de septembre 2026 de poe2-craft-assistant).

La base neuve et sa revente n'ont pas de cote : elles restent saisies. La comparaison « craftez ou achetez » attend
une source de prix d'objets (même blocage OAuth).

## Découpage en PR

1. **Moteur exact** (`poe2-core::forge`, Rust pur) : modèle d'objet, monnaies, Omens, Essences, Désécration, objectif,
   Monte-Carlo, avec les tests d'origine. *Cette PR, avec ce cadrage.*
2. **Pool d'une base** (`forge::donnees`) : construire l'`AffixPool` d'une base à un niveau d'objet à partir des
   tables RePoE et des poids poe2db, mods sans poids listés à part ; distribution exacte du prochain mod ; tests
   croisés avec `craft_proba.py` sur trois vraies bases (`tools/theorycraft/fixture_forge.py`).
3. **Solveur** (`forge::solveur`, `forge::catalogue`) : reverse-crafting repris de `craft-solver` (MDP, policy
   iteration), coût espéré, quantiles P50 / P90, plan de décision, vérification sur le moteur exact ; catalogue des
   18 monnaies (Greater / Perfect compris) et des 6 Omens d'Exaltation, d'Annulment et d'Erasure. Essences,
   Désécration et Liquid Emotions viendront avec leurs données (cible par base, poids des mods désacralisés).
4. **Prix** (`forge::prix`, `forge::ninja`, commandes `get_forge_prices` / `set_forge_prices` /
   `reset_forge_prices` / `refresh_forge_prices`) : table de prix enregistrée dans `forge-prix.json` (dossier de
   configuration d'Augur) ; corrections du joueur, relevé poe.ninja, valeurs indicatives. Une saisie invalide est
   refusée en bloc, un fichier abîmé est réparé au chargement.
5. **Panneau Forge** dans la fenêtre principale : choix de la base et de l'objectif, plan, coût espéré, P90, sources.
6. **Overlay** : Ctrl+Alt+C sur l'objet en cours de craft, l'overlay affiche la prochaine étape du plan.

Hors périmètre pour l'instant : « craftez ou achetez » (pas de prix d'objets), boucle de validation communautaire.

## Hypothèses à vérifier

Les règles du moteur sont celles de poe2-craft-assistant (jeu 0.5). À revérifier pour le jeu actuel (0.5.5e) :

| # | Hypothèse | Comment vérifier |
|---|---|---|
| F-01 | Un objet ne porte qu'**un** mod crafted (Essence, Alloy, Liquid Emotion) depuis 0.5.0. | Essence sur un objet qui en porte déjà une : le jeu refuse. |
| F-02 | Chaos : retrait **puis** ajout, le tirage tient compte du retrait. | Notes de patch / poe2wiki. |
| F-03 | Alchemy sur un objet Magique repart de zéro (4 mods, patch 0.3.1). | poe2wiki, Orb of Alchemy. |
| F-04 | Os Ancient : niveau de mod 40 minimum pour la Désécration. | poe2db, Ancient bones. |
| F-05 | Les poids poe2db sont ceux du jeu actuel (relevé du 05/11, RePoE 0.5.5.5). | Nouveau relevé à chaque patch. |
