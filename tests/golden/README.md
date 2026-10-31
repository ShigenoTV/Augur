# Cas de test « golden »

Chaque fichier `.yaml` de ce dossier (sous-dossiers compris) est exécuté par
`cargo test -p poe2-core --test golden`. Un cas qui échoue fait échouer la CI.

Format : voir `crates/poe2-core/src/calc/golden.rs` (documentation en tête de
fichier) et la section 3.1 du plan fondateur. Résumé :

| Champ | Rôle |
|---|---|
| `id`, `patch` | Identifiant unique et patch du jeu concerné |
| `confiance` | `confirmé`, `mesuré` ou `supposé` |
| `statut` | `actif` (défaut) ou `non_supporte` (mécanique pas encore implémentée : listé, non exécuté) |
| `personnage.competence` | Gemme : `temps_incantation`, `critique_base`, `efficacite_degats_ajoutes`, `degats: { feu: [min, max] }` |
| `personnage.modificateurs` | Liste de `{ stat, type, valeur, source }` ; `type` ∈ `base`, `added`, `increased`, `more`, `override` (valeur négative = reduced / less) |
| `conditions` | `cible: { resistances: { feu: 30 }, armure: 0 }`, `coup_subi` (dégâts physiques d'un coup reçu) |
| `attendu` | `nom_de_sortie: valeur` ou `{ valeur, tolerance_pct, tolerance_abs }` ; `tolerance_pct` par défaut 0,5 |
| `preuve` | Lien vers la capture ou la vidéo |

Sorties disponibles : `dps`, `coup_moyen`, `coup_moyen_brut`,
`chance_critique`, `bonus_degats_critiques`, `incantations_par_seconde`, `vie`,
`bouclier_energie`, `armure`, `resistance_<type>`, `reduction_armure`,
`coup_max_<type>`. Types : `physique`, `feu`, `froid`, `foudre`, `chaos`.

Pour lire la trace complète d'un cas :

```
cargo run -p poe2-core --example trace -- tests/golden/moteur/sort-feu-nominal-001.yaml
```

> **Important.** Les cas du dossier `moteur/` sont des cas de **cohérence du
> moteur**, calculés à la main à partir des formules supposées. Ils ne
> remplacent pas les cas mesurés en jeu par le Theorycrafter : ils vérifient
> que le code applique bien l'ordre des opérations décrit, pas que cet ordre
> est celui du jeu. Les cas mesurés iront dans `archetype/`.
