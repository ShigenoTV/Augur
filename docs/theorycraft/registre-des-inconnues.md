# Registre des inconnues

Feuille de route des tests en jeu. Priorité : **P0** bloque l'exactitude de l'archétype du Sprint 1 ; **P1** fausse un indicateur affiché ; **P2** cas rares ou hors périmètre.

Personne ne peut fermer une entrée sans preuve (capture ou vidéo référencée dans un cas golden, voir le protocole de la section 3.4 du plan).

| Id | Prio | Mécanique | Question | Comment la trancher | Bloqué par |
|---|---|---|---|---|---|
| I-01 | P0 | ES (fiche 07) | Le chaos retire-t-il toujours 2× l'ES sur le patch courant ? | Personnage avec ES, recevoir un coup de chaos de taille connue, comparer ES perdue et dégâts | Jeu installé |
| I-02 | P0 | Résistances (fiche 04) | Pénalité de campagne réelle : toujours −10 % par acte, −60 % au total ? La campagne a changé entre les patchs | Relever la résistance du panneau avant/après chaque acte, ou objets connus + panneau en fin de campagne | Jeu installé |
| I-03 | P0 | Dégâts (fiche 01) | Les dégâts convertis ou gagnés profitent-ils uniquement des modificateurs du type final ? | Cas `degats-003` : tooltip d'un sort avec conversion, avec et sans un objet « increased <type source> » | Jeu installé |
| I-04 | P0 | Évasion (fiche 06) | La formule d'évasion (source de septembre 2025) est-elle toujours valable ? | Chance d'éviter du panneau pour 3 valeurs d'évasion, ajustement des constantes 1,25 et 0,3 | Jeu installé |
| I-05 | P0 | Armure (fiche 05) | Constante 10 dans A/(A+10D) | Réduction estimée affichée par le panneau, ou coup de taille connue | Jeu installé |
| I-06 | P0 | Évasion (fiche 06) | Précision des monstres par niveau | Table extraite des données du jeu | Extraction `.datc64` |
| I-07 | P0 | Dégâts (fiche 01) | Arrondis du minimum et du maximum affichés dans le tooltip | Comparer nos valeurs non arrondies au tooltip sur 10 configurations | Jeu installé |
| I-13 | P0 | Gemmes | Dégâts de base, chance de critique et temps d'incantation de la gemme choisie, par niveau | Extraction des tables de gemmes, recoupement avec le tooltip | Extraction `.datc64` |
| I-08 | P1 | Critique (fiche 02) | « +X% chance » avant les increased ? « +X% bonus » additif avec « increased bonus » ? | Tooltip de chance de critique avec un objet de chaque type | Jeu installé |
| I-09 | P1 | Vitesse (fiche 03) | Le temps d'incantation est-il arrondi au tick serveur (~33 ms) ? | Temps d'incantation affiché pour 5 valeurs de vitesse ; mesure vidéo à 60 i/s si l'affichage ne tranche pas | Jeu installé |
| I-11 | P1 | Résistances (fiche 04) | Résistances des boss et des monstres de carte | Données extraites ; à défaut, documentation de GGG | Extraction `.datc64` |
| I-12 | P1 | ES (fiche 07) | Qualité additive aux increased locaux ? Arrondi par objet ? | ES affichée d'un objet à plusieurs qualités | Jeu installé |
| I-10 | P2 | Résistances (fiche 04) | Résistance minimale (−200 % en PoE 1) | Cumul de malus de résistance en jeu | Jeu installé |
| I-14 | P2 | Dégâts (fiche 01) | Base du gain en dégâts supplémentaires quand une conversion existe dans la même étape | Cas `degats-007` | Jeu installé |
| I-15 | P2 | Dégâts (fiche 01) | Plancher à 0 quand les increased sont inférieurs à −100 % | Cas `degats-002` | Jeu installé |

## Autre point bloquant

Le **numéro du patch courant** n'est pas connu sans le client : tous les cas ont `patch: null` en attendant.
