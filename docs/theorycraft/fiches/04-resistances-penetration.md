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
- **Pénalité de campagne : −10 % par acte terminé, −60 % au total après les six actes**, sur les résistances élémentaires seulement, pas sur le chaos. ⚠️ **Valeur la plus fragile de toutes les fiches** : la structure de la campagne de PoE 2 a changé entre les patchs. La pénalité est un **paramètre** du moteur, jamais une constante (I-02).
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

- [S-RES] mobalytics, *Resistances* (PoE 2), date non relevée : maximum 75 %, plafond 90 %, −10 % par acte, −60 % au total, chaos non concerné. https://mobalytics.gg/poe-2/guides/resistances
- [S-PEN] mobalytics, *Penetration* (PoE 2), date non relevée : « Penetration does not work beyond 0% Resistance ». https://mobalytics.gg/poe-2/guides/penetration
- [S-ORDRE] mobalytics, 20 janvier 2026 : « Penetration cannot bypass 0% Resistance ».

## À vérifier en jeu

- Pénalité de campagne réelle sur le patch courant : comparer la résistance affichée avant et après chaque acte, ou au minimum la somme brute des objets et la valeur du panneau en fin de campagne (I-02).
- Valeurs de résistance des boss et des monstres de carte (nécessaires pour un DPS « réaliste » ; I-11).
