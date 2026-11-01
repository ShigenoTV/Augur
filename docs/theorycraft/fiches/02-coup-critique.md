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
- `crit_plat` (chance de base additionnelle) s'ajoute à la base **avant** les increased, more et less.
- Le bonus : on ajoute d'abord les « +X% to Critical Damage Bonus » aux 100 % de base, puis on applique les increased et more. Les +X% des armes sont locaux (seulement pour cette arme).
- Un seul jet de critique par utilisation de la compétence (de 0 à 99,99) : tous les coups de cette utilisation sont critiques ou aucun. Cela ne change pas la moyenne, seulement la variance. Les projectiles multiples du joueur tirent chacun leur critique.
- Côté défense : un coup critique d'attaque qui touche peut être dégradé en coup normal par un second test d'évasion.

## Arrondis

Le panneau affiche la chance de critique avec deux décimales (à confirmer). Hypothèse : aucun arrondi interne.

## Cas limites

- Chance plafonnée à 100 %.
- Chance de base nulle : les increased n'ont aucun effet.
- Les modificateurs défensifs qui réduisent le bonus critique subi (ex. Battle-hardened) s'appliquent à cette étape (hors périmètre côté cible pour le Sprint 1).

## Sources

- [S-WIKI-CRIT] poe2wiki.net, captures transmises par Max le 31 octobre 2026 : *Critical hit*. https://www.poe2wiki.net/wiki/Critical_hit
- [S-ORDRE] mobalytics, mis à jour le 20 janvier 2026 : « Player character Critical Damage Bonus is 100% by default ».

## À vérifier en jeu

- Rien de bloquant : les deux points (I-08) sont tranchés par le wiki. La mesure 4 les recoupera.
