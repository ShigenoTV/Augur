# Archétype du Sprint 1 : sort élémentaire à coups

**Statut :** proposition, alignée avec le thread moteur de calcul. À valider par Max et l'équipe (le plan prévoit un choix conjoint Theorycrafter + devs back en S1).

## Choix

Un **sort élémentaire qui inflige des coups** (hits), sans dégâts dans le temps, lancé par un personnage dont la défense repose sur la vie, l'Energy Shield, l'armure et l'évasion.

Le choix exact de la gemme se fait **en jeu**, sur le patch courant, avec un critère simple : un sort à un seul coup par incantation, sans projectiles multiples ni rebonds, pour que « dégâts par coup × cadence » soit une définition non ambiguë du DPS.

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
| Niveaux de gemmes | **Bloqué** : demande l'extraction des `.datc64` |
