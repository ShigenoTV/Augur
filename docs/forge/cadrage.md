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
- Limites du pool actuel : mods de liche (Désécration) à poids 1 pour tous, faute de mesure (poe2db n'en a
  pas, Craft of Exile met 1 partout ; voir « Désécration » plus bas) ; plafond de la base fixé à 3 préfixes / 3 suffixes (les implicites « +1 Prefix Modifier allowed »
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
   18 monnaies (Greater / Perfect compris) et des Omens (voir « Omens » plus bas). **Essences**
   (08/11) : mod garanti par classe d'après Craft of Exile (`tools/theorycraft/coe_essences.py`,
   `donnees/essences-coe.json`). Depuis le 08/11 aussi les Essences à plusieurs mods possibles (Infinite,
   Enhancement, Greater Battle, Abyss) et les 13 Alloys ; une Essence ou un Alloy n'est proposé que s'il a un
   prix (poe.ninja, catégories `Essences` et `Verisium`, ou saisie). Les mods à plusieurs groupes d'exclusion
   (Swift Alloy sur anneau et gants, Transcendent Alloy sur arme d'attaque) sont gérés depuis le 09/11 (voir
   « Alloys hybrides » plus bas). Os abyssaux (Désécration) au catalogue depuis le 21/11 (voir « Désécration »
   plus bas). Liquid Emotions hors catalogue : elles ne posent un mod que sur les joyaux, que Forge ne traite pas.
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
   **Essais enchaînés** (`forge::enchainement`, demande de Max du 19/11) : d'un Ctrl+Alt+C à l'autre, le panneau
   reste le même et se met à jour ; un bandeau en tête dit « Ça a marché, reste X » (coût restant en baisse ou
   objet fini) ou « Raté, reste X », avec le nombre d'essais de la série et la dépense estimée (coût unitaire de
   chaque action conseillée puis tentée). Une copie identique ne compte pas d'essai. La série repart de zéro à
   chaque nouveau plan suivi, à l'arrêt du suivi, après un objet fini ou une base neuve conseillée ; elle n'est
   gardée qu'en mémoire.
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


## Omens (21/11/2026)

Tous les Omens qui touchent aux affixes, hors Désécration, sont au catalogue. Règles relevées dans l'émulateur
de Craft of Exile (`exalt`, `annul`, `poe2_alchemy`, `regalParams`, `poe2_essence`), sans copier de données :

| Omen | Monnaie | Effet dans Forge |
|---|---|---|
| Sinistral / Dextral Exaltation | Exalted (toutes) | mod ajouté du côté choisi |
| Greater Exaltation | Exalted (toutes) | deux mods ajoutés, le second seulement s'il reste une place ; se combine avec Sinistral / Dextral Exaltation |
| Sinistral / Dextral Annulment | Annulment | retrait du côté choisi |
| Greater Annulment | Annulment | deux mods retirés (le second s'il en reste un) ; se combine avec Sinistral / Dextral Annulment |
| Sinistral / Dextral Erasure | Chaos (toutes) | retrait du côté choisi |
| Whittling | Chaos (toutes) | retrait du mod du niveau le plus bas (égalité : au hasard) |
| Sinistral / Dextral Coronation | Regal (toutes) | mod ajouté du côté choisi |
| Sinistral / Dextral Alchemy | Alchemy | autant de mods que possible du côté choisi (3 en général), puis le reste des 4 de l'autre côté |
| Sinistral / Dextral Crystallisation | Essence Perfect ou Corrupted | le mod retiré avant la pose est pris du côté choisi |

Un Omen sans prix (ni cote poe.ninja, ni saisie) n'est pas proposé. Le 21/11, poe.ninja ne cote ni Greater
Annulment, ni Coronation, ni Alchemy : il faut saisir leur prix dans l'onglet Prix pour que le plan les utilise.

Pas au catalogue : **Homogenising Exaltation / Coronation** (le mod ajouté doit partager une étiquette avec un mod
déjà présent ; l'état abstrait du solveur ne connaît pas les étiquettes des mods hors objectif), **Catalysing
Exaltation** (qualité de catalyseur, que Forge ne suit pas), **Putrefaction** (voile tous les mods et corrompt)
et ceux qui ne touchent pas aux affixes (Sanctification, Blessed, Corruption, Chance, Ancients, Omens de Waystone…).

Tests : `solveur::tests::nouveaux_omens_meme_loi_que_le_moteur` (loi d'un essai, modèle contre moteur exact,
depuis cinq objets), `plan_avec_les_nouveaux_omens` (plan complet simulé), `alchemy_et_crystallisation_sur_le_moteur`.


## Désécration (21/11/2026, approximatif)

Règles relevées dans l'émulateur de Craft of Exile 0.5.5.5 (`poe2_desecrate`, `getRevealPicks`), sans copier
de données. Forge traite l'os et la révélation au Well of Souls comme une seule action :

1. **Os** selon la classe : Jawbone (armes, carquois), Rib (armures, boucliers, focus), Collarbone (bijoux),
   Cranium (joyaux, Preserved seulement). Gnawed : objet de niveau 64 au plus ; Preserved : sans condition ;
   Ancient : niveau de mod 40 minimum (options de liche et du pool normal). Prix poe.ninja, catégorie `Abyss`.
2. **Côté** : forcé par l'Omen of Sinistral / Dextral Necromancy (un mod de ce côté retiré s'il est plein) ;
   sinon un mod retiré au hasard si l'objet est plein, puis un côté ouvert au hasard (50/50 si les deux le sont).
3. **Révélation** : trois options, dont 1, 2 ou 3 mods de liche (80 / 15 / 5 %), le reste tiré dans le pool
   normal de ce côté ; tirages pondérés sans remise, un groupe tiré écarte ses autres paliers. Omen of the
   Sovereign / Liege / Blackblooded : une des options est un mod d'Ulaman / Amanamu / Kurgal. Omen of Abyssal
   Echoes : un second jeu d'options. Le joueur prend un mod voulu s'il y en a un, sinon un mod inutile plutôt
   qu'un palier trop bas d'un voulu.
4. Le mod révélé est **Désécré** : un seul par objet (pas de seconde Désécration tant qu'il est là), et l'Omen
   of Light (avec l'Orb of Annulment) ne retire que lui.

Données : les 197 mods `AbyssMod…` étiquetés `ulaman_mod`, `amanamu_mod` ou `kurgal_mod` du domaine
`desecrated` de RePoE (`forge_data.py`), poids 1 pour tous (aucune mesure publique ; Craft of Exile fait de
même). Ils partagent le groupe d'exclusion des mods normaux du même effet, mais forment leur propre famille
(paliers à part). Le coût d'un plan qui passe par un os est donc **approximatif**, et le panneau marque ces mods
« Os ».

Modèle du solveur : options tirées indépendamment (approximation du tirage sans remise), mêmes règles sinon.
Omens combinés : Necromancy avec une liche (6 paires). Pas encore : Omen of Putrefaction, os Breach (Altered
Collarbone) et Vertebrae (Waystones).

Recoupement avec l'émulateur (21/11, `tools/theorycraft/coe_emulateur_poe2.js`, cas exportés par
`atelier::recoupement::exporter_cas_desecration`, 20 000 essais) : chance que la famille visée soit proposée.

| Cas | Moteur Augur | Modèle du solveur | Émulateur |
|---|---|---|---|
| Anneau, Preserved Collarbone + Dextral Necromancy, résistance feu | 15,7 % | 14,6 % | 15,2 ± 0,3 % |
| Anneau, Preserved Collarbone + Dextral Necromancy + Liege, vol de vie (Amanamu) | 21,5 % | 21,3 % | 18,6 ± 0,3 % |
| Anneau, Preserved Collarbone sans Omen, vol de vie (Amanamu) | 4,3 % | 4,1 % | 4,0 ± 0,1 % |
| Robe à 3 préfixes, Ancient Rib, résistance feu | 20,3 % | 18,8 % | 19,6 ± 0,3 % |
| Robe, Preserved Rib + Abyssal Echoes, vie | 43,5 % | 40,4 % | 42,3 ± 0,4 % |
| Anneau ilvl 60, Gnawed Collarbone + Dextral Necromancy, résistance froid | 24,8 % | 22,7 % | 14,9 ± 0,3 % |

Écarts connus : le modèle du solveur est un peu pessimiste (options indépendantes) ; l'Omen de liche donne un
peu plus chez Augur (l'émulateur a sans doute plus de mods de cette liche) ; sur un objet de niveau 64 ou moins,
l'émulateur propose des mods de liche alors que RePoE leur donne le niveau 65 (Augur suit RePoE : à vérifier en
jeu, I-D1).

Tests : `monte_carlo::tests::revelation_trois_options_et_omen_de_liche` (loi 80/15/5, Omen de liche, choix du
joueur), `solveur::tests::desecration_meme_loi_que_le_moteur` (modèle contre moteur exact, cinq actions depuis
quatre objets), `plan_avec_desecration` (plan complet simulé sur le moteur exact).

## Test complet contre l'émulateur (21/11/2026)

Une base par classe (la plus haute), plusieurs crafts par classe, comparés à l'émulateur de Craft of
Exile : PoE 2, 27 classes × 6 crafts (`exporter_test_complet_poe2`, `tools/theorycraft/coe_test_complet_poe2.js`) ;
PoE 1, 24 classes × 4 crafts (`exporter_test_complet_poe1`, `tools/theorycraft/coe_emulateur.js`).

- Alchemy, Transmute, Exalt, Greater Exalt, Chaos (PoE 2), Alteration, Chaos, fossile, Harvest Reforge
  (PoE 1) : écart moyen ≤ 0,3 point, maximum ≈ 1 point.
- Corrigé grâce au test (PoE 1) : multiplicateurs de génération (`generation_weights`, mods d'attaque à
  50 % sur dagues runiques, bâtons, sceptres, baguettes de sort) et mods de PoE Royale écartés.
- Reste : Désécration PoE 2 (modèle 1 à 4 points trop prudent ; moteur décalé sur Gloves, Helmet, Quiver),
  Pristine Fossil sur bouclier PoE 1 (−3,2 points), bases expérimentales de Heist (l'émulateur n'y tire
  aucun mod, exclues du test).
