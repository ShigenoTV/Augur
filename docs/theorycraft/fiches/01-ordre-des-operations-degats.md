# 01. Ordre des opérations des dégâts (coups)

**Confiance :** `supposé` · **Patch de référence :** inconnu (à relever en jeu) · **Cas golden :** `tests/golden/degats.yaml`

## Résumé

Les dégâts d'un coup se calculent type par type (physique, feu, froid, foudre, chaos), dans cet ordre :

1. **Dégâts de base** : la gemme pour un sort (table par niveau), l'arme pour une attaque.
2. **Dégâts ajoutés** (flat) : ajoutés au minimum et au maximum de chaque type.
3. **Conversion et gain en dégâts supplémentaires**, en deux étapes :
   - étape 1 : conversion et gain **propres à la compétence** ;
   - étape 2 : conversion et gain venant de l'**équipement, de l'arbre et des buffs**.
4. **Increased / reduced** : tous les pourcentages applicables à un type sont **additionnés**, puis appliqués une fois : `× (1 + Σ/100)`.
5. **More / less** : chaque modificateur est **multiplicatif** : `× Π(1 + m/100)`.
6. **Coup critique** (fiche 02).
7. **Tirage** entre minimum et maximum (lucky / unlucky : deux tirages, meilleur ou pire, les deux s'annulent).
8. **Double / triple dégâts** (le triple remplace le double s'ils se déclenchent ensemble).

La mitigation de la cible (résistance, pénétration, fiche 04) s'applique après.

## Formule (valeur moyenne par type)

```
moyenne_base(t)     = (min_base(t) + max_base(t)) / 2
apres_ajoutes(t)    = moyenne_base(t) + moyenne_ajoutes(t) × efficacite_ajoutes
apres_conversion(t) = répartition après étapes 1 et 2 (voir ci-dessous)
final(t)            = apres_conversion(t) × max(0, 1 + Σ increased(t)/100) × Π(1 + more/100)
coup_moyen          = Σ_t final(t)
```

Le calcul est linéaire jusqu'au crit : la moyenne des dégâts est la moyenne du minimum et du maximum. Le moteur doit pourtant garder le minimum et le maximum séparément, car ce sont eux que le tooltip affiche.

### Conversion

- Dans une même étape, si la somme des conversions d'un type source dépasse 100 %, elles sont **ramenées proportionnellement à 100 %** (ex. 60 % vers feu + 60 % vers foudre deviennent 50 % + 50 %).
- La normalisation se fait **par étape**, pas sur les deux étapes cumulées. Les étapes 1 et 2 ne s'additionnent pas : l'étape 2 s'applique à ce qui reste après l'étape 1.
- **Rupture avec PoE 1 :** les dégâts convertis **ne se souviennent pas** de leur type d'origine. Ils ne profitent que des modificateurs de leur type final. Exemple : du feu converti en foudre profite de « increased Lightning Damage », pas de « increased Fire Damage ».
- Hypothèse de travail : à l'intérieur d'une étape, une conversion ne s'enchaîne pas (du froid converti en feu à l'étape 2 n'est pas reconverti en foudre par une autre conversion de la même étape). Les dégâts gagnés à l'étape 2 ne sont pas reconvertis.

### Gain en dégâts supplémentaires (« gain X% of Y as extra Z »)

- Les dégâts gagnés ne profitent que des modificateurs du **type gagné** (pas de ceux du type source).
- Hypothèse de travail : le montant gagné se calcule sur le montant du type source **avant** les conversions de la même étape.

### Étiquettes des increased

Un pourcentage s'applique à un type s'il porte l'une de ses étiquettes : le type lui-même (`froid`), `elementaire` pour feu/froid/foudre, `sort` pour tous les dégâts d'un sort, `tous` pour « increased Damage ».

## Arrondis

**Inconnus.** Hypothèse de travail : calcul en flottant, arrondi uniquement à l'affichage (minimum et maximum du tooltip arrondis à l'entier le plus proche). À mesurer (registre, entrée I-07).

## Cas limites et interactions

- Somme des increased inférieure à −100 % : on suppose un plancher à 0 (pas de dégâts négatifs).
- Un more de −100 % annule le type.
- Ordre étape 1 puis étape 2 : une conversion de compétence suivie d'une conversion d'équipement donne un résultat différent d'une conversion unique (cas `degats-005`).

## Sources

- [S-ORDRE] mobalytics, *Damage & Defence Calculation Order* (PoE 2), mis à jour le 20 janvier 2026, pas de numéro de patch indiqué. https://mobalytics.gg/poe-2/guides/damage-defence-calc-order
- [S-CONVERSION] mobalytics, *Damage Conversion* (PoE 2), date non relevée. https://mobalytics.gg/poe-2/guides/damage-conversion
- Le wiki communautaire poe2wiki.net n'a pas pu être consulté (protection anti-robots) : à relire par un humain.

## À vérifier en jeu

- Héritage des modificateurs après conversion : comparer le tooltip d'un sort converti avec et sans un objet « increased <type source> » (cas discriminant `degats-003`).
- Base du gain en dégâts supplémentaires quand une conversion de la même étape existe (`degats-007`).
- Règles d'arrondi du minimum et du maximum affichés.
