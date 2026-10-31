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
- La **précision des monstres** dépend de leur niveau : table à extraire des données (bloqué, I-06).

## Déflexion

```
chance_deflexion = min(95 %, 150 × (1 − P / (P + 0,12 × D)))       (D : déflexion du défenseur)
coup dévié       = dégâts du coup et des ailments réduits de 40 %
```

- Tirée **en plus** de l'évasion, sur les coups qui n'ont pas été évités.
- La déflexion vient le plus souvent d'un pourcentage de l'évasion, calculé après la mise à l'échelle de l'évasion.

## Sources

- [S-EVASION] mobalytics, *Evasion* (PoE 2), mis à jour le 11 septembre 2025 : « Chance to Evade = ( 1 - ( AA * 1.25) / ( AA + DE * 0.3 ) ) * 100 », plafond 95 %, entropie. https://mobalytics.gg/poe-2/guides/evasion
- [S-DEFLEXION] mobalytics, *Deflection* (PoE 2), mis à jour le 22 mai 2026 : « Chance to Deflect = 150 ( 1 - A / ( A + 0.12 * D ) ) », 40 % de réduction par défaut. https://mobalytics.gg/poe-2/guides/deflection

## À vérifier en jeu

- La formule d'évasion a plus d'un an : à recouper avec la chance d'éviter affichée dans le panneau pour au moins trois valeurs d'évasion (I-04).
- Table de précision des monstres par niveau (I-06, bloqué par l'extraction).
