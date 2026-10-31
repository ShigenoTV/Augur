# 06. Évasion et déflexion

**Confiance :** `supposé` · **Patch de référence :** inconnu · **Cas golden :** `tests/formules/evasion.yaml`

## Évasion

```
chance_toucher = 1,25 × P / (P + 0,3 × E)          (P : précision de l'attaquant, E : évasion du défenseur)
chance_eviter  = clamp(0 %, 95 %, (1 − chance_toucher) × 100)
```

- Pas d'exposant (PoE 1 en utilise un).
- Plafond : 95 % d'évasion, donc toujours au moins 5 % de chance d'être touché.
- **Entropie, pas de hasard pur :** au premier coup évitable, le jeu tire une valeur de 0 à 99, puis ajoute la chance de toucher à chaque coup ; le coup porte quand le total atteint 100 (on retire alors 100). L'entropie est partagée par tous les attaquants et se réinitialise après 100 ticks serveur (environ 3,33 s) sans coup évitable. Pour l'EHP et l'affichage, on utilise la chance moyenne.
- L'évasion s'applique aux coups, y compris certains sorts de monstres (exemple de la source : une Fireball de monstre). Elle n'agit pas sur les dégâts dans le temps. Les attaques de boss télégraphiées ne peuvent pas être évitées.
- **La chance d'éviter affichée dans le panneau** est calculée contre la précision moyenne d'un monstre du niveau du personnage : c'est la table `accuracy` de `default_monster_stats.json` (RePoE). Exemple : niveau 80, précision 1964.
- **Les monstres ne peuvent pas éviter les sorts** du joueur : la précision ne compte pas pour l'archétype du Sprint 1.
- Les attaques du joueur perdent de la précision avec la distance (aucune pénalité à 2 m, −90 % à 9 m) ; hors périmètre.

## Déflexion

```
chance_deflexion = min(95 %, 150 × (1 − P / (P + 0,12 × D)))       (D : déflexion du défenseur)
coup dévié       = dégâts du coup et des ailments réduits de 40 %
```

- Tirée **en plus** de l'évasion, sur les coups qui n'ont pas été évités.
- La déflexion vient le plus souvent d'un pourcentage de l'évasion, calculé après la mise à l'échelle de l'évasion.

## Sources

- [S-WIKI-EVA] poe2wiki.net, captures transmises par Max le 31 octobre 2026 : *Evasion*, « Uncapped Chance to Hit = Accuracy × 1,25 × 100 / (Accuracy + Evasion × 0,3) », entropie, panneau calculé contre un monstre du niveau du joueur. https://www.poe2wiki.net/wiki/Evasion
- [S-EVASION] mobalytics, *Evasion* (PoE 2), mis à jour le 11 septembre 2025 : « Chance to Evade = ( 1 - ( AA * 1.25) / ( AA + DE * 0.3 ) ) * 100 », plafond 95 %, entropie. https://mobalytics.gg/poe-2/guides/evasion
- [S-DEFLEXION] mobalytics, *Deflection* (PoE 2), mis à jour le 22 mai 2026 : « Chance to Deflect = 150 ( 1 - A / ( A + 0.12 * D ) ) », 40 % de réduction par défaut. https://mobalytics.gg/poe-2/guides/deflection

## À vérifier en jeu

- Formule tranchée par le wiki et mobalytics (I-04). La mesure 3 la vérifie directement : chance d'éviter du panneau contre la précision RePoE du niveau du personnage.
