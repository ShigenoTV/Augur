# 09. Arc (deuxième archétype)

**Confiance :** `supposé` pour le DPS complet, `mesuré` pour les données de gemme, le nombre de chaînes et la pénétration · **Patch :** client 0.5.5e, données RePoE 0.5.5.5 · **Cas golden :** `tests/golden/archetype/arc-*.yaml`

Arc est le sort que joue Max (Gemling Legionnaire niveau 95, Arc lancé par un totem). Il valide le deuxième archétype du plan : un sort de foudre qui **chaîne**. Le moteur reste sur la définition de DPS du panneau d'information du jeu, c'est-à-dire **un coup sur une cible par incantation** ; les chaînes ne comptent que pour le nombre de cibles touchées.

## Données de la gemme (RePoE 0.5.5.5, `ArcPlayer`)

| Donnée | Valeur |
|---|---|
| Temps d'incantation | 1,10 s |
| Chance de critique de base | 9 % |
| Dégâts de foudre, niveau 20 / 22 / 32 / 33 | 20 à 386 / 26 à 499 / 94 à 1791 / 107 à 2039 |
| Chaînes de base, niveau 1 / 20 / 22 / 32 | 5 / 9 / 9 / 11 |
| Infusion de foudre (consommée si possible) | +6 chaînes, 200 % de dégâts en plus |
| Choc | 50 % d'ampleur en plus (ailment, hors périmètre) |

Les niveaux 22 et 32 sont **mesurés** : le tooltip du jeu retombe exactement dessus (I-13).

## Nombre de chaînes

```
chaines = chaines_de_base(niveau) + plancher(qualité × 0,1) + bonus
chaines_avec_Dominus_Grasp = chaines × 2          (« Chain 100 % more times »)
```

`mesuré` sur trois relevés de Max : niveau 22, qualité 31 % : 9 + 3 = **12** (tooltip) ; niveau 32, qualité 36 % : 11 + 3 = **14** (tooltip) ; avec Dominus' Grasp dans le panneau d'information : 14 × 2 = **28**. Le jeu affiche 6 chaînes supplémentaires avec l'infusion, comme RePoE.

## Pénétration affichée par le panneau

Le panneau additionne la pénétration et la réduction de résistance de l'ennemi : foudre **68 %** = 30 (gemme Lightning Penetration) + 30 (arbre : Electric Amplification 18 et 12 ailleurs) + 8 (Stormcharged) ; feu et froid **8 %** (Stormcharged). `mesuré` : chaque terme retombe sur l'affichage.

## Combinaison des chiffres du panneau

`mesuré` (cas `arc-panneau-mesure-001`) :

```
dégâts moyens par coup = moyenne(total) × (1 + chance_critique × bonus_critique)    = 31 501 × (1 + 31,41 % × 465 %) = 77 510,26
DPS                    = dégâts moyens par coup × incantations par seconde          = 77 510,26 × 1,55 = 120 140,9
```

## Ce que le panneau montre sur les dégâts

- Le maximum de foudre est environ 10 % plus haut que le minimum, comparé à la base : c'est Pure Power (« 10 % more maximum Lightning Damage », nœud de l'arbre), qui ne touche que le maximum.
- Le total de foudre (2445 à 51 241 au niveau 32) n'est atteignable qu'avec l'infusion à 200 % de dégâts en plus : sans elle, la somme des modificateurs connus donnerait 8,5 fois la base au lieu de 26 fois. `supposé` : le panneau semble afficher les dégâts **comme si Arc était infusé**.
- Le feu et le froid viennent des gains de la baguette (27 % et 25 % des dégâts en extra, 11 % des dégâts élémentaires en froid) et de deux nœuds de l'arbre (6 % de foudre en froid, 6 % d'élémentaire en foudre).

## Supports d'Arc chez Max et leur effet (RePoE)

| Gemme | Effet |
|---|---|
| Spell Totem (niveau 14) | Lance Arc par un totem ; 10 % de dégâts en plus par charge de puissance consommée (non vérifié) |
| Urgent Totems III | 20 % de vitesse d'incantation en plus pour les sorts des totems ; 100 % de placement |
| Lightning Penetration | Pénétration de 30 % de la résistance à la foudre |
| Zenith II | 30 % de dégâts de sort en plus au-dessus de 90 % de mana |
| Dominus' Grasp | 100 % de chaînes en plus |

## Sources

- RePoE (fork PoE 2), export 0.5.5.5 : `skills.json` (`ArcPlayer` et les supports), `passive_skill_trees/Default.json`.
- Captures de Max du 1er novembre 2026 : tooltips d'Arc (niveaux 22 et 32), panneau d'information d'Arc, export Path of Building (décodé).

## À vérifier en jeu

- **Cadence** (I-16, fermée le 5 novembre 2026) : Arc est lancé par Spell Totem, qui retire 25 % de vitesse d'incantation (`skills.json`). Cadence = (1 / 1,1 s) × 0,75 × (1 + somme des increased + 20 % d'Urgent Totems III). Mesuré : 1,55/s avec Urgent Totems III, 1,41/s sans (DPS 120 140,9 et 109 289,46 = 77 510,26 × cadence arrondie à 2 décimales) ; les dégâts par coup ne bougent pas.
- **Infusion** : le panneau suppose-t-il l'infusion (×3 sur la foudre) ? (I-17). Test : retirer Zenith II (−30 % de dégâts attendus) puis lire si l'infusion change l'affichage.
- **Charges de puissance** du Spell Totem : prises en compte ou non dans le DPS affiché ?
