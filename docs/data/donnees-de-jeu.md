# Données de jeu : mods et bases d'objets

Tables extraites de l'export communautaire **RePoE** (https://repoe-fork.github.io/poe2/),
qui lit les fichiers du client. Elles alimentent le parseur (reconnaître un mod et ses
`stat_id`), le pricing et, plus tard, Forge (tirages pondérés). Elles sont publiées dans
un paquet de données signé (voir [paquets-de-donnees.md](../devops/paquets-de-donnees.md)).

Outil : [`tools/gamedata/import_repoe.py`](../../tools/gamedata/import_repoe.py), repris
et adapté de l'importeur de poe2-craft-assistant (même auteur, 31/10/2026).

## Produire les tables

```bash
# 1. Télécharger depuis un navigateur ou une machine avec accès internet :
#    https://repoe-fork.github.io/poe2/mods.min.json
#    https://repoe-fork.github.io/poe2/base_items.min.json
# 2. Noter la version affichée dans le titre de la page (« RePoE - PoE2 version X.Y.Z.W »).
python tools/gamedata/import_repoe.py --mods mods.min.json --bases base_items.min.json \
    --out var/gamedata --repoe-version X.Y.Z.W
# 3. Paquet signé :
python tools/datapack/augur_datapack.py build --src var/gamedata --out out --patch 0.4.1
```

La sortie est déterministe (JSON canonique, sans date) : un patch qui ne touche pas aux
mods redonne le même blob, que l'application ne retélécharge pas.

## Tables

`tables/mods.json` : `{"schema": 1, "mods": [...]}`, un mod par **palier**, trié par `id`.

| Champ | Sens |
|---|---|
| `id` | identifiant du jeu (`IncreasedLife9`) |
| `nom` | nom du palier, celui de l'en-tête Ctrl+Alt+C (`Athlete's`) |
| `domaine` | `item` (équipement), `misc` (joyaux), `desecrated` (mods désacralisés) |
| `slot` | `prefix` ou `suffix` |
| `groupe`, `groupes` | groupe d'exclusion : deux mods du même groupe ne coexistent jamais |
| `type` | famille de paliers dans le groupe |
| `niveau` | niveau d'objet requis |
| `texte` | texte affiché avec plages, balises retirées (`+(120-149) to maximum Life`) |
| `stats` | `[{id, min, max}]` : clé vers les `stat_id` du moteur |
| `tags` | tags du mod (`life`, `fire`…) |
| `spawn` | poids par tag de base, **dans l'ordre** : le premier tag porté par la base fixe le poids |
| `essence_seulement` | mod obtenu uniquement par Essence |
| `tirable` | peut sortir d'un tirage classique (poids > 0 hors `default`, pas Essence) |

`tables/bases.json` : `{"schema": 1, "bases": [...]}`, une entrée par vraie base
d'équipement publiée (`id` = chemin Metadata, `nom`, `classe`, `tags`, `niveau_drop`,
`implicites` = ids de mods, `exigences`).

`tables/source.json` : provenance, version RePoE et comptages.

## Limites connues

- **Poids de spawn** : RePoE donne les poids du client. poe2-craft-assistant notait que le
  client met souvent un poids de 1 à tous les paliers ; les vrais poids sont estimés par la
  communauté (Craft of Exile / Prohibited Library). Ces estimations sont des données d'un
  tiers : **non reprises** ici, à ne pas redistribuer sans accord. Sujet à trancher avant Forge.
- Les Talismans et Pièges, ajoutés à la main dans poe2-craft-assistant, ne sont pas dans
  la liste des classes tant que leur présence dans RePoE n'est pas vérifiée.
- Les mods d'objets uniques, de corruption et les implicites ne sont pas encore extraits.
- Le domaine `desecrated` et les champs utilisés sont ceux observés par poe2-craft-assistant ;
  l'outil échoue avec un message clair si le format RePoE change.
- Pas encore de données de gemmes (`skills.json` de RePoE est la source candidate).
