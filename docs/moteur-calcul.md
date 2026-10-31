# Moteur de calcul (`poe2-core`)

État au sprint 1 : squelette du moteur, archétype provisoire **sort à dégâts
directs**, mitigations défensives de base, traces de calcul, exécuteur des cas
golden et parseur d'objets.

## Organisation

| Module | Rôle |
|---|---|
| `calc::modifier` | Modificateurs typés `base`, `added`, `increased`, `more`, `override`, avec leur source |
| `calc::moddb` | Évaluation générique `(base + added) × (1 + Σinc) × Π(1 + more)` |
| `calc::offense` | DPS d'un sort contre une cible |
| `calc::defense` | Résistances, armure, coup maximal encaissable |
| `calc::trace` | Arbre de calcul (texte et JSON) |
| `calc::golden` | Lecture et exécution des cas YAML, rapport Markdown pour la CI |
| `parser` | Texte Ctrl+C / Ctrl+Alt+C → objet structuré |

Le crate respecte la frontière de l'ADR-001 : aucune E/S réseau ni fichier,
aucune dépendance Tauri ou tokio. L'appelant fournit les textes.

## Hypothèses à valider par le Theorycrafter

Toutes ces formules sont au niveau de confiance **supposé**. Chacune doit
recevoir une fiche mécanique et des cas golden mesurés en jeu.

| Mécanique | Formule implémentée |
|---|---|
| Ordre des opérations | gemme + ajoutés × efficacité → × max(0, 1 + Σ increased) → × Π more (plancher à 0 des increased cumulés) |
| Stats d'increased d'un sort | `damage`, `spell_damage`, `elemental_damage` (feu, froid, foudre), `<type>_damage` |
| Critique | chance = (base gemme + added) × increased × more, plafond 100 % ; multiplicateur moyen = 1 + chance × bonus ; bonus de base 100 % |
| Pénétration | ne fait pas descendre une résistance positive sous 0 ; sans effet sur une résistance négative |
| Résistances | min(Σ, maximum) ; maximum = 75 % + bonus, plafond absolu 90 % |
| Armure | réduction = A / (A + 10 × D), plafond 90 % |
| Réserve | vie + bouclier d'énergie ; contre le chaos, vie + ES / 2 (le chaos retire l'ES au double, fiche 07, I-01) |
| Pénalité de campagne | paramètre explicite `personnage.penalite_campagne` (ex. −60), ajouté aux résistances élémentaires avant plafonnement, sans effet sur le chaos (I-02) |
| Arrondis | aucun en interne ; l'arrondi relève de l'affichage et des tolérances |

## Pas encore couvert

Attaques, conversions de dégâts, dégâts dans le temps et ailments, buildup
(Electrocute, Freeze), Spirit et réservations, évasion, blocage, Heavy Stun,
tables de correspondance `stat_id` → stat interne. Un cas golden qui en
dépend se marque `statut: non_supporte`.

## Commandes

```
cargo test -p poe2-core                       # tout
cargo test -p poe2-core --test golden -- --nocapture   # rapport des cas golden
AUGUR_RAPPORT_GOLDEN=rapport.md cargo test -p poe2-core --test golden   # rapport pour la CI
cargo run -p poe2-core --example trace -- tests/golden/moteur/sort-feu-nominal-001.yaml
```
