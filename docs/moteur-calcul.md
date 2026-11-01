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
| `calc::golden` | Lecture et exécution des cas YAML personnage (`tests/golden/`), rapport Markdown pour la CI |
| `calc::formules` | Cas unitaires de formule du Theorycrafter (`tests/formules/`), rejoués par le moteur |
| `parser` | Texte Ctrl+C / Ctrl+Alt+C → objet structuré |

Le crate respecte la frontière de l'ADR-001 : aucune E/S réseau ni fichier,
aucune dépendance Tauri ou tokio. L'appelant fournit les textes.

## Hypothèses à valider par le Theorycrafter

Toutes ces formules sont au niveau de confiance **supposé**. Chacune doit
recevoir une fiche mécanique et des cas golden mesurés en jeu.

| Mécanique | Formule implémentée |
|---|---|
| Ordre des opérations | gemme + ajoutés × efficacité → conversions et gains (étape 1 compétence, étape 2 équipement) → × max(0, 1 + Σ increased du type final) → × Π more (plancher à 0 des increased cumulés) |
| Conversion (fiche 01) | à chaque étape, gains d'abord, calculés sur les montants d'avant l'étape (`degats` = total), puis conversions sur les montants gains compris, ramenées à 100 % par type source, sans enchaînement ; les dégâts convertis ou gagnés ne profitent que des modificateurs de leur type final |
| Défenses des objets (fiche 07) | valeur locale = base × (1 + (increased local + qualité) / 100), puis Σ objets + plats globaux, × increased global × more global |
| Recharge de l'ES (fiche 07) | délai = 4 s / (1 + démarrage plus rapide) ; 12,5 % du maximum par seconde × (1 + increased) |
| Évasion et déflexion (fiche 06) | éviter = 1 − 1,25 P / (P + 0,3 É) ; déflexion = 150 % × (1 − P / (P + 0,12 D)) ; entre 0 et 95 % |
| Stats d'increased d'un sort | `damage`, `spell_damage`, `elemental_damage` (feu, froid, foudre), `<type>_damage` |
| Critique | chance = (base gemme + added) × increased × more, plafond 100 % ; multiplicateur moyen = 1 + chance × bonus ; bonus de base 100 % |
| Pénétration | ne fait pas descendre une résistance positive sous 0 ; sans effet sur une résistance négative |
| Résistances | min(Σ, maximum) ; maximum = 75 % + bonus, plafond absolu 90 % |
| Armure | réduction = A / (A + 10 × D) + `physical_damage_reduction`, plafond 90 % |
| Dégâts subis | `damage_taken` : × max(0, 1 + Σ increased) × Π more, appliqué au coup maximal encaissable |
| Réserve | vie + bouclier d'énergie ; contre le chaos, vie + ES / 2 (le chaos retire l'ES au double, fiche 07, I-01) |
| Pénalité de campagne | paramètre explicite `personnage.penalite_campagne` (ex. −60), ajouté aux résistances élémentaires avant plafonnement, sans effet sur le chaos (I-02) |
| Arrondis | aucun en interne ; l'arrondi relève de l'affichage et des tolérances |

## Pas encore couvert

Attaques, dégâts dans le temps et ailments, buildup (Electrocute, Freeze),
Spirit et réservations, blocage, Heavy Stun, effet de la déflexion sur les
dégâts, plancher de 1 dégât par instance après armure (poe2wiki),
tables de correspondance `stat_id` → stat interne. Un cas golden qui en
dépend se marque `statut: non_supporte`.

## Commandes

```
cargo test -p poe2-core                       # tout
cargo test -p poe2-core --test golden -- --nocapture   # rapport des cas golden
cargo test -p poe2-core --test formules -- --nocapture # rapport des cas de formule
AUGUR_RAPPORT_GOLDEN=rapport.md cargo test -p poe2-core --test golden   # rapport pour la CI
cargo run -p poe2-core --example trace -- tests/golden/moteur/sort-feu-nominal-001.yaml
```
