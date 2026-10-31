# 02. Coup critique

**Confiance :** `supposé` · **Patch de référence :** inconnu · **Cas golden :** `tests/formules/critique.yaml`

## Formule

```
chance_crit  = min(100, (crit_base + crit_plat) × (1 + increased_crit/100))       en %
bonus_crit   = (100 + bonus_plat) × (1 + increased_bonus/100)                       en %
mult_moyen   = 1 + (chance_crit/100) × (bonus_crit/100)
```

- Le **bonus de dégâts critiques du joueur vaut 100 % par défaut** : un critique inflige 200 % des dégâts.
- `crit_base` est la chance de base de la gemme de sort (donnée de la gemme, extraite des `.datc64`).
- `crit_plat` (« +X% to Critical Hit Chance ») s'ajoute à la base **avant** les increased : hypothèse de travail.
- `increased_bonus` multiplie le bonus total (« increased Critical Damage Bonus ») : hypothèse de travail.

## Arrondis

Le panneau affiche la chance de critique avec deux décimales (à confirmer). Hypothèse : aucun arrondi interne.

## Cas limites

- Chance plafonnée à 100 %.
- Chance de base nulle : les increased n'ont aucun effet.
- Les modificateurs défensifs qui réduisent le bonus critique subi (ex. Battle-hardened) s'appliquent à cette étape (hors périmètre côté cible pour le Sprint 1).

## Sources

- [S-ORDRE] mobalytics, mis à jour le 20 janvier 2026 : « Player character Critical Damage Bonus is 100% by default ».

## À vérifier en jeu

- Ordre plat puis increased pour la chance de critique (I-08).
- Additivité entre « +X% bonus » et « increased bonus » (I-08).
