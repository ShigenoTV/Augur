# Atlas Ledger : cadrage

**Statut :** démarrage du module, 7 novembre 2026 · **Code :** [`crates/poe2-core/src/ledger`](../../crates/poe2-core/src/ledger)

Atlas Ledger est le troisième produit du [plan fondateur](../plan-fondateur.md) (section 2, produit 3) : mesurer la rentabilité réelle des stratégies de farm de l'Atlas.

## Données disponibles aujourd'hui

| Source | Disponible | Ce qu'elle donne |
|---|---|---|
| `Client.txt` (déjà suivi par l'appli) | oui | Génération de zone (identifiant interne, niveau, graine de l'instance), entrée de zone (nom affiché), horodatage à la seconde. |
| Réserves de compte (`account:stashes`) | **non** | Fermé pour PoE 2 (voir [api-ggg.md](../architecture/api-ggg.md)) : pas de snapshot avant / après. |
| Prix | **non en production** | Le backend de prix attend un client OAuth confidentiel, que GGG n'accorde pas pour l'instant. |

**Décision de Max (7 novembre 2026) :** tant que l'API des réserves est fermée, Ledger mesure **le temps seulement** (maps par heure, temps par map, répartition map / hideout / ville). La valeur du butin se branchera quand une source existera ; le modèle de données lui garde la place.

## Découpage en PR

1. **Moteur de suivi de session** (`poe2-core::ledger`, Rust pur, sans E/S) : lecture des lignes du journal, classement des zones, découpage en parties de map, répartition du temps, maps par heure.
2. **Historique local et étiquetage** : sessions enregistrées en SQLite dans `%APPDATA%` (comme prévu au plan, section 1.4) ; stratégie étiquetée par le joueur (tablettes de précurseur, mécaniques ciblées, tier de Waystone).
3. **Panneau Ledger** dans la fenêtre principale : session en cours, liste des maps, historique.

Hors périmètre pour l'instant : valeur du butin, couche communautaire et prévision de marché (backend et historique nécessaires), stash tracker (en attente de l'API).

## Règles du suivi

- Le temps entre deux lignes « Generating level » est attribué à la zone en cours.
- **Classement** d'après l'identifiant interne (table `WorldAreas`) : `Map…` = map, `…Hideout…` = hideout (sauf `…_Claimable`), `…_town` = ville, `MapLeaguePortal` (Realmgate) = carrefour, le reste = autre.
- **Partie de map** : même identifiant et même graine = même instance. Un retour par portail après un passage au hideout prolonge la partie ; une zone « autre » ouverte depuis la map compte dans la partie ; une autre map ferme la précédente.
- Maps par heure = maps commencées / durée de la session (à partir d'une minute).

## Hypothèses à vérifier en jeu

| # | Hypothèse | Comment vérifier |
|---|---|---|
| L-01 | Revenir dans une instance de map déjà ouverte réécrit une ligne « Generating » avec **la même graine**. | Session de mesures : entrer dans une map, repasser au hideout, reprendre le portail, exporter. |
| L-02 | Les sous-zones ouvertes depuis une map (portails de mécaniques) ont un identifiant hors `Map…`. | Même session, en ouvrant une mécanique à sous-zone. |
| L-03 | L'horodatage du journal est l'heure locale du PC. | Comparer une ligne à l'heure du PC. |
