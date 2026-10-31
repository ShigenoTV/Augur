# 03. Vitesse d'incantation

**Confiance :** `supposé` · **Patch de référence :** inconnu · **Cas golden :** `tests/formules/vitesse.yaml`

## Formule

```
cadence = (1 / temps_base) × max(0, 1 + Σ increased/100) × Π(1 + more/100)     en incantations par seconde
temps   = 1 / cadence
dps     = coup_moyen × mult_crit_moyen × cadence × mitigation_cible
```

`temps_base` est le temps d'incantation de la gemme (donnée extraite).

## Arrondis

**Inconnus et potentiellement importants.** Le serveur fonctionne par ticks (environ 33 ms d'après [S-EVASION], « 100 server ticks ≈ 3,333 s »). Si les actions sont arrondies au tick supérieur, la cadence réelle est en escalier. Hypothèse de travail : pas d'arrondi au tick, valeur continue. À mesurer (I-09).

## Sources

- Formule générique increased/more : [S-ORDRE].
- Durée d'un tick serveur : [S-EVASION] mobalytics, *Evasion*, 11 septembre 2025.

## À vérifier en jeu

- Le temps d'incantation affiché suit-il la formule continue ? Mesurer à plusieurs valeurs de vitesse d'incantation (I-09).
