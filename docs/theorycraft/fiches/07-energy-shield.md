# 07. Energy Shield

**Confiance :** `supposé` · **Patch de référence :** inconnu · **Cas golden :** `tests/formules/energy_shield.yaml`

## Maximum

```
es_objet(i) = base(i) × (1 + (increased_local(i) + qualite(i)) / 100)        (hypothèse : qualité additive aux increased locaux)
es_max      = (Σ es_objet + es_plat_global) × (1 + increased_global/100) × Π(1 + more/100)
```

- Les increased **locaux** (sur l'objet) s'appliquent à la base de cet objet seul.
- Le total des objets et des sources plates est ensuite multiplié par les increased **globaux** (y compris « increased Global Defences »).

## Recharge

```
delai_recharge = 4 s / (1 + demarrage_plus_rapide/100)
vitesse        = 12,5 % × es_max × (1 + increased_recharge/100)         par seconde
```

- La recharge démarre après **4 secondes** sans perte d'ES par des dégâts ; une perte d'ES pendant ce délai le relance.
- « Faster start » : délai = 400 / (100 + r) secondes, r étant la somme des « faster start » en % (100 % donne 2 s). Formule donnée par le wiki.
- Les dégâts pris sur l'ES sont ceux qui restent après toute la mitigation (armure, résistances).
- Tout dégât pris sur la vie ou l'ES relance le délai.

## Absorption

- L'ES absorbe les dégâts avant la vie.
- **Le chaos retire deux fois plus d'ES** : 100 dégâts de chaos retirent 200 ES. **`confirmé`** : c'est le texte du mot-clé Energy Shield dans le jeu, repris par poe2db (« Chaos damage removes twice as much Energy Shield »).
- Poison et saignement contournent l'ES.

## Arrondis

Hypothèse : l'ES de chaque objet est arrondie à l'entier à l'affichage. Arrondi local (par objet) ou global : inconnu (I-12).

## Sources

- [S-WIKI-ES] poe2wiki.net, captures transmises par Max le 31 octobre 2026 : *Energy Shield*, recharge de 12,5 %/s après 4 s, délai = 400/(100 + r), chaos ×2. https://www.poe2wiki.net/wiki/Energy_Shield
- [S-POE2DB-ES] poe2db.tw, *Energy Shield*, consulté le 31 octobre 2026 : texte du mot-clé du jeu. https://poe2db.tw/us/Energy_Shield
- [S-ES] mobalytics, *Energy Shield* (PoE 2), date non relevée : délai 4 s, 12,5 %/s, 100 % faster start → 2 s, chaos ×2, sources plates additionnées puis multipliées par les modificateurs globaux. https://mobalytics.gg/poe-2/guides/energy-shield
- [S-ORDRE] mobalytics, 20 janvier 2026 : chaos ×2 sur l'ES, poison et saignement contournent l'ES.

## À vérifier en jeu

- Rôle de la qualité sur l'ES d'un objet (additive aux increased locaux ou multiplicative ; I-12).
