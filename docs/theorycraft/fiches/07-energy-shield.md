# 07. Energy Shield

**Confiance :** `supposé` (l'ES locale d'un objet est `mesuré`) · **Patch de référence :** inconnu · **Cas golden :** `tests/formules/energy_shield.yaml`

## Maximum

```
es_objet(i) = base(i) × (1 + increased_local(i) / 100) × (1 + qualite(i) / 100)        (qualité multiplicative : mesuré)
es_max      = (Σ es_objet + es_plat_global) × (1 + increased_global/100) × Π(1 + more/100)
```

- Les increased **locaux** (sur l'objet, runes comprises) s'additionnent entre eux et s'appliquent à la base de cet objet seul, **plats locaux compris** (base de l'objet + ES plate de ses affixes). **La qualité ne s'ajoute pas à ces increased : elle multiplie le résultat.** **`mesuré`** le 1er novembre 2026 sur 4 objets de Max (casque, torse, gants, bottes, client 0.5.5e), écart inférieur à 0,3 sur chacun (cas `tests/golden/archetype/es-objets-mesures.yaml`). Le calcul additif donnait 263, 491, 132 et 237 pour 275, 528, 146 et 254 affichés.
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

L'ES affichée d'un objet est **arrondie au plus proche** (145,8 est affiché 146, donc ce n'est pas un arrondi à l'inférieur). Arrondi du total du personnage, par objet ou global : inconnu, car le personnage de Max a 0 ES au panneau (I-12).

## Sources

- [S-WIKI-ES] poe2wiki.net, captures transmises par Max le 31 octobre 2026 : *Energy Shield*, recharge de 12,5 %/s après 4 s, délai = 400/(100 + r), chaos ×2. https://www.poe2wiki.net/wiki/Energy_Shield
- [S-POE2DB-ES] poe2db.tw, *Energy Shield*, consulté le 31 octobre 2026 : texte du mot-clé du jeu. https://poe2db.tw/us/Energy_Shield
- [S-ES] mobalytics, *Energy Shield* (PoE 2), date non relevée : délai 4 s, 12,5 %/s, 100 % faster start → 2 s, chaos ×2, sources plates additionnées puis multipliées par les modificateurs globaux. https://mobalytics.gg/poe-2/guides/energy-shield
- [S-ORDRE] mobalytics, 20 janvier 2026 : chaos ×2 sur l'ES, poison et saignement contournent l'ES.

## À vérifier en jeu

- Arrondi du total d'ES du personnage (par objet ou global ; I-12), sur un personnage qui a de l'ES au panneau.
