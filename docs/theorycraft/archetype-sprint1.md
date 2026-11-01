# Archétype du Sprint 1 : sort élémentaire à coups

**Statut :** sort choisi (Lightning Bolt) d'après les données RePoE du patch 0.5.5.5, à recouper avec le tooltip en jeu (mesure 4 de la fiche de Max).

## Choix

Un **sort élémentaire qui inflige des coups** (hits), sans dégâts dans le temps, lancé par un personnage dont la défense repose sur la vie, l'Energy Shield, l'armure et l'évasion.

Critère : un sort à un seul coup par incantation, sans projectiles multiples ni rebonds, pour que « dégâts par coup × cadence » soit une définition non ambiguë du DPS.

## Sort retenu : Lightning Bolt

Source : RePoE, export du patch 0.5.5.5 (`skills.json`, compétence `LightningBoltPlayer`), dépôt https://github.com/repoe-fork/poe2.

- Temps d'incantation de base : 0,85 s (`cast_time` 850 ms).
- Chance de critique de base : 10 % (`crit_chance` 1000, en centièmes de %).
- Un seul coup de foudre par incantation, dans une petite zone (rayon 0,8 m) ; pas de projectile, pas de rebond, pas de mécanique de temps d'incantation ajouté.
- Le choc qu'il applique est un ailment : hors périmètre, affiché « non supporté ».

| Niveau de gemme | Dégâts de foudre |
|---|---|
| 1 | 3 à 13 |
| 5 | 4 à 48 |
| 10 | 7 à 137 |
| 15 | 15 à 292 |
| 19 | 26 à 496 |
| 20 | 30 à 565 |
| 21 | 34 à 642 |
| 25 | 56 à 1070 |
| 30 | 107 à 2024 |

Écarté : Comet (1 s d'incantation plus « +1 seconde au temps d'incantation total », infusion de feu, critique 13 %), qui ajoute deux mécaniques à vérifier.

Cas de test : [`tests/golden/archetype/lightning-bolt.yaml`](../../tests/golden/archetype/lightning-bolt.yaml), six cas au format du moteur (nominal, +1 niveau, équipement, pénétration, résistance négative, bonus critique). Les chiffres de gemme sont confirmés en jeu (Arc, I-13) ; la confiance des cas reste « supposé » parce que le tooltip de la gemme n'affiche aucun modificateur : il ne permet pas de mesurer un DPS.

**Note du 1er novembre 2026 :** le personnage de Max joue Arc (niveau 32 de gemme avec l'équipement), un sort à chaînes, donc hors du critère « un seul coup ». Le tooltip d'Arc a servi à vérifier la table RePoE (niveaux 22 et 32, tous deux exacts). Arc est un candidat pour le deuxième archétype : proposition à valider par Max.

## Pourquoi un sort plutôt qu'une attaque à l'arc

- Moins de mécaniques à vérifier pour un résultat « entièrement vérifié » : pas de précision ni de chance de toucher côté attaquant, pas de modificateurs locaux d'arme, pas d'efficacité des dégâts ajoutés.
- Les dégâts de base viennent de la gemme (table par niveau extraite des `.datc64`), ce qui rend les tooltips du jeu directement comparables à nos calculs.
- Les objets qui comptent (bâtons, baguettes, anneaux, amulettes, casques) portent peu de types de stats : `+niveaux des sorts`, `% dégâts de sort/élémentaires`, vitesse d'incantation, critique, résistances. La table de correspondance des stats reste petite.

L'arc reste le meilleur candidat pour le **deuxième** archétype : c'est là que le delta DPS d'une arme est le plus demandé.

## Périmètre

| Dans le périmètre (doit être « vérifié ») | Hors périmètre (affiché « non supporté ») |
|---|---|
| Dégâts de base de la gemme par niveau, `+niveaux` | Ailments (ignite, shock, chill, freeze, electrocute) et leur buildup |
| Ordre des opérations des dégâts, conversion, gain en dégâts supplémentaires | Dégâts dans le temps, poison, saignement |
| Coup critique | Projectiles multiples, chaînes, rebonds, zones qui se chevauchent |
| Vitesse d'incantation | Minions, totems, déclencheurs (triggers), méta-gemmes |
| Résistance et pénétration de la cible, exposition | Malédictions autres que celles qui réduisent les résistances |
| Défense : résistances, armure, évasion, déflexion, ES, EHP | Blocage, Mind over Matter, Guard, Sorcery Ward |

## Fiches nécessaires

| Fiche | Usage |
|---|---|
| [01 Ordre des opérations des dégâts](fiches/01-ordre-des-operations-degats.md) | DPS |
| [02 Coup critique](fiches/02-coup-critique.md) | DPS |
| [03 Vitesse d'incantation](fiches/03-vitesse-incantation.md) | DPS |
| [04 Résistances et pénétration](fiches/04-resistances-penetration.md) | DPS (cible) et défense |
| [05 Armure](fiches/05-armure.md) | Défense |
| [06 Évasion et déflexion](fiches/06-evasion-deflexion.md) | Défense |
| [07 Energy Shield](fiches/07-energy-shield.md) | Défense |
| [08 EHP](fiches/08-ehp.md) | Défense (indicateur produit) |
| Niveaux de gemmes | Débloqué via RePoE (Lightning Bolt ci-dessus) |
