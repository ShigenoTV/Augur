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
  jeu refusent). Fists of Stone (gants d'Ascendance, et leur version Runeforged) sont écartées : Craft of Exile les
  marque non modifiables (contrôle du 08/11 : toutes nos autres bases existent chez lui, modifiables). Une base qui a sa propre page de poids (Trarthan Cannon, arbalète à grenades) l'utilise au lieu de
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

La base neuve et sa revente n'ont pas de cote : elles restent saisies. Les objets Rares n'ont de cote nulle part
(poe.ninja ne cote que les objets échangeables en vrac ; vérifié le 13/11/2026) : « crafter ou acheter » s'appuie
donc sur un prix d'achat que le joueur relève lui-même (point 8 ci-dessous).

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
   `donnees/essences-coe.json`). Depuis le 08/11 aussi les Essences à plusieurs mods possibles (Infinite,
   Enhancement, Greater Battle, Abyss) et les 13 Alloys ; une Essence ou un Alloy n'est proposé que s'il a un
   prix (poe.ninja, catégories `Essences` et `Verisium`, ou saisie). Les mods à plusieurs groupes d'exclusion
   (Swift Alloy sur anneau et gants, Transcendent Alloy sur arme d'attaque) sont gérés depuis le 09/11 (voir
   « Alloys hybrides » plus bas). Désécration et Liquid Emotions restent hors catalogue (pas de poids des mods
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
   dernier plan calculé est suivi (jusqu'au prochain calcul ou à « Arrêter le suivi » ; gardé à la fermeture d'Augur et
   recalculé au démarrage, aux prix du moment). Ctrl+Alt+C sur un
   objet de la même base : chaque mod est retrouvé dans le pool (côté et nom de l'en-tête avancé, puis texte sans les
   nombres, puis tier), l'objet est projeté sur l'état du solveur et l'overlay affiche la prochaine étape, le coût
   restant et les résultats possibles du prochain essai. Un mod introuvable (sans poids, désacralisé, artisanal)
   compte comme mauvais mod. Une copie simple (Ctrl+C) est refusée : elle ne dit pas quels mods sont des
   préfixes. Un objet d'une autre base passe à Upgrade Copilot.
7. **Historique** (`forge::historique`, `forge-historique.json`, commandes `get_forge_history` /
   `delete_forge_history_entry`, demande de Max du 08/11) : chaque plan calculé y entre (50 au plus, la même
   demande recalculée remplace son entrée) ; onglet Historique du panneau pour le rouvrir (nouveau calcul) ou le
   supprimer.

8. **Crafter ou acheter** (`forge::achat`, commande `open_forge_buy_search`, demande de Max du 13/11) : sous le
   plan calculé, « Chercher l'objet à l'achat » ouvre dans le navigateur du joueur la recherche de l'objet visé sur
   le site de trade (même base, non unique et non corrompu, en ligne, du moins cher au plus cher ; chaque ligne de
   mod voulu devient un filtre au minimum du tier visé, la borne basse du palier le plus faible accepté ; « Adds X
   to Y » = moyenne des deux bornes basses ; stat globale ou locale acceptée par un groupe « count » ; stat
   rangée sous sa forme inverse (« reduced ») en maximum négatif). Augur n'envoie aucune requête au site : le
   joueur reporte le prix qu'il y voit, dans la monnaie d'affichage. Forge le compare au coût moyen du plan (base
   neuve comprise, sauf depuis un objet) : « crafter », « acheter », ou « équivalent » sous 10 % d'écart, plus la
   part des crafts simulés qui coûtent moins que l'achat. Le prix saisi est gardé par demande (stockage local)
   pour qu'un plan rouvert le retrouve. Identifiants de stats : `crates/poe2-core/data/trade-stats.json`
   (`tools/gamedata/trade_stats.py`, 257 lignes de mods reliées, 15 sans stat de trade : pêche, pièges, multiplicateurs
   de dégâts dans le temps). Une ligne sans stat reste hors filtre et le panneau la signale.

Hors périmètre pour l'instant : relevé automatique du prix d'achat (il faudrait interroger le site de trade
depuis Augur), boucle de validation communautaire.

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

Essences et Alloys, d'après le même émulateur (`poe2_essence`) : Lesser / Normal / Greater sur un objet Magique, qui
devient Rare avec le mod garanti ; Perfect / Corrupted / Alloy sur un Rare. Quand l'Essence a plusieurs mods
possibles pour la classe, un seul est posé, tiré **à chances égales** parmi ceux dont le groupe n'est pas déjà
sur l'objet (refusée si aucun) ; l'émulateur ne filtre pas ces mods selon la base (une Essence of Enhancement
peut poser un % d'évasion sur un casque d'armure). Sur un Rare, un mod est ensuite retiré **du côté du mod tiré si
ce côté est plein**, au hasard sinon, puis le mod est posé. Vérifié le 08/11 sur 6 cas × 3 000 essais lancés
dans l'émulateur (test `memes_tirages_que_craft_of_exile`). Le niveau d'objet n'est pas regardé : un mod garanti au-dessus du niveau d'objet, ou réservé aux Essences
(% de vie des armures de corps…), entre dans le pool avec un poids nul et peut être visé.

**Alloys hybrides** (09/11) : le mod du Swift Alloy sur anneau (vitesse d'attaque) et sur gants (vitesse
d'incantation) prend les deux groupes vitesse d'attaque ET vitesse d'incantation ; celui du Transcendent Alloy sur
arme d'attaque (attributs et % de dégâts physiques) prend Force, Dextérité et Intelligence. Règle de l'émulateur :
deux mods s'excluent dès qu'ils ont **un groupe en commun** (`Affix::extra_groups` dans le moteur). Donc l'Alloy est
refusé si l'objet porte un mod de l'un de ces groupes (l'objet reste tel quel, comme pour une Essence), et une fois posé, aucun mod de ces groupes n'est plus tiré. Vérifié le 09/11 dans l'émulateur, 7 cas ×
3 000 essais (test `alloys_hybrides_comme_craft_of_exile`). Dans le solveur, un mod voulu à plusieurs groupes
occupe une « zone » : tout mod de l'un de ses groupes la bloque (un second bloqueur, ex. Force ET Dextérité, compte
comme un mauvais affixe). Ces Alloys ne sont proposés que quand leur mod est voulu : hors objectif, l'état
abstrait du solveur ne sait pas quels mauvais affixes les bloqueraient. Un objectif qui veut à la fois le mod
hybride et un mod de l'un de ses groupes (Transcendent + Dextérité) est refusé.

`tools/theorycraft/coe_controle.py` vérifie aussi, base par base, les mods écartés par chaque niveau minimum.
