# Registre des inconnues

Feuille de route des tests en jeu. Priorité : **P0** bloque l'exactitude de l'archétype du Sprint 1 ; **P1** fausse un indicateur affiché ; **P2** cas rares ou hors périmètre.

Personne ne peut fermer une entrée sans preuve (capture ou vidéo référencée dans un cas golden, voir le protocole de la section 3.4 du plan).

| Id | Prio | Mécanique | Question | Comment la trancher | Bloqué par |
|---|---|---|---|---|---|
| I-02 | P0 | Résistances (fiche 04) | **Données de Max reçues le 1er novembre 2026** (carte niveau 66, export Path of Building décodé). Sources connues pour le feu : objets 105 (l'anneau « +14 % à toutes les résistances » vaut 16 avec la qualité), arbre +5 (All Natural), quêtes +10 ; total 120, affiché 70, soit un net de **−50** identique pour feu, froid et foudre. Le tableau du wiki annonce −60 à ce niveau de zone : écart de 10, qui s'expliquerait par une source cachée (Mageblood, flacons, charmes) ; le chaos montre déjà +45 sans source connue. **Ceinture Mageblood retirée (1er novembre 2026)** : le chaos tombe de 89 à 44, exactement la somme des deux anneaux (17 + 27), donc la ceinture donnait +45 de chaos, le chaos n'a aucune pénalité et il n'a pas d'autre source ; en revanche feu et froid ne bougent pas (70 et 77), donc la ceinture n'explique pas l'écart de 10 sur les éléments, qui reste ouvert (net de −50, ou −60 avec +10 caché). Pénalité de progression : le wiki donne −10/−20/−30 % aux actes 2 à 4, puis −40/−50/−60 % selon le niveau de zone (54, 60, 65+). Reste à confirmer −60 % en endgame sur le patch courant | Objets connus + panneau dans une zone de niveau 65+ (mesure 2) | Mesure de Max |
| I-06 | P0 | Évasion (fiche 06) | Précision des monstres par niveau | Table `accuracy` de `default_monster_stats.json` dans RePoE (ex. niveau 65 : 1114, niveau 80 : 1964) ; à recouper avec la chance d'éviter du panneau (mesure 3) | **Débloqué via RePoE** (voir plus bas) |
| I-09 | P1 | Vitesse (fiche 03) | Le temps d'incantation est-il arrondi au tick serveur (~33 ms) ? **Partiel (1er novembre 2026)** : le DPS du panneau d'Arc utilise une cadence de 1,55/s, sans arrondi au tick, alors que le temps d'incantation s'affiche 0,64 s (1/1,55 = 0,645, donc affichage tronqué). Reste : la cadence réelle en jeu. | Temps d'incantation affiché pour 5 valeurs de vitesse ; mesure vidéo à 60 i/s si l'affichage ne tranche pas | Jeu installé |
| I-11 | P1 | Résistances (fiche 04) | Résistances des boss et des monstres de carte | Données extraites ; à défaut, documentation de GGG | Extraction `.datc64` (absent de `default_monster_stats.json` de RePoE) |
| I-10 | P2 | Résistances (fiche 04) | Résistance minimale (−200 % en PoE 1) | Cumul de malus de résistance en jeu | Jeu installé |
| I-15 | P2 | Dégâts (fiche 01) | Plancher à 0 quand les increased sont inférieurs à −100 % | Cas `degats-002` | Jeu installé |

## Inconnues fermées

Fermées par la documentation (sources concordantes). Les fiches restent au niveau `supposé` tant qu'une mesure en jeu ne les a pas recoupées, sauf I-01 (texte du jeu), I-07, I-12 et I-13 (mesurés).

| Id | Mécanique | Réponse | Source |
| --- | --- | --- | --- |
| I-03 | Dégâts (fiche 01) | Les dégâts convertis ou gagnés ne profitent que des modificateurs de leur type final | poe2wiki *Damage conversion* + mobalytics |
| I-04 | Évasion (fiche 06) | Chance de toucher = 1,25 × P / (P + 0,3 × E), plafonds 5 % / 95 % | poe2wiki *Evasion* + mobalytics ; recoupé par la mesure 3 |
| I-05 | Armure (fiche 05) | Constante 10 : A / (A + 10 × D), plafond 90 %, plancher de 1 dégât | poe2wiki *Armour* + mobalytics |
| I-08 | Critique (fiche 02) | Chance de base additionnelle avant increased/more ; bonus = (100 + plats) puis increased/more | poe2wiki *Critical hit* |
| I-14 | Dégâts (fiche 01) | Gains calculés sur la base d'avant l'étape, puis convertis par les conversions de la même étape | poe2wiki *Damage conversion* (exemple, cas `degats-011`) |
| I-01 | ES (fiche 07) | Le chaos retire bien 2× l'ES | Texte du mot-clé du jeu, via [poe2db](https://poe2db.tw/us/Energy_Shield), 31 octobre 2026 |
| I-13 | Gemmes | Les tables de gemmes de RePoE 0.5.5.5 sont exactes pour le client 0.5.5e : Arc niveau 22 = 26 à 499, niveau 32 = 94 à 1791, incantation 1,10 s, critique 9 % | **Mesuré** sur le tooltip de Max, 1er novembre 2026 (2 captures) |
| I-07 | Dégâts (fiche 01) | Le tooltip de la gemme affiche les dégâts **de base du niveau**, entiers, **sans aucun modificateur** (ni increased, ni vitesse, ni critique) ; il n'y a donc pas d'arrondi de dégâts modifiés à recouper à cet endroit | **Mesuré**, mêmes captures |
| I-12 | ES (fiche 07) | La qualité multiplie l'ES locale (elle ne s'ajoute pas aux increased locaux) ; affichage arrondi au plus proche | **Mesuré** sur 4 objets de Max le 1er novembre 2026 (client 0.5.5e). Reste : arrondi du total du personnage |

## Source de données en attendant notre propre extraction

[RePoE (fork PoE 2)](https://repoe-fork.github.io/poe2/) publie les tables du jeu en JSON, **patch 0.5.5.5** au 31 octobre 2026 : `skills`, `skill_gems`, `mods`, `stats_by_file`, `stat_translations`, `base_items`, `default_monster_stats`, `world_areas`, etc. C'est une extraction communautaire des fichiers du jeu, pas une donnée de GGG.

- Utilisable tout de suite pour les tables (niveaux de gemmes, précision et armure des monstres par niveau, mods), en indiquant `RePoE 0.5.5.5` comme source dans les cas.
- Ne remplace pas les mesures en jeu : RePoE donne des **données**, pas les **formules** qui les combinent.
- Ne contient pas les résistances des monstres (I-11).
- Le pipeline d'extraction `.datc64` du plan reste la cible pour la production ; RePoE sert de référence croisée pour le valider.

## Autre point bloquant

Le client de Max affiche **0.5.5e** (1er novembre 2026), l'export RePoE est étiqueté 0.5.5.5. Les données de gemme sont identiques (Arc aux niveaux 22 et 32, I-13) : on garde 0.5.5.5 pour les tables et 0.5.5e pour les mesures en jeu.
