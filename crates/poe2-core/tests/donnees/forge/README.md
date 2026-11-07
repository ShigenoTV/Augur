# Cas de test croisés de Forge

Chaque fichier est généré par [`tools/theorycraft/fixture_forge.py`](../../../../../tools/theorycraft/fixture_forge.py)
à partir de l'export RePoE 0.5.5.7 et du relevé de poids poe2db du 05/11/2026 :

```bash
python3 tools/theorycraft/fixture_forge.py REPOE_DIR "Vile Robe" 82 crates/poe2-core/tests/donnees/forge/vile-robe-82.json
python3 tools/theorycraft/fixture_forge.py REPOE_DIR "Gold Ring" 75 crates/poe2-core/tests/donnees/forge/gold-ring-75.json
python3 tools/theorycraft/fixture_forge.py REPOE_DIR "Suede Bracers" 45 crates/poe2-core/tests/donnees/forge/suede-bracers-45.json
```

Le test [`forge_croise.rs`](../../forge_croise.rs) vérifie que le moteur Rust redonne, à 1e-12 près, les
probabilités calculées par `craft_proba.py`.

**Licence des poids.** Le champ `releve` de ces fichiers est un extrait du relevé poe2db : poids estimés par
Krakenbul (Prohibited Library), publiés par PoE2DB, sous licence
[CC BY-NC-SA 3.0](https://creativecommons.org/licenses/by-nc-sa/3.0/). Attribution complète :
[`LICENCE-poids-poe2db.md`](../../../../../tools/theorycraft/donnees/LICENCE-poids-poe2db.md).
