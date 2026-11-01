# 04. Résistances et pénétration

**Confiance :** `supposé` · **Patch de référence :** inconnu · **Cas golden :** `tests/formules/resistances.yaml`

## Résistances du joueur

```
max_effectif  = min(90, 75 + bonus_max)
apres_penalite = brute − penalite_campagne        (penalite_campagne = 0 pour le chaos)
finale        = min(max_effectif, apres_penalite)
surplus       = max(0, apres_penalite − max_effectif)    ("overcap", affiché à part)
mult_subi     = 1 − finale/100
```

- Maximum par défaut : **75 %**. Plafond dur du maximum : **90 %**.
- **Pénalité de progression** (élémentaires seulement, pas le chaos). Elle dépend de l'avancement, puis du **niveau de zone** dans les interludes et l'endgame :

  | Point de progression | Pénalité |
  | --- | --- |
  | Acte 1 | 0 % |
  | Acte 2 | −10 % |
  | Acte 3 | −20 % |
  | Acte 4 | −30 % |
  | Interlude, niveau de zone 54 à 59 | −40 % |
  | Interlude, niveau de zone 60 à 64 | −50 % |
  | Endgame, niveau de zone 65 et plus | −60 % |

  Le moteur garde la pénalité en **paramètre** ; pour l'archétype du Sprint 1 (endgame), on utilise −60 %. Une ancienne source (mobalytics) parlait de −10 % par acte sur six actes : même total en endgame, mais un découpage dépassé (I-02).
- Sous 0 %, la résistance devient un malus : −20 % fait subir 20 % de dégâts en plus.
- Minimum : non documenté par la source. PoE 1 utilise −200 %. Non modélisé (I-10).

## Résistance de la cible et pénétration

```
si res_cible ≤ 0 : effective = res_cible                 (la pénétration n'a aucun effet)
sinon           : effective = max(0, res_cible − penetration)
mult_degats     = 1 − effective/100
```

- **La pénétration ne fait jamais descendre la résistance sous 0 %.** Malédictions et exposition, elles, peuvent la rendre négative, et s'appliquent avant la pénétration. Conséquence : on ne cumule pas les bénéfices d'une pénétration et d'une résistance négative.
- Exception connue : l'unique Leopold's Applause permet de pénétrer jusqu'à −50 % (hors périmètre).
- La pénétration ne s'applique qu'aux coups. Les dégâts déplacés par « damage taken as » perdent la pénétration de leur type d'origine.

## Arrondis

Les résistances sont des entiers dans le panneau. Hypothèse : aucun arrondi supplémentaire.

## Sources

- [S-WIKI-RES] poe2wiki.net, *Resistance*, section « Elemental resistance penalties », capture transmise par Max le 31 octobre 2026 (patch non indiqué sur la capture). https://www.poe2wiki.net/wiki/Resistance
- [S-RES] mobalytics, *Resistances* (PoE 2), date non relevée : maximum 75 %, plafond 90 %, −10 % par acte, −60 % au total, chaos non concerné. https://mobalytics.gg/poe-2/guides/resistances
- [S-WIKI-PEN] poe2wiki.net, captures transmises par Max le 31 octobre 2026 : *Penetration*, « to a minimum of 0% resistance by default », calculée après les autres modificateurs de résistance de la cible, sans effet sur les ailments. https://www.poe2wiki.net/wiki/Penetration
- [S-PEN] mobalytics, *Penetration* (PoE 2), date non relevée : « Penetration does not work beyond 0% Resistance ». https://mobalytics.gg/poe-2/guides/penetration
- [S-ORDRE] mobalytics, 20 janvier 2026 : « Penetration cannot bypass 0% Resistance ».

## À vérifier en jeu

- Pénalité en endgame : **mesurée à −60 %** le 1er novembre 2026 (I-02), sur un personnage de niveau 95 dans une zone de niveau 66. Reste à vérifier les paliers −40 et −50 %, qui ne sont connus que par le wiki.
- Valeurs de résistance des boss et des monstres de carte (nécessaires pour un DPS « réaliste » ; I-11).
