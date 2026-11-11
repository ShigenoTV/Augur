# Recherche d'upgrades (Alt+U)

Dans l'overlay, **Alt+U** (ou « chercher mieux ») ouvre dans le navigateur une
recherche du site de trade officiel, pondérée pour ton personnage et pour
l'emplacement de l'objet copié.

## Calcul des poids

Code : `crates/poe2-core/src/comparaison/recherche.rs`.

1. L'emplacement de l'objet copié est vidé (l'arme reste, elle porte les dégâts
   des attaques). Pour une bague, c'est « Ring 1 ».
2. Chaque stat candidate est ajoutée seule, à une valeur typique
   (« +50 to maximum Life », « 30% increased Lightning Damage »…). Son gain de
   DPS et d'EHP (en %), divisé par cette valeur, donne son **poids** : le % de
   gain par point. 1 % de DPS vaut 1 % d'EHP.
3. Les stats qui rapportent moins de 0,1 % sont écartées. Une résistance déjà
   au plafond ne rapporte rien, elle n'entre donc pas dans la recherche.
4. Le **minimum** de la somme pondérée est ce que rapporte l'objet équipé :
   les résultats font au moins aussi bien, et sont triés du meilleur au moins bon.
5. Pièces d'armure : l'armure, l'évasion et le bouclier d'énergie demandés
   valent au moins 80 % de ceux de l'objet équipé (filtre « équipement »).
6. Budget : le prix maximum en exalts choisi dans **Réglages > Budget de la
   recherche d'upgrades** (vide : sans limite). La ligue est la « Ligue souhaitée ».

Si la compétence n'est pas calculée par le moteur, seule la défense compte (la
recherche le précise).

## Conformité

Augur n'envoie aucune requête au site de trade : il ouvre l'adresse
`https://www.pathofexile.com/trade2/search/poe2/<ligue>?q=<requête>` dans le
navigateur par défaut, sur l'action du joueur. C'est le navigateur qui lance la
recherche, une fois.

## Limites connues

- Les poids sont linéaires : deux stats qui se cumulent mal (résistances près
  du plafond) sont un peu surestimées ensemble.
- Les dégâts locaux des armes (dégâts physiques %, dégâts ajoutés de l'arme)
  ne sont pas pondérés.
- Les défenses locales d'une armure (« +# to maximum Energy Shield » local) ne
  sont pas pondérées, seulement filtrées au minimum.

## Sources

Identifiants des stats du site de trade (`explicit.stat_…`) relevés dans les
données d'[Exiled Exchange 2](https://github.com/Kvan7/Exiled-Exchange-2)
(`renderer/public/data/en/stats.ndjson`, licence MIT, © Alexander Drozdov et
contributeurs). Catégories d'objets du site de trade : même source.
