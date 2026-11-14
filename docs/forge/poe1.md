# Forge PoE 1 : cadrage du premier jalon

**Statut :** premier jalon, 14 novembre 2026 · **Code :** [`crates/poe2-core/src/poe1/forge`](../../crates/poe2-core/src/poe1/forge),
panneau [`ForgePoe1Card.tsx`](../../apps/desktop/src/index/ForgePoe1Card.tsx)

En mode POE 1, l'onglet Forge compare les méthodes de **spam** pour obtenir un objet : pour chaque méthode, la
chance de réussite d'un essai, le coût d'un essai, le coût moyen et le nombre d'essais (médian et 9 fois sur 10).
Le mode POE 2 ne change pas.

## Périmètre

| Méthode | Ce qui est calculé | Monnaies d'un essai |
|---|---|---|
| Alteration + Augmentation | objectif d'au plus 1 préfixe et 1 suffixe ; l'Augmentation n'est utilisée que si l'objet n'a qu'un mod et qu'elle peut le finir | 1 Alteration + Augmentations (moyenne) |
| Chaos Orb | objet Rare refait | 1 Chaos |
| Alchemy + Scouring | même chance que le Chaos | 1 Alchemy + 1 Scouring |
| Chaque Essence utile | Essence dont le mod garanti est un des mods voulus, utilisable à ce niveau d'objet | 1 Essence (+ 1 Scouring sous Screaming) |
| … + établi | chaque méthode ci-dessus, finie par le craft d'établi d'un des mods voulus (le meilleur pour la méthode) : il suffit que les autres mods voulus sortent et qu'il reste une place libre de son côté | celles de la méthode, plus le craft payé une fois |

Choix de l'objet : classe, base (les homonymes comme Two-Stone Ring sont départagés par leur implicite), niveau
d'objet, influence (Shaper, Elder, Crusader, Redeemer, Hunter, Warlord), puis les affixes voulus au tier choisi ou
mieux. Hors de ce jalon : fossiles et résonateurs, metamods, Exalted / Annulment, Harvest,
Eldritch, Veiled, double influence, joyaux et flacons, suivi dans l'overlay.

## Données

Une seule source : **RePoE PoE 1** (https://repoe-fork.github.io/, version notée dans le fichier), tables
extraites des fichiers du jeu, avec les **vrais poids** de génération des mods. Pas d'estimation tierce comme en
PoE 2. `tools/gamedata/forge_poe1_data.py` produit `crates/poe2-core/data/forge-poe1.json.gz` (environ 100 Ko) :

- 24 classes d'équipement et leurs étiquettes d'influence (`item_classes.json`) ;
- les bases publiées de ces classes, avec leurs implicites (`base_items.json`) ;
- les préfixes et suffixes du domaine `item` qui ont un poids, et les mods garantis des Essences (`mods.json`) ;
- les Essences : mod garanti par classe, niveau d'objet maximal (Whispering 35, Muttering 45, Weeping 60, Wailing
  75), et si elles refont un objet Rare (« reforges a rare item » : Screaming et au-dessus, Essences spéciales) ;
- les crafts d'établi qui ajoutent un mod (`crafting_bench_options.json`, 727 crafts) : mod artisanal, classes et
  coût par monnaie. On suppose tous les crafts débloqués. Un seul mod artisanal par objet (sans metamod).

Tier d'un mod jamais tiré (Essence, établi) : celui du meilleur tier tirable de sa famille dont la première valeur
minimale ne dépasse pas la sienne (« vaut au moins ce tier »).

Poids d'un mod sur une base : celui de la **première** étiquette de sa liste `spawn_weights` portée par l'objet
(étiquettes de la base, plus celle de l'influence). Tiers : par famille (`type`), parmi les mods tirables sur la
base, T1 = niveau requis le plus haut.

## Règles de tirage

Relevées dans le code de Craft of Exile pour PoE 1 (`packages/package.js`, 14/11/2026), sans copier de données :

- objet **Magique** refait : 1 ou 2 mods à 50/50 ; avec 1 mod, le côté suit les poids ; avec 2, un préfixe et un suffixe ;
- objet **Rare** refait : 4, 5 ou 6 mods à 8/12, 3/12 et 1/12, tirés l'un après l'autre (3 préfixes et 3 suffixes
  au plus, un mod par groupe d'exclusion) ;
- **Essence** : mod garanti d'abord, puis le reste au hasard avec la même loi du nombre de mods (hypothèse P1-01,
  à confirmer en jeu).

## Calcul

- **Magique : exact.** Formule fermée sur les poids ; un test la compare à l'énumération complète sur le moteur
  (écart < 10⁻⁹).
- **Rare : approché.** Chaque mod voulu est suivi exactement ; les autres mods sont résumés par côté (poids restant
  moyen après k tirages sans remise), comme le solveur PoE 2. Les tests le comparent à 200 000 objets simulés :
  écart de 2 à 3 % sur un anneau (vie + résistances, avec ou sans Essence).
- **Coût** : fait par le panneau, à partir des prix poe.ninja PoE 1 (pages `Currency` et `Essence` de la ligue
  PoE 1 des réglages, en Chaos), ou du prix saisi par le joueur (gardé sur le poste). Le prix de la base n'est pas
  compté. Coût moyen = coût d'un essai / chance ; essais médian et 9 sur 10 par la loi géométrique.

- **Établi** : le voulu craftable n'est plus à tirer ; l'essai réussit si les autres voulus sortent, sans mauvais
  tier de son groupe et avec une place libre de son côté. Rare : même calcul que ci-dessus ; Magique : énumération
  exacte des tirages (l'Augmentation sert aussi quand elle laisse la place au craft). Testé contre la simulation
  (écart < 3 %). Le coût compte le craft même quand le mod sort tout seul : c'est un majorant léger.

## Plan conseillé : solveur Exalt / Annul / metamods (`poe1::forge::solveur`)

Choisi par Max le 14/11 (option « Spams + établi et metamods »). Au lieu de comparer des spams, le solveur cherche
la meilleure décision dans chaque situation et donne le coût moyen minimal pour finir l'objet.

- **Actions** : Transmutation, Alteration, Augmentation, Regal, Alchemy, Chaos, Essences, Exalted, Annulment,
  Scouring, metamods d'établi (« Prefixes / Suffixes Cannot Be Changed », « Cannot roll Attack / Caster
  Modifiers »), retrait du craft (1 Orb of Scouring), craft d'établi final. Un seul mod d'établi à la fois (pas de
  multimod).
- **Règles des metamods**, relevées dans l'émulateur Craft of Exile PoE 1 : Chaos et Scouring gardent le côté
  verrouillé, le metamod part ; le Chaos tire 4, 5 ou 6 mods au total, moins les mods gardés ; Annulment retire un
  mod au hasard hors du côté verrouillé, metamod compris ; « Cannot roll Attack / Caster » retire des ajouts les
  mods aux tags `attack` / `caster` (`implicit_tags` de RePoE, champ `tags` des données).
- **État** : rareté, voulus présents, voulus bloqués par un mauvais tier, nombre d'autres mods par côté (poids
  restant moyen, comme les spams Rare), metamod posé.
- **Méthode** : plus court chemin stochastique. Les relances (Chaos, Alteration, Essence, Chaos verrouillé…) mènent
  à une loi d'objets commune, traitée comme un nœud. Itération de politique : chaque politique est évaluée
  exactement par un système creux résolu par BiCGSTAB, puis chaque état prend sa meilleure action. Les nombres
  moyens d'actions et de monnaies viennent des passages moyens par état (système transposé).
- **Prix** : le panneau envoie la cote du marché corrigée par les saisies du joueur ; une action sans prix est
  écartée et listée.
- **Limite** : jusqu'à 4 mods voulus (moins d'une seconde). À 5 ou 6 mods, la recherche d'une première politique
  qui aboutit échoue encore : le panneau le dit.
- **Validation** (tests `solveur::tests`) : Chaos seul retrouve la formule du spam ; sur une Glorious Plate, la
  politique optimale simulée sur le moteur exact donne 7,58 c contre 7,57 c calculés, et 190,4 c contre 191,2 c
  dans un cas à verrous (Exalted cher, metamods bon marché ; test long, lancé en release avec `--ignored`).

## Réutilisé de la Forge PoE 2

Le modèle d'objet et le tirage pondéré (`forge::moteur`), l'objectif (`forge::objectif`), la colonne d'affixes du
panneau, le cache poe.ninja de l'overlay PoE 1. Le solveur PoE 2 (MDP) n'est pas utilisé : il ne connaît pas les
monnaies qui refont tout l'objet.

## Hypothèses à vérifier

| Id | Hypothèse | Comment vérifier |
|---|---|---|
| P1-01 | Une Essence suit la même loi 4/5/6 mods que le Chaos, mod garanti compris | 50 Essences sur un objet, compter les mods |
| P1-02 | Un objet Magique refait a 2 mods une fois sur deux | 100 Alterations, compter |

## Suite possible (sur décision de Max)

Fossiles et résonateurs (effets réels dans `fossils.json`), plan pas à pas dans le panneau (prochaine étape choisie
par Max le 14/11), solveur au-delà de 4 mods voulus, multimod, suivi dans l'overlay (Ctrl+Alt+C), mise à jour PoE 1
dans `mise_a_jour.py`.
