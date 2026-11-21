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
| Fossiles (1 à 4) | pour chaque taille de Chaotic Resonator, la meilleure combinaison de fossiles : coût moyen minimal aux prix connus, sinon chance maximale ; objet Normal ou Rare refait en Rare | les fossiles + le résonateur |
| … + établi | chaque méthode ci-dessus, finie par le craft d'établi d'un des mods voulus (le meilleur pour la méthode) : il suffit que les autres mods voulus sortent et qu'il reste une place libre de son côté | celles de la méthode, plus le craft payé une fois |

Choix de l'objet : classe, base (les homonymes comme Two-Stone Ring sont départagés par leur implicite), niveau
d'objet, influence (Shaper, Elder, Crusader, Redeemer, Hunter, Warlord), puis les affixes voulus au tier choisi ou
mieux. Hors de ce jalon : Harvest, Eldritch, Veiled, double influence, joyaux et flacons.

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
  coût par monnaie. On suppose tous les crafts débloqués. Un seul mod artisanal par objet (sans metamod) ;
- les étiquettes de chaque mod (`implicit_tags`), lues par les metamods et les fossiles ;
- les fossiles (`fossils.json`) : les 16 qui changent des poids (multiplicateur en % par étiquette de mod,
  1000 = ×10, 15 = ×0,15, 0 = bloqué), et depuis le 21/11 trois fossiles à effet spécial : **Faceted** (ses
  mods `delve` de niveau des gemmes entrent dans le tirage, au poids du jeu), **Hollow** (Abyssal Socket forcé,
  sur arme, casque, bottes, gants et armure) et **Sanctified** (tirage chanceux). Les mods `delve` sont dans
  les données (`fossile`), avec un poids nul hors fossile. Écartés : Tangled (effets au hasard, Essence
  corrompue une fois sur dix), Glyphic (Essence corrompue et objet corrompu), Fractured (copie fracturée,
  absente de l'émulateur de référence), Bloodstained (implicite corrompu), Gilded (prix de revente), Opulent
  (étiquette `drop`) ;
- depuis le 21/11 (lot 2), les **mods voilés** (domaine `unveiled`, champ `voile`, 260 mods) : ceux qu'un
  dévoilement peut proposer, avec leurs poids au dévoilement. Poids nul dans le pool (jamais tirés par Chaos ou
  Exalted) ; leurs tiers se comptent entre mods voilés de même famille. Pris dans RePoE 3.30.0.4 (le reste des
  données reste en 3.29.3.6). Le panneau les propose comme objectif, comme les mods `delve` des fossiles, avec
  la mention « jamais tiré au hasard ».

Tier d'un mod jamais tiré (Essence, établi) : celui du meilleur tier tirable de sa famille dont la première valeur
minimale ne dépasse pas la sienne (« vaut au moins ce tier »).

Poids d'un mod sur une base : celui de la **première** étiquette de sa liste `spawn_weights` portée par l'objet
(étiquettes de la base, plus celle de l'influence, ou des deux influences d'un objet à double influence).

**Double influence** (lot 3, 21/11) : le panneau propose une seconde influence ; la demande porte alors
`influence: "Hunter+Shaper"` (deux influences jointes par `+`, rangées par ordre alphabétique). Les mods des deux
influences sont tirables, l'objet collé à deux influences est reconnu et la recherche d'achat filtre les deux.
L'objet à double influence est supposé déjà en main (acheté, ou fait avec un Awakener's Orb) : l'Awakener's Orb
lui-même n'est pas modélisé (il faut deux objets influencés et le mod transféré dépend du second objet). Tiers : par famille (`type`), parmi les mods tirables sur la
base, T1 = niveau requis le plus haut.

## Règles de tirage

Relevées dans le code de Craft of Exile pour PoE 1 (`packages/package.js`, 14/11/2026), sans copier de données :

- objet **Magique** refait : 1 ou 2 mods à 50/50 ; avec 1 mod, le côté suit les poids ; avec 2, un préfixe et un suffixe ;
- objet **Rare** refait : 4, 5 ou 6 mods à 8/12, 3/12 et 1/12, tirés l'un après l'autre (3 préfixes et 3 suffixes
  au plus, un mod par groupe d'exclusion) ;
- **Essence** : mod garanti d'abord, puis le reste au hasard avec la même loi du nombre de mods (hypothèse P1-01,
  à confirmer en jeu) ;
- **fossiles** (`poec_getModdedWeight`, mode « additif » par défaut de l'émulateur) : par étiquette, un 0 la bloque,
  les bonus (> 1) de plusieurs fossiles s'additionnent et les malus (< 1) se multiplient ; par mod, une étiquette
  bloquée annule son poids, sinon le poids est multiplié par la somme des bonus de ses étiquettes et le produit de
  leurs malus. Exemple : Frigid (froid ×10) + Prismatic (élémentaire ×6) donne ×16 sur une résistance au froid.
  L'objet refait suit la loi 4/5/6 du Chaos (hypothèse P1-03).
- **fossiles à effet spécial** (`pool_fossiles_base`, même émulateur) : Sanctified multiplie chaque poids par
  `1 + (niveau du mod − 40) / 100` avant les étiquettes ; Faceted rend tirables ses mods ajoutés, puis les
  étiquettes s'appliquent (gemmes ×10) ; Hollow pose l'Abyssal Socket, puis le reste de l'objet est tiré
  comme après le mod garanti d'une Essence.

- **monnaies Eldritch** (`eldritchChaos`, `eldritchExalt`, `eldritchAnnul` de l'émulateur) : elles ne
  touchent que le côté dominant des implicites Eldritch (Exarch → préfixes, Eater → suffixes). Eldritch Chaos
  retire les mods de ce côté et tire 4, 5 ou 6 mods au total moins les mods restants, de ce côté seulement ;
  Eldritch Exalted ajoute un mod de ce côté ; Eldritch Annulment en retire un au hasard de ce côté. Le joueur
  choisit la dominance dans le panneau (« Implicites Eldritch ») : le prix des Embers et Ichors pour poser les
  implicites n'est pas compté. Pas d'implicites Eldritch sur un objet influencé ;
- **monnaies voilées** (`veiledChaos`, `veiledExalt`, `getRevealPicks`) : Veiled Chaos refait l'objet avec un
  mod voilé (côté au hasard parmi les côtés ouverts) puis les autres mods ; Veiled Exalted retire un mod au
  hasard puis pose un mod voilé. Au dévoilement, trois mods voilés de ce côté sont proposés (tirés au poids, sans
  remise, un par groupe, hors groupes déjà sur l'objet) ; le plan prend un mod voulu s'il y en a un.
- **Harvest** (lot 4, 21/11 ; `harvestReforge`, `harvestAugment`, coûts en Lifeforce relevés dans les données
  de l'émulateur PoE 1 3.30.0.1, `methods.harvest`) : **Reforge [étiquette]** refait un objet Rare avec d'abord un
  mod de l'étiquette (tiré au poids parmi les mods qui la portent), puis 4, 5 ou 6 mods au total ;
  **Augment [étiquette]** ajoute un mod de l'étiquette puis retire au hasard un des mods d'avant (hors côté
  verrouillé et mods fracturés). Seules les étiquettes portées par un mod voulu sont proposées. Prix : Wild,
  Vivid et Primal Crystallised Lifeforce (page Currency de poe.ninja). Écartés : « Reforge More/Less Likely »
  (dépend des mods exacts de l'objet), changement de résistance (l'état du solveur ne sait pas quels autres mods
  sont des résistances), changement d'influence et enchantements.

## Calcul

- **Magique : exact.** Formule fermée sur les poids ; un test la compare à l'énumération complète sur le moteur
  (écart < 10⁻⁹).
- **Rare : approché.** Chaque mod voulu est suivi exactement ; les autres mods sont résumés par côté (poids restant
  moyen après k tirages sans remise), comme le solveur PoE 2. Les tests le comparent à 200 000 objets simulés :
  écart de 2 à 3 % sur un anneau (vie + résistances, avec ou sans Essence).
- **Fossiles** : même calcul Rare sur les poids changés (testé contre la simulation). Seuls les fossiles qui, seuls,
  améliorent la chance du Chaos sont combinés (les 10 meilleurs, soit au plus 385 combinaisons) ; un fossile qui
  bloque un mod voulu n'en fait donc jamais partie. Une méthode par taille de résonateur, choisie aux prix envoyés
  par le panneau.
- **Coût** : fait par le panneau, à partir des prix poe.ninja PoE 1 (pages `Currency`, `Essence`, `Fossil` et
  `Resonator` de la ligue
  PoE 1 des réglages, en Chaos), ou du prix saisi par le joueur (gardé sur le poste). Le prix de la base n'est pas
  compté. Coût moyen = coût d'un essai / chance ; essais médian et 9 sur 10 par la loi géométrique.

- **Établi** : le voulu craftable n'est plus à tirer ; l'essai réussit si les autres voulus sortent, sans mauvais
  tier de son groupe et avec une place libre de son côté. Rare : même calcul que ci-dessus ; Magique : énumération
  exacte des tirages (l'Augmentation sert aussi quand elle laisse la place au craft). Testé contre la simulation
  (écart < 3 %). Le coût compte le craft même quand le mod sort tout seul : c'est un majorant léger.

## Plan conseillé : solveur Exalt / Annul / metamods (`poe1::forge::solveur`)

Choisi par Max le 14/11 (option « Spams + établi et metamods »). Au lieu de comparer des spams, le solveur cherche
la meilleure décision dans chaque situation et donne le coût moyen minimal pour finir l'objet.

- **Actions** : Transmutation, Alteration, Augmentation, Regal, Alchemy, Chaos, Essences, fossiles (depuis le
  21/11 : la meilleure combinaison par taille de Chaotic Resonator, celle des méthodes de spam, sur un objet
  Normal ou Rare sans metamod ; mods fracturés gardés), Exalted, Annulment,
  Scouring, metamods d'établi (« Prefixes / Suffixes Cannot Be Changed », « Cannot roll Attack / Caster
  Modifiers »), retrait du craft (1 Orb of Scouring), craft d'établi final. Un seul mod d'établi à la fois, sauf
  pour finir (lot 3, 21/11) : « Can have up to 3 Crafted Modifiers » (2 Divine Orbs, un suffixe) puis les deux
  voulus qui manquent, craftés à l'établi, s'il reste la place de chaque côté. Le multimod ne sert pas à garder
  un verrou d'établi pendant qu'on relance. Depuis le 21/11 (lot 2) : Eldritch Chaos, Exalted et Annulment quand le joueur indique la dominance
  (Eldritch Chaos seulement sans verrou d'établi et sans metamod du côté refait ; les filtres « Cannot roll … »
  restent appliqués), Veiled Chaos et Veiled Exalted quand un mod voulu est un mod voilé (dévoilement aussitôt).
  Lot 4 : Harvest Reforge et Augment par étiquette (objet Rare ; la reforge seulement sans verrou d'établi).
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
- **Approximations du dévoilement** : quand deux mods voulus sont proposés au même dévoilement, le plan prend
  le premier de la liste demandée ; les autres mods de l'objet n'écartent pas les groupes voilés qu'ils
  occupent (seuls les voulus le font) ; un mod voilé à deux groupes n'est rangé que dans le premier.
- **Validation** (tests `solveur::tests`) : Chaos seul retrouve la formule du spam ; Eldritch (Exarch et
  Eater) et mods voilés : la politique simulée sur le moteur exact retrouve le coût du solveur à moins de 5 % ; sur une Glorious Plate, la
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
| P1-03 | Les fossiles suivent la loi 4/5/6 mods du Chaos ; bonus additifs entre fossiles (mode par défaut de Craft of Exile) | recouper un cas avec l'émulateur en mode multiplicatif ; compter les mods sur des objets refaits aux fossiles |
| P1-04 | Sur un objet fracturé, Alteration, Chaos, Alchemy et Essence tirent le nombre de mods habituel moins les mods fracturés ; un Scouring laisse l'objet Magique (1 mod fracturé) ou Rare | 50 Chaos sur un objet fracturé, compter les mods ; un Scouring, noter la rareté |
| P1-05 | Eldritch Chaos garde les metamods « Cannot roll … » de l'autre côté et les applique ; Veiled Exalted respecte un verrou d'établi | essai sur l'émulateur, puis en jeu |
| P1-06 | Au dévoilement, les trois choix excluent les groupes de tous les mods de l'objet | 20 dévoilements sur un objet qui porte déjà un mod du groupe |
## Suite possible (sur décision de Max)

Awakener's Orb, multimod avec verrou, Harvest More/Less Likely et changement de résistance, mise à jour PoE 1 dans `mise_a_jour.py`.

## Dispersion du coût et budget (21/11, idée n°6 validée par Max)

Le coût moyen ne dit pas combien prévoir. Avec le plan, le solveur suit sa politique craft après craft sur son
propre graphe (`simuler_plan`, graine fixe : mêmes chiffres d'un calcul à l'autre) et renvoie `simulation` :
médiane, 9 sur 10, 99 sur 100 et les quantiles tous les 0,5 %. Le nombre de crafts simulés s'adapte à la
longueur moyenne d'un craft (2 000 à 20 000, environ 2·10⁷ actions au total), et un craft trop long est compté
comme un échec. La moyenne simulée retrouve le coût moyen du solveur (test à 3 %).

Le panneau en tire la même bande de chiffres qu'en PoE 2 (coût moyen, une fois sur deux, budget sûr 9 sur 10)
et « Avec mon budget » : le joueur saisit un montant dans la monnaie d'affichage et voit sa chance de finir
(même lecture prudente des quantiles qu'en PoE 2, `chanceQuantiles`). Montants hors prix de la base, comme le
coût moyen.

**Prix anciens** : la cote PoE 1 porte l'heure du relevé (`releveA`). Au-delà de 24 h (appli restée ouverte),
un avertissement s'affiche au-dessus du résultat avec « Actualiser », qui relit poe.ninja et relance le même
calcul. Aucune requête au site de trade. Même avertissement dans la Forge PoE 2 (relevé de l'onglet Prix).

## Pas à pas du plan conseillé

Le solveur renvoie, avec le plan, l'action conseillée dans chaque situation où le plan passe au moins une
fois sur mille crafts (`etapes` : rareté, mods voulus présents, mods voulus pris par un tier trop bas,
autres préfixes et suffixes, metamod, coût moyen restant). Le panneau en tire :

- des règles par rareté : l'action habituelle (« dans les autres cas »), puis les situations où le plan
  fait autre chose, les plus fréquentes d'abord ;
- « Où en est ton objet ? » : le joueur décrit son objet et voit l'action à faire et le coût moyen restant.
  Une situation absente des étapes est une situation où le plan ne passe presque jamais.


## Arbre de décision, étapes et liste de courses

Comme en PoE 2, le plan conseillé s'affiche en trois onglets. Le composant d'arbre, sa mise en page
(`arbre()`), le tri des étapes (`etapes()`) et les tableaux sont ceux de la Forge PoE 2 ; seuls changent le
format des montants et la description des cases (avec le metamod), passés en options au composant.

- Chaque étape renvoie ses issues : directes (Exalted, Annulment, établi…) dans `issues`, ou, quand l'action
  refait l'objet (Alchemy, Chaos, Essence, Alteration…), l'indice de sa loi dans `relances`, écrite une seule
  fois. Les issues vers des situations hors du pas à pas sont regroupées (« +N »).
- `arbrePoe1` (panneau) met le plan à la forme d'un plan PoE 2. Les situations qui font la même relance sont
  regroupées en une case (« Rare · 270 situations, surtout … ») : les issues entre elles deviennent la relance
  sur place (« ~N× »). Une flèche est verte quand il reste moins à dépenser après qu'avant.
- La liste de courses chiffre les monnaies consommées en moyenne aux prix utilisés par le calcul.

## Coûts affichés en Chaos ou Divine

En PoE 1, la barre latérale propose Chaos ou Divine (`devisePoe1.ts`, gardé à part du choix PoE 2). Les
calculs restent en Chaos ; le cours de la Divine vient des prix de la Forge (saisie, sinon poe.ninja) ou du
dernier relevé des coffres. Les prix unitaires saisis restent en Chaos.

## Jusqu'à 6 mods voulus

Le plan conseillé accepte 6 mods voulus (il s'arrêtait à 4). Au-delà de 4, la première politique (relance
à valeur fixe) pouvait tourner en rond sans jamais atteindre l'objectif, par exemple en enchaînant Exalted
et Annulment alors qu'un mod voulu ne vient que d'une Essence. `Graphe::reparer` rend cette politique
propre : un état d'où l'objectif n'est jamais atteint prend l'action la moins chère parmi celles qui y
mènent. L'itération de politique part ensuite d'une politique qui aboutit. Test : arc à 5 mods dont celui
de l'Essence of Insanity, comparé à la simulation.

## Objet de départ

Comme en PoE 2, le joueur colle le texte d'un objet (Ctrl+Alt+C) : la Forge lit sa base (entre bases
homonymes, celle dont l'implicite est sur l'objet), son niveau, son influence et ses mods (lecture du suivi
PoE 2), reconnaît un metamod d'établi, et le plan conseillé repart de l'état de cet objet
(`poe1::forge::depart`, `solveur::plan_depuis_objet`). Les autres mods artisanaux prennent une place sans
servir l'objectif. Les méthodes de spam partent toujours d'une base neuve. Le bouton « Réinitialiser » décoche les mods voulus, retire l'objet de départ et efface le
résultat.

### Objet fracturé

Les mods fracturés restent sur l'objet : le plan ne les retire jamais (Annulment, Scouring et relances les
gardent). Une relance tire le nombre de mods habituel moins les mods fracturés (Alteration : 1 ou 2 mods au
total ; Chaos, Alchemy, Essence : 4, 5 ou 6), comme le Chaos avec un côté verrouillé (hypothèse P1-04). Un
Scouring laisse un objet Magique (un mod fracturé) ou Rare (deux et plus). Une Essence dont le groupe est pris
par un mod fracturé n'est pas proposée. Un mod fracturé non reconnu, ou au mauvais tier d'un mod voulu,
est refusé avec un message (objectif impossible). Le calcul est recoupé par simulation sur le moteur exact
(`objet_fracture_garde_son_mod`).

## Suivi en jeu, historique et « crafter ou acheter »

Comme en PoE 2 (demande de Max du 15/11/2026, « toutes les fonctionnalités du 2 dans le 1 ») :

- **Historique** : chaque plan conseillé calculé avec des prix entre dans `forge-poe1-historique.json`
  (30 plans au plus ; une même demande recalculée remonte en tête). Le panneau peut rouvrir, suivre ou
  supprimer un plan.
- **Suivi par l'overlay** : le dernier plan calculé (ou celui choisi avec « Suivre ») est suivi. Ctrl+Alt+C
  sur un objet de la même base affiche les mods voulus présents et manquants, puis l'action conseillée,
  recalculée depuis l'objet tel qu'il est (`poe1::forge::suivi::conseiller`, qui s'appuie sur l'objet de
  départ). Un objet d'une autre base garde l'overlay habituel (cote et comparaison avec le build).
  Comme en PoE 2, les essais s'enchaînent dans le même panneau : « Ça a marché » ou « Raté, reste X »,
  nombre d'essais et dépense de la série (`forge::enchainement`, voir `cadrage.md`).
- **Crafter ou acheter** : la recherche de l'objet visé s'ouvre sur le site de trade de PoE 1 (même base,
  influence, chaque mod voulu au tier visé ou mieux, du moins cher au plus cher). Le joueur reporte le prix
  vu ; Forge le compare au coût moyen du plan (hors prix de la base), avec la même marge « équivalent »
  de 10 % qu'en PoE 2. Les identifiants des stats viennent de `data/trade-stats-poe1.json`, généré par
  `tools/gamedata/trade_stats_poe1.py` depuis `TradeSiteStats.lua` de Path of Building (MIT). Une ligne
  sans stat de trade est signalée « non filtrée ».
