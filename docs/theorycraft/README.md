# Theorycraft Augur : fiches mécaniques et cas golden

Ce dossier contient la « vérité » du moteur `poe2-core`, au sens de la section 3 du [plan fondateur](../plan-fondateur.md) : le Theorycrafter écrit les fiches et les cas golden, les développeurs implémentent jusqu'à ce que les cas passent.

## Contenu

| Fichier | Rôle |
|---|---|
| [archetype-sprint1.md](archetype-sprint1.md) | Archétype retenu pour la bêta du Sprint 1 et son périmètre |
| [fiches/](fiches/) | Une fiche par mécanique (formule, ordre des opérations, arrondis, cas limites, confiance, sources) |
| [registre-des-inconnues.md](registre-des-inconnues.md) | Ce qui reste à mesurer en jeu, par priorité |
| [format-cas-golden.md](format-cas-golden.md) | Le format YAML des cas golden (contrat avec l'exécuteur Rust) |
| `../../tests/formules/*.yaml` | Les cas golden, un fichier par mécanique |
| `../../tools/theorycraft/verifier_golden.py` | Vérificateur de cohérence : recalcule chaque cas unitaire avec les formules des fiches |

## Niveaux de confiance

Repris du plan fondateur, section 3.1 :

- `confirmé` : patch notes, déclaration d'un développeur GGG, ou valeur affichée par le jeu (tooltip, panneau du personnage).
- `mesuré` : test empirique reproductible, capture à l'appui.
- `supposé` : tout le reste, y compris les formules reprises de guides communautaires.

**État au 31 octobre 2026 : tout est `supposé`.** Les fiches ont été rédigées sans accès au jeu, à partir de sources communautaires datées (citées dans chaque fiche). Ces pages ont souvent un patch de retard. Aucune formule ne passe à `confirmé` ou `mesuré` sans la preuve correspondante (voir le protocole de mesure, section 3.4 du plan).

## Ce qui est bloquant et ne peut pas être fait sans le jeu

1. **Les mesures en jeu** (personnage de test, captures du panneau et des tooltips) : seul moyen de faire passer une fiche de `supposé` à `mesuré`.
2. **L'extraction des `.datc64`** (tables de niveaux de gemmes, mods, stats) : elle demande un client du jeu installé et à jour. Les cas golden de type `personnage` en dépendent.
3. **Le numéro du patch courant** : le champ `patch` des cas reste `null` tant qu'il n'a pas été relevé sur le client.

Aucune valeur n'a été inventée pour combler ces trous : les cas concernés ont `attendu: null` et le statut `à-mesurer`.
