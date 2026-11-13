# Données embarquées par `poe2-core`

Générées par des scripts de `tools/gamedata/`, jamais modifiées à la main.

| Fichier | Contenu | Source |
|---|---|---|
| `gemmes.json` | Chaque sort à dégâts par niveau (dégâts par type, chaînes, temps d'incantation, critique, tags d'élément ; infusion d'Arc) ; Lightning Bolt et Arc marqués vérifiés en jeu ; chaque attaque (efficacité par niveau, vitesse, conversion) ; dégâts dans le temps par seconde ; effets propres de la gemme sur les ailments (chance de poison ou de saignement, Flammability, magnitude, durée) ; gemmes de minions (dégâts d'un monstre allié de niveau 2 × niveau de gemme, table de Path of Building PoE 2 `src/Data/Misc.lua`, licence MIT) ; bases d'armes (dégâts, temps d'attaque, critique) ; texte des effets de chaque support | RePoE PoE 2, `skills.json` et `base_items.json` (https://repoe-fork.github.io/poe2/) |
| `forge.json.gz` | Forge (`poe2-core::forge::atelier`) : mods d'objets tirables et mods garantis par les Essences, bases, relevé de poids de mods poe2db complété par Craft of Exile, et mod garanti de chaque Essence par classe (Craft of Exile) | RePoE PoE 2 (`mods.json`, `base_items.json`) ; **poids : PoE2DB / Krakenbul, CC BY-NC-SA 3.0**, complément Craft of Exile (voir ci-dessous) |
| `trade-stats.json` | « Crafter ou acheter » de Forge (`poe2-core::forge::achat`) : identifiants de stats du site de trade (`explicit.stat_…`) de chaque ligne de mod de `forge.json.gz` | Exiled Exchange 2, `renderer/public/data/en/stats.ndjson` (https://github.com/Kvan7/Exiled-Exchange-2), licence MIT |
| `arbre.json` | Texte des stats de chaque passif, version d'arbre `0_5` | Path of Building Community PoE 2, `src/TreeData/0_5/tree.json` |

Régénérer après un patch du jeu :

```sh
curl -sSO https://repoe-fork.github.io/poe2/skills.json
curl -sSO https://repoe-fork.github.io/poe2/base_items.json
curl -sS -o tree.json https://raw.githubusercontent.com/PathOfBuildingCommunity/PathOfBuilding-PoE2/dev/src/TreeData/0_5/tree.json
python tools/gamedata/extract_build_data.py --skills skills.json --bases base_items.json --tree tree.json \
    --tree-version 0_5 --out crates/poe2-core/data
```

Régénérer `forge.json.gz` (tables produites par `tools/gamedata/import_repoe.py`) :

```sh
python tools/gamedata/forge_data.py --tables var/gamedata/tables \
    --poids tools/theorycraft/donnees/poids-poe2db.json \
    --complement tools/theorycraft/donnees/poids-coe.json \
    --essences tools/theorycraft/donnees/essences-coe.json --out crates/poe2-core/data/forge.json.gz
```

Régénérer `trade-stats.json` (après `forge.json.gz`) :

```sh
curl -sSLO https://raw.githubusercontent.com/Kvan7/Exiled-Exchange-2/master/renderer/public/data/en/stats.ndjson
python tools/gamedata/trade_stats.py --forge crates/poe2-core/data/forge.json.gz \
    --stats stats.ndjson --out crates/poe2-core/data/trade-stats.json
```

Un test Rust (`forge::achat`) échoue si une ligne de mod de Forge n'est ni reliée ni listée dans `sans_stat`.

## Attribution

Les poids de mods de `forge.json.gz` ne sont **pas** sous la licence du reste du dépôt : ce sont les
estimations de **Krakenbul** (Discord *Prohibited Library*), publiées par **PoE2DB**
(https://poe2db.tw/us/weightings), sous licence Creative Commons BY-NC-SA 3.0
(https://creativecommons.org/licenses/by-nc-sa/3.0/). Pas d'utilisation commerciale ; une version modifiée se
partage sous la même licence. Détail et modifications :
[`tools/theorycraft/donnees/LICENCE-poids-poe2db.md`](../../../tools/theorycraft/donnees/LICENCE-poids-poe2db.md).
L'appli n'affiche pas les sources (choix de Max, 08/11/2026) : l'attribution est dans le dépôt.

Les poids des mods que PoE2DB n'a pas mesurés viennent de **Craft of Exile**
(https://www.craftofexile.com/?game=poe2, mêmes estimations de Krakenbul), données tierces copiées avec leur
attribution : [`tools/theorycraft/donnees/LICENCE-poids-coe.md`](../../../tools/theorycraft/donnees/LICENCE-poids-coe.md).
La liste des Essences et de leur mod garanti par classe vient de la même source (même fichier d'attribution).

Les identifiants de stats du site de trade de `trade-stats.json` viennent du projet Exiled Exchange 2
(© Alexander Drozdov et contributeurs), distribué sous licence MIT.

Les données de l'arbre passif viennent du projet Path of Building Community
(PoE 2), distribué sous licence MIT (texte complet ci-dessous) :

```
Copyright (c) 2016 David Gowor

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Les textes des passifs et des gemmes sont ceux du jeu Path of Exile 2, propriété de
Grinding Gear Games.
