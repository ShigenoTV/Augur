# Registre des inconnues

Feuille de route des tests en jeu. Priorité : **P0** bloque l'exactitude de l'archétype du Sprint 1 ; **P1** fausse un indicateur affiché ; **P2** cas rares ou hors périmètre.

Personne ne peut fermer une entrée sans preuve (capture ou vidéo référencée dans un cas golden, voir le protocole de la section 3.4 du plan).

| Id | Prio | Mécanique | Question | Comment la trancher | Bloqué par |
|---|---|---|---|---|---|
| I-02 | P0 | Résistances (fiche 04) | Pénalité de progression : le wiki donne −10/−20/−30 % aux actes 2 à 4, puis −40/−50/−60 % selon le niveau de zone (54, 60, 65+). Reste à confirmer −60 % en endgame sur le patch courant | Objets connus + panneau dans une zone de niveau 65+ (mesure 2) | Mesure de Max |
| I-06 | P0 | Évasion (fiche 06) | Précision des monstres par niveau | Table `accuracy` de `default_monster_stats.json` dans RePoE (ex. niveau 65 : 1114, niveau 80 : 1964) ; à recouper avec la chance d'éviter du panneau (mesure 3) | **Débloqué via RePoE** (voir plus bas) |
| I-07 | P0 | Dégâts (fiche 01) | Arrondis du minimum et du maximum affichés dans le tooltip | Comparer nos valeurs non arrondies au tooltip sur 10 configurations | Jeu installé |
| I-13 | P0 | Gemmes | Dégâts de base, chance de critique et temps d'incantation de la gemme choisie, par niveau | `skills.json` de RePoE (dégâts min/max par niveau, `cast_time` en ms, `crit_chance` en centièmes de %), recoupé avec le tooltip (mesure 4) | **Débloqué via RePoE** |
| I-09 | P1 | Vitesse (fiche 03) | Le temps d'incantation est-il arrondi au tick serveur (~33 ms) ? | Temps d'incantation affiché pour 5 valeurs de vitesse ; mesure vidéo à 60 i/s si l'affichage ne tranche pas | Jeu installé |
| I-11 | P1 | Résistances (fiche 04) | Résistances des boss et des monstres de carte | Données extraites ; à défaut, documentation de GGG | Extraction `.datc64` (absent de `default_monster_stats.json` de RePoE) |
| I-12 | P1 | ES (fiche 07) | Qualité additive aux increased locaux ? Arrondi par objet ? | ES affichée d'un objet à plusieurs qualités | Jeu installé |
| I-10 | P2 | Résistances (fiche 04) | Résistance minimale (−200 % en PoE 1) | Cumul de malus de résistance en jeu | Jeu installé |
| I-15 | P2 | Dégâts (fiche 01) | Plancher à 0 quand les increased sont inférieurs à −100 % | Cas `degats-002` | Jeu installé |

## Inconnues fermées

Fermées par la documentation (sources concordantes). Les fiches restent au niveau `supposé` tant qu'une mesure en jeu ne les a pas recoupées, sauf I-01 (texte du jeu).

| Id | Mécanique | Réponse | Source |
| --- | --- | --- | --- |
| I-03 | Dégâts (fiche 01) | Les dégâts convertis ou gagnés ne profitent que des modificateurs de leur type final | poe2wiki *Damage conversion* + mobalytics |
| I-04 | Évasion (fiche 06) | Chance de toucher = 1,25 × P / (P + 0,3 × E), plafonds 5 % / 95 % | poe2wiki *Evasion* + mobalytics ; recoupé par la mesure 3 |
| I-05 | Armure (fiche 05) | Constante 10 : A / (A + 10 × D), plafond 90 %, plancher de 1 dégât | poe2wiki *Armour* + mobalytics |
| I-08 | Critique (fiche 02) | Chance de base additionnelle avant increased/more ; bonus = (100 + plats) puis increased/more | poe2wiki *Critical hit* |
| I-14 | Dégâts (fiche 01) | Gains calculés sur la base d'avant l'étape, puis convertis par les conversions de la même étape | poe2wiki *Damage conversion* (exemple, cas `degats-011`) |
| I-01 | ES (fiche 07) | Le chaos retire bien 2× l'ES | Texte du mot-clé du jeu, via [poe2db](https://poe2db.tw/us/Energy_Shield), 31 octobre 2026 |

## Source de données en attendant notre propre extraction

[RePoE (fork PoE 2)](https://repoe-fork.github.io/poe2/) publie les tables du jeu en JSON, **patch 0.5.5.5** au 31 octobre 2026 : `skills`, `skill_gems`, `mods`, `stats_by_file`, `stat_translations`, `base_items`, `default_monster_stats`, `world_areas`, etc. C'est une extraction communautaire des fichiers du jeu, pas une donnée de GGG.

- Utilisable tout de suite pour les tables (niveaux de gemmes, précision et armure des monstres par niveau, mods), en indiquant `RePoE 0.5.5.5` comme source dans les cas.
- Ne remplace pas les mesures en jeu : RePoE donne des **données**, pas les **formules** qui les combinent.
- Ne contient pas les résistances des monstres (I-11).
- Le pipeline d'extraction `.datc64` du plan reste la cible pour la production ; RePoE sert de référence croisée pour le valider.

## Autre point bloquant

Le **numéro du patch courant** reste à confirmer dans le client (mesure 1). Les données RePoE correspondent au patch 0.5.5.5.
