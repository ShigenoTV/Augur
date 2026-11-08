# Poids de mods Craft of Exile : attribution

Le fichier [`poids-coe.json`](poids-coe.json) **n'est pas sous la licence du reste du dépôt** : ce sont des
données tierces, copiées avec leur attribution sur décision de Max (08/11/2026).

- **Œuvre d'origine** : poids de génération des mods d'objets de Path of Exile 2, tels que les publie
  [Craft of Exile](https://www.craftofexile.com/?game=poe2) dans le fichier de données de son émulateur
  (`json/poe2/0.5.5.5/data.json`, champ `classmods`).
- **Auteurs** : poids estimés par **Krakenbul** (Discord *Prohibited Library*) ; compilés et publiés par
  **Craft of Exile**. Ces poids ne viennent pas des fichiers du jeu.
- **Ce qui est repris** : uniquement les poids des mods **que PoE2DB n'a pas mesurés** (PoE2DB reste la source
  principale, voir [`LICENCE-poids-poe2db.md`](LICENCE-poids-poe2db.md)). Les 6 434 poids connus des deux
  sources sont identiques. Les poids « 1 » (non mesurés) sont écartés. Chaque mod est nommé par son identifiant
  RePoE, chaque classe d'objet par le nom de page PoE2DB correspondant (`tools/theorycraft/coe_poids.py`).
- **Relevé** : 08/11/2026, données Craft of Exile 0.5.5.5. 3 347 poids sur 51 pages.
- **Contrôle** : `tools/theorycraft/coe_controle.py` compare chaque pool de Forge à Craft of Exile ; au 08/11/2026,
  1 714 bases × 4 niveaux d'objet, 6 856 pools identiques.

Ces données ne sont pas utilisées commercialement (Augur n'est pas vendu). Toute redistribution garde cette
attribution.

Path of Exile est une marque de Grinding Gear Games ; Augur n'est ni affilié ni approuvé par GGG.
