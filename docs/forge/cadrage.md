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
| Poids Craft of Exile (mêmes estimations, données de l'émulateur 0.5.5.5) : 3 347 poids de mods absents de poe2db, 51 pages ; les 6 434 poids communs sont identiques | oui, **seulement là où poe2db n'a rien**, avec l'attribution (décision de Max, 08/11) | poids des tirages |
| Calculateur `tools/theorycraft/craft_proba.py` (PR #39) | sert de **référence** : le moteur Rust doit redonner ses probabilités | tests croisés (PR 2) |
| Parseur Ctrl+Alt+C d'Augur (`poe2-core::parser`) | oui | lire la base en cours de craft (PR 6) |
| Jeu de données de poe2-craft-assistant (`data/poe2/dataset.json`) | non : remplacé par le relevé direct de Craft of Exile ci-dessus | — |
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
  charmes, flacons, armures force/dextérité/intelligence. Depuis le 08/11, Craft of Exile complète les armes et les
  armures ; restent sans poids (aucune source) : joyaux, charmes, flacons, pièges, mods désacralisés.
- Une base n'est proposée que si **tous** ses mods ont un poids. Contrôle du 08/11 (`tools/theorycraft/coe_controle.py`) :
  1 714 bases × 4 niveaux d'objet, 6 856 pools identiques à Craft of Exile. Écartées : bases `[DNT]` (pas en jeu),
  Energy Blade (créée par une compétence), Grasping Mail (Craft of Exile lui donne des mods de bijou que les tables du
  jeu refusent). Une base qui a sa propre page de poids (Trarthan Cannon, arbalète à grenades) l'utilise au lieu de
  celle de sa classe.
- Page poe2db d'une base : classe au pluriel (`Rings`, `Quarterstaves` pour les bâtons de guerre…), suivie des
  attributs pour les armures (`Body_Armours_int`, `Shields_str_dex`). Pas de page pour les armures force /
  dextérité / intelligence hors armures de corps, ni pour les quelques bases d'armure sans attribut.
- Limites du pool actuel : pas de mods désacralisés (aucun poids chez poe2db ni chez Craft of Exile, qui met 1
  partout : la Désécration n'est donc pas calculable, contrôle du 08/11) ; plafond de la base fixé à 3 préfixes / 3 suffixes (les implicites « +1 Prefix Modifier allowed »
  ne sont pas encore extraits des tables) ; joyaux non traités (pas de bases de joyaux dans les tables).
- L'attribution des poids suit les données dans le dépôt (`LICENCE-poids-poe2db.md`, `LICENCE-poids-coe.md`). L'appli
  n'affiche pas les sources (choix de Max, 08/11). Le dépôt reste sans licence.

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
   18 monnaies (Greater / Perfect compris) et des 6 Omens d'Exaltation, d'Annulment et d'Erasure. **Essences**
   (08/11) : mod garanti par classe d'après Craft of Exile (`tools/theorycraft/coe_essences.py`,
   `donnees/essences-coe.json`), seulement celles à une cible par classe (Infinite, Enhancement, Greater Battle,
   Abyss écartées) et hors Alloys ; une Essence n'est proposée que si elle a un prix (poe.ninja, catégorie
   `Essences`, ou saisie). Désécration et Liquid Emotions restent hors catalogue (pas de poids des mods
   désacralisés).
4. **Prix** (`forge::prix`, `forge::ninja`, commandes `get_forge_prices` / `set_forge_prices` /
   `reset_forge_prices` / `refresh_forge_prices`) : table de prix enregistrée dans `forge-prix.json` (dossier de
   configuration d'Augur) ; corrections du joueur, relevé poe.ninja, valeurs indicatives. Une saisie invalide est
   refusée en bloc, un fichier abîmé est réparé au chargement.
5. **Panneau Forge** dans la fenêtre principale (`forge::atelier`, commandes `get_forge_bases` /
   `get_forge_affixes` / `get_forge_catalogue` / `plan_forge_craft`) : données embarquées
   (`data/forge.json.gz`, `tools/gamedata/forge_data.py`), choix de la base, du niveau d'objet, des affixes voulus
   et des monnaies autorisées ; coût espéré, P50 / P90 sur 4 000 crafts simulés par le moteur exact, que faire
   selon l'état de l'objet, achats moyens ; onglet Prix (corrections, relevé
   poe.ninja, ligue). Seules les bases dont tous les mods ont un poids sont proposées.
6. **Overlay** (`forge::suivi`, commandes `get_forge_advice` / `get_forge_tracking` / `stop_forge_tracking`) : le
   dernier plan calculé est suivi (en mémoire, jusqu'au prochain calcul ou à « Arrêter le suivi »). Ctrl+Alt+C sur un
   objet de la même base : chaque mod est retrouvé dans le pool (côté et nom de l'en-tête avancé, puis texte sans les
   nombres, puis tier), l'objet est projeté sur l'état du solveur et l'overlay affiche la prochaine étape, le coût
   restant et les résultats possibles du prochain essai. Un mod introuvable (sans poids, désacralisé, artisanal)
   compte comme mauvais mod. Une copie simple (Ctrl+C) est refusée : elle ne dit pas quels mods sont des
   préfixes. Un objet d'une autre base passe à Upgrade Copilot.

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

Contrôle du 08/11 avec le code de l'émulateur de Craft of Exile (référence de Max) : F-01 à F-04 concordent
(Chaos : retrait puis ajout ; Alchemy : 4 mods ; Désécration Ancient : niveau 40). Trois écarts corrigés :

- niveau de mod minimum des Transmutation et Augmentation **Greater 44 / Perfect 70** (35 / 50 restent pour
  Regal, Exalted et Chaos) ;
- sous le niveau minimum, un tier n'est écarté que si sa famille a un tier accessible plus haut : le meilleur
  tier accessible d'une famille reste toujours tirable (avant : la famille entière était écartée) ;
- Omen de Whittling : à égalité de niveau, le mod retiré est tiré au hasard.

Essences, d'après le même émulateur (`poe2_essence`) : Lesser / Normal / Greater sur un objet Magique, qui devient
Rare avec le mod garanti ; Perfect / Corrupted sur un Rare, refusée si le groupe du mod garanti est déjà pris,
sinon un mod est retiré **du côté du mod garanti si ce côté est plein**, au hasard sinon, puis le mod garanti est
posé. Le niveau d'objet n'est pas regardé : un mod garanti au-dessus du niveau d'objet, ou réservé aux Essences
(% de vie des armures de corps…), entre dans le pool avec un poids nul et peut être visé.

`tools/theorycraft/coe_controle.py` vérifie aussi, base par base, les mods écartés par chaque niveau minimum.
