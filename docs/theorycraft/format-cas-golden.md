# Format des cas golden (YAML)

Contrat entre le Theorycrafter et l'exécuteur de tests de `poe2-core`. Il reprend l'exemple de la section 3.1 du plan fondateur et l'étend pour les tests de formule.

## Organisation

Deux familles de cas, dans deux dossiers distincts :

- **`tests/formules/`** (ce document) : cas **unitaires** de formule, un fichier par mécanique avec une liste `cas:`. Ils fixent ce que chaque fiche affirme, sans personnage. Les gabarits de mesure en jeu sont dans `tests/formules/personnage.yaml`.
- **`tests/golden/`** (format de l'exécuteur de `poe2-core`, voir `tests/golden/README.md` et la PR #5) : cas **personnage complet**, exécutés en CI par `cargo test -p poe2-core --test golden`. Les cas mesurés en jeu iront dans `tests/golden/archetype/`, au format de l'exécuteur.

Les deux dossiers sont séparés parce que l'exécuteur lit tous les `.yaml` de `tests/golden/` : y placer des fichiers d'un autre format casserait la CI. Un exécuteur Rust des cas `calcul` reste à écrire (suite logique, à valider).

```yaml
mecanique: armure
fiche: docs/theorycraft/fiches/05-armure.md
cas:
  - id: armure-001                # unique dans tout le dépôt, jamais réutilisé
    type: unitaire                # unitaire | personnage
    calcul: armure.reduction      # nom de la fonction de référence (voir ci-dessous)
    nature: nominal               # nominal | limite | interaction | discriminant
    patch: null                   # numéro de patch relevé en jeu ; null tant qu'inconnu
    confiance: supposé            # confirmé | mesuré | supposé
    statut: brouillon             # brouillon | à-mesurer | validé
    description: "..."
    entrees: { armure: 1000, degats: 100 }
    attendu: { reduction_pct: 50.0, degats_subis: 50.0 }
    tolerance_pct: 0.01           # écart relatif accepté, en %
    a_verifier_en_jeu: "..."      # optionnel
    preuve: null                  # lien vers la capture ou la vidéo
```

## Règles pour l'exécuteur

- `type: unitaire` : appeler la fonction du moteur qui correspond à `calcul` avec `entrees`, comparer chaque clé de `attendu` avec `tolerance_pct`. Ces cas sont exécutables dès maintenant.
- `type: personnage` : gabarit de mesure. Tant que `statut` n'est pas `validé`, `attendu` est `null` et l'exécuteur **ignore** le cas (il le compte comme « en attente » dans le rapport de CI).
- `nature: discriminant` : le cas sert à trancher entre deux hypothèses ; `a_verifier_en_jeu` décrit ce que donnerait l'hypothèse concurrente.
- Un cas `confiance: supposé` qui échoue bloque la fusion comme les autres : c'est le contrat. Si une mesure en jeu le contredit, on corrige **d'abord** le cas (et la fiche), puis le moteur.

## Calculs de référence

`tools/theorycraft/verifier_golden.py` implémente chaque `calcul` en Python, directement d'après les fiches. Il ne remplace pas le moteur : il garantit que les valeurs attendues sont cohérentes avec les fiches.

| `calcul` | Entrées | Sorties attendues |
|---|---|---|
| `degats.coup_moyen` | `base`, `ajoutes` (`{type: [min, max]}`), `efficacite_ajoutes`, `conversion_competence`, `gain_competence`, `conversion_secondaire`, `gain_secondaire` (`{source: {cible: pct}}`), `increased` (`{etiquette: pct}`), `more` (liste de pct) | `par_type` (`{type: moyenne}`), `total` |
| `critique.multiplicateur` | `crit_base`, `crit_plat`, `increased_crit`, `bonus_plat`, `increased_bonus` | `chance_pct`, `bonus_pct`, `multiplicateur_moyen` |
| `vitesse.incantation` | `temps_base`, `increased`, `more` | `cadence`, `temps` |
| `dps.sort` | `coup_moyen_par_type`, `multiplicateur_crit`, `cadence`, `res_cible`, `penetration` | `dps` |
| `resistances.joueur` | `type`, `brute`, `penalite_campagne`, `bonus_max` | `max_effectif`, `finale`, `surplus`, `multiplicateur_subi` |
| `resistances.cible` | `res_cible`, `penetration` | `effective`, `multiplicateur` |
| `armure.reduction` | `armure`, `degats`, `reduction_additionnelle_pct` | `reduction_pct`, `degats_subis` |
| `evasion.chance` | `precision`, `evasion` | `chance_eviter_pct` |
| `deflexion.chance` | `precision`, `deflexion` | `chance_deflexion_pct` |
| `es.maximum` | `objets` (liste de `{base, increased_local, qualite}`), `plat_global`, `increased_global`, `more` | `es_max` |
| `es.recharge` | `es_max`, `increased_recharge`, `demarrage_plus_rapide` | `delai_s`, `par_seconde` |
| `ehp.coup_max` | `type`, `vie`, `es`, `resistance`, `armure`, `increased_subi`, `more_subi` | `coup_max` |
