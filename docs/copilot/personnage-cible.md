# Personnage cible : « où j'en suis, où je vais »

Dans Copilot, à côté du personnage actuel (celui que l'overlay compare), le joueur
peut importer un **personnage cible** : le build qu'il vise, le sien plus tard ou
celui d'un guide. L'import est le même (code d'export Path of Building collé à la
main, rien n'est téléchargé), un par jeu (PoE 1 et PoE 2).

La cible sert à la carte **Écart actuel → cible**, à la ligne « vers la cible » de
l'overlay et, si le joueur le choisit, à Alt+U. Le delta de DPS et d'EHP de l'overlay
compare toujours au personnage actuel.

## Ce que montre la carte Écart

Code : `crates/poe2-core/src/cible.rs` (détail), `apps/desktop/src-tauri/src/gap.rs`
(chiffres et commande `get_gap`), `apps/desktop/src/index/GapCard.tsx` (affichage).

1. **Chiffres** côte à côte : DPS, EHP, vie, bouclier d'énergie, résistances, et
   l'écart (en % pour DPS, EHP, vie et ES ; en points pour les résistances).
   - PoE 2 : calculés par le moteur d'Augur, exactement comme l'overlay
     (`comparaison::chiffres`), défenses étalonnées sur les stats de PoB quand
     l'export les contient. Les deux builds passent par le même calcul, donc
     l'écart tient même quand le DPS est « approximatif ».
   - PoE 1 : lus dans les stats exportées par Path of Building avec chaque code
     (`<PlayerStat>`). DPS = Full DPS, sinon DPS combiné, sinon DPS total. EHP =
     moyenne des cinq « Maximum Hit Taken » s'ils sont exportés (même définition
     que l'overlay), sinon « — ».
   - Si les deux builds n'utilisent pas la même compétence, la carte le dit : les
     DPS ne se comparent pas vraiment.
2. **Équipement**, emplacement par emplacement, les plus en retard d'abord :
   - unique de la cible : atteint si l'objet actuel est le même unique (les jets
     ne sont pas comparés), sinon « unique à obtenir » ;
   - rare ou magique : chaque mod de l'objet cible (implicites compris) est cherché
     sur l'objet actuel par son modèle (les nombres remplacés par `#`, « +# » et
     « # » confondus). Absent : **manquant**. Présent mais sous 90 % de la valeur
     de la cible : **plus faible**. Sinon atteint ;
   - les balises de PoB sont lues : `{range:…}` résout les plages `(a-b)`,
     `{variant:…}` garde seulement la variante choisie de l'unique ;
   - les deux anneaux sont interchangeables : l'appariement qui laisse le moins à
     faire est retenu ;
   - les sets d'armes de rechange (« Swap ») sont ignorés.
3. **Gemmes** : compétences de la cible absentes du build actuel, et pour chaque
   compétence présente des deux côtés, les supports de la cible qui manquent.
4. **Passifs** : nombre de passifs à prendre et à retirer (différence des deux
   arbres). Si la classe ou l'ascendance diffère, la carte prévient que l'arbre ne
   se compare pas vraiment.

## Stockage

`personnage-cible.json` (PoE 2) et `personnage-poe1-cible.json` (PoE 1), dans le
dossier de données d'Augur, au même format que `personnage.json` : le code collé et
sa date d'import. Rien d'autre n'est gardé.

## Dans l'overlay : « vers la cible »

Quand un personnage cible est importé, Ctrl+C sur un objet d'équipement ajoute sous
l'overlay (PoE 1 et PoE 2) une ligne qui compare l'objet copié à l'objet de la cible
pour son emplacement : « Vers la cible (casque) : 3 mods sur 5 · ton objet : 1 sur 5 »,
puis les mods qui manquent encore (trois au plus). Mêmes règles que la carte Écart
(modèle du mod, 90 % de la valeur). Pour un unique de la cible : « C'est l'unique de ta
cible » ou « Ta cible porte … ». Pour les anneaux, l'emplacement où l'objet est le plus
proche de la cible. Rien n'est affiché sans cible, sans objet cible à cet emplacement,
ou pour les flacons, charmes et joyaux.

Code : `cible::progression`, commande `get_target_progress` (lit le dernier objet copié),
`apps/desktop/src/overlay/TargetLine.tsx`.

## Alt+U « vers la cible »

Réglages > **Recherche d'upgrades (Alt+U)** : « Mieux que mon objet » (la recherche
pondérée d'avant) ou « L'objet de ma cible ». Le lien « Chercher l'objet de la cible »
de l'overlay fait la même recherche quel que soit le réglage.

La recherche de l'objet cible (`forge::achat::recherche_objet_cible`, commande
`open_target_search`) :

- unique : son nom et sa base, rareté unique ;
- rare ou magique : même base, chaque mod explicite de la cible (sans implicites,
  enchantements ni runes) à au moins **80 %** de sa valeur, objet non corrompu ;
- les identifiants de stats viennent des mêmes tables que « Crafter ou acheter » de la
  Forge (`data/trade-stats.json` en PoE 2, `data/trade-stats-poe1.json` en PoE 1). Un mod
  absent de la table n'est pas cherché, et l'overlay dit combien ;
- budget et ligue : ceux de la recherche d'upgrades du jeu (exalts en PoE 2, chaos ou
  divines en PoE 1) ;
- comme Alt+U, Augur n'envoie aucune requête au site de trade : il ouvre l'adresse dans
  le navigateur. Si la cible n'a pas d'objet pour l'emplacement copié, Alt+U fait la
  recherche « mieux que mon objet ».
