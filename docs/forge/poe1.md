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

Choix de l'objet : classe, base (les homonymes comme Two-Stone Ring sont départagés par leur implicite), niveau
d'objet, influence (Shaper, Elder, Crusader, Redeemer, Hunter, Warlord), puis les affixes voulus au tier choisi ou
mieux. Hors de ce jalon : fossiles et résonateurs, craft d'établi et metamods, Exalted / Annulment, Harvest,
Eldritch, Veiled, double influence, joyaux et flacons, suivi dans l'overlay.

## Données

Une seule source : **RePoE PoE 1** (https://repoe-fork.github.io/, version notée dans le fichier), tables
extraites des fichiers du jeu, avec les **vrais poids** de génération des mods. Pas d'estimation tierce comme en
PoE 2. `tools/gamedata/forge_poe1_data.py` produit `crates/poe2-core/data/forge-poe1.json.gz` (environ 100 Ko) :

- 24 classes d'équipement et leurs étiquettes d'influence (`item_classes.json`) ;
- les bases publiées de ces classes, avec leurs implicites (`base_items.json`) ;
- les préfixes et suffixes du domaine `item` qui ont un poids, et les mods garantis des Essences (`mods.json`) ;
- les Essences : mod garanti par classe, niveau d'objet maximal (Whispering 35, Muttering 45, Weeping 60, Wailing
  75), et si elles refont un objet Rare (« reforges a rare item » : Screaming et au-dessus, Essences spéciales).

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

Fossiles et résonateurs (effets réels dans `fossils.json`), craft d'établi et metamods (demande un solveur PoE 1),
Alteration + Regal, suivi dans l'overlay (Ctrl+Alt+C), mise à jour PoE 1 dans `mise_a_jour.py`.
