# PoE 1 : « cet objet est-il mieux que le mien ? » avec Path of Building

Augur n'a pas de moteur de calcul PoE 1 écrit à la main. En mode PoE 1, il fait
tourner **Path of Building Community (PoE 1)** lui-même, sans fenêtre, dans
l'interpréteur LuaJIT embarqué (crate `crates/pob-poe1`, bibliothèque `mlua`).

## Ce que fait l'overlay

Ctrl+C sur un objet d'équipement en jeu :

1. Augur passe le texte copié à PoB, qui le lit comme un objet collé dans son onglet Items.
2. PoB recalcule le build importé avec cet objet à la place de celui porté
   (`calcsTab:GetMiscCalculator()`, le même calcul que son infobulle de comparaison).
   Les anneaux sont essayés aux deux emplacements, les armes à une main dans les deux mains.
3. L'overlay affiche le verdict, le delta de DPS (Full DPS si le build l'utilise, sinon
   le DPS combiné de la compétence principale), l'EHP, les résistances qui bougent, puis
   les lignes de PoB qui changent (vie, mana, attributs…).

Les jewels et les flasques ne sont pas encore comparés. Les devises, gemmes et cartes
gardent seulement leur cote poe.ninja.

## Performances

Mesuré sur Linux (build de test Berserker, PoB 2.67.2+303c104) :

| Étape | Durée | Quand |
|---|---|---|
| Démarrage de PoB | ≈ 4 s | une fois, en tâche de fond (démarrage en mode PoE 1, import du build, passage en PoE 1) |
| Chargement du build | ≈ 1 s | à l'import, ou au premier objet si le build a changé |
| Comparaison d'un objet | 25 à 40 ms | à chaque Ctrl+C |

Mémoire : environ 360 Mo tant que PoB est démarré (seulement en mode PoE 1 avec un build importé).

## Le paquet Path of Building

`crates/pob-poe1/paquet/pob-poe1.tar.gz` (≈ 5 Mo) est préparé par
`tools/pob/paquet_pob_poe1.py` à partir d'une version fixée de PoB (`VERSION`). Il est
embarqué dans l'appli et décompressé au premier lancement dans le dossier de données
locales d'Augur (`pob-poe1/<empreinte>/`). Il contient le code Lua, les données du jeu et
seulement le dernier arbre passif. Un build sur un arbre plus ancien est recalé sur le
dernier, comme le fait la conversion de PoB. Les tables des Timeless Jewels (170 Mo) ne
sont pas embarquées.

Mettre PoB à jour (nouvelle ligue) : `python tools/pob/paquet_pob_poe1.py --ref <tag ou commit>`,
puis `cargo test -p pob-poe1`.

PoB ne fait aucune requête réseau depuis Augur (`lcurl` absent, `os.execute` et
`io.popen` coupés). Il écrit seulement dans son dossier de cache.

## Licence et attribution

Path of Building Community est sous licence MIT (© Path of Building Community,
https://github.com/PathOfBuildingCommunity/PathOfBuilding). Le texte de la licence est
livré dans le paquet (`LICENSE.md`). Comme pour les autres sources d'Augur, rien n'est
affiché dans l'interface.

Le build de test `crates/pob-poe1/tests/donnees/berserker-pob.xml` vient des tests de
PoB (`spec/TestBuilds/3.13/Generals Perforate Zerker.xml`, MIT).
