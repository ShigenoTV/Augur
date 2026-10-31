# 08. EHP (points de vie effectifs)

**Confiance :** convention interne Augur (formules) + `supposé` (mécaniques sous-jacentes) · **Cas golden :** `tests/formules/ehp.yaml`

L'EHP n'est pas une valeur du jeu : c'est **notre** indicateur produit. Cette fiche fixe sa définition pour que l'overlay et le planificateur affichent la même chose.

## Définition retenue : « coup maximal encaissable » par type de dégâts

Pour chaque type `t`, l'EHP est la taille du plus gros coup de ce type que le personnage peut encaisser depuis la pleine vie et le plein ES, sans évasion, déflexion ni blocage (ces protections sont probabilistes et affichées séparément).

```
reserve(t)   = vie + es                si t ≠ chaos
             = vie + es / 2            si t = chaos   (le chaos retire 2× l'ES, fiche 07)
m            = Π(1 + more_subi/100) × (1 + Σ increased_subi/100)    (modificateurs de dégâts subis)
```

**Types élémentaires et chaos :**
```
ehp(t) = reserve(t) / ((1 − res_finale(t)/100) × m)
```

**Physique avec armure** (l'armure dépend de la taille du coup D, fiche 05) : on résout `D × (1 − A/(A + 10D)) × m = R`, soit `10·m·D² − 10·R·D − R·A = 0` :
```
D = (10·R + √(100·R² + 40·m·R·A)) / (20·m)
si A / (A + 10·D) > 0,90 : D = R / (0,10 × m)         (plafond de 90 %)
```

## Choix assumés

- L'armure s'applique au coup **entrant** (avant les modificateurs de dégâts subis), conformément à l'ordre [S-ORDRE] : mitigation (étape 4) puis dégâts subis (étape 5).
- La mana n'entre pas dans la réserve (Mind over Matter hors périmètre).
- Les plafonds et pénalités de résistances viennent de la fiche 04.

## À vérifier en jeu

Rien de directement mesurable ; la fiabilité de l'EHP dépend des fiches 04, 05 et 07. Un test de bon sens possible : vérifier en jeu qu'un coup connu (ex. un boss au script fixe) tue ou non le personnage comme prévu.
