# 05. Armure

**Confiance :** `supposé` · **Patch de référence :** inconnu · **Cas golden :** `tests/formules/armure.yaml`

## Formule

```
reduction_armure = A / (A + 10 × D)                         (A : armure totale, D : dégâts physiques du coup)
reduction_totale = min(0,90, reduction_armure + reduction_physique_additionnelle)
degats_subis     = D × (1 − reduction_totale)
```

- **Constante 10** (PoE 1 utilise 5) : il faut une armure égale à 10 fois le coup pour réduire de 50 %.
- La réduction dépend de la taille du coup : l'armure protège bien des petits coups et mal des gros.
- La « réduction des dégâts physiques additionnelle » s'additionne à la réduction de l'armure, puis le total est plafonné à **90 %**.
- L'armure ne s'applique qu'aux **coups physiques**. Certains modificateurs l'appliquent en partie aux dégâts élémentaires (ex. 30 % de l'armure contre le feu) : ce calcul se fait **avant** les résistances, type par type. Hors périmètre du Sprint 1.

## Ordre

L'armure fait partie de l'étape « mitigation » : elle se calcule sur le coup entrant, **avant** les modificateurs de dégâts subis (increased/more damage taken). Voir la fiche 08 pour l'impact sur l'EHP.

## Arrondis

Inconnus. Hypothèse : flottant, aucun arrondi.

## Sources

- [S-ARMURE] mobalytics, *Armour* (PoE 2), mis à jour le 11 septembre 2025 : « Damage Reduction = AR / ( AR + 10 * DMG ) », plafond 90 %. https://mobalytics.gg/poe-2/guides/armour
- [S-ORDRE] mobalytics, 20 janvier 2026 : la réduction additionnelle s'ajoute à celle de l'armure, plafond 90 %.

## À vérifier en jeu

- Constante 10 : le panneau du personnage indique-t-il une réduction estimée pour un coup de taille donnée ? Si oui, deux relevés suffisent à confirmer la constante (I-05).
