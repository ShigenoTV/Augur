# Kit « mise à jour patch 1.0 »

Procédure pour remettre Augur d'aplomb après le méga patch de la 1.0 (notes vers le 4 décembre 2026,
sortie le 11 décembre 2026), sans rien oublier et sans afficher de chiffre faux entre-temps. Elle s'appuie
sur la commande unique de [mise à jour des données](donnees-de-jeu.md#mise-à-jour-après-un-patch-commande-unique).

Règles qui ne changent pas :

- jamais de poids de mod inventé : un mod sans poids reste « probabilité indisponible » ;
- aucune source n'est affichée dans l'appli, l'attribution reste dans la doc ;
- rien n'est fusionné ni publié sans le feu vert de Max.

## Les outils du kit

| Outil | Ce qu'il fait |
|---|---|
| `python tools/gamedata/mise_a_jour.py --essai` | Télécharge RePoE et l'arbre PoB, régénère tout dans un dossier temporaire et écrit le rapport des différences, sans toucher au dépôt. |
| `python tools/gamedata/mise_a_jour.py [--arbre 1_0] [--coe]` | Même chose, puis remplace les fichiers de données du dépôt. |
| Workflow GitHub « Mise à jour des données » | La même commande sur GitHub : rapport dans le résumé du run, fichiers en artefact (ne pousse rien). |
| `python tools/gamedata/cas_mesures.py [--version 1.0.0.1]` | Liste les cas golden mesurés en jeu sur une version plus ancienne que les données : ce sont les mesures à refaire. La CI affiche les mêmes avertissements (sans échouer). |
| `python tools/theorycraft/coe_controle.py` | Contrôle que nos pools de mods et ceux de Craft of Exile coïncident (après un nouveau relevé de poids). |

Chaque fichier de données note maintenant sa version : `repoe_version` dans `gemmes.json` (sorts) et
`forge.json.gz` (mods, bases), `version` dans `arbre.json`, dates de relevé des poids dans `forge.json.gz`.
Le rapport les affiche en tête.

### Ce que le rapport signale tout seul

- un sort vérifié en jeu (Arc, Lightning Bolt) modifié ou retiré ;
- une nouvelle version d'arbre (import des codes PoB à vérifier) ;
- les cas golden mesurés sur une version majeure plus ancienne (0.5 → 1.0) : à re-mesurer ;
- les nouveaux mods sans poids, sans texte (le parseur ne les reconnaîtrait pas) ou sans stat de trade ;
- un mod retiré encore visé par une Essence (la commande s'arrête alors avant d'écrire quoi que ce soit).

Si une étape échoue (format RePoE changé, par exemple), la commande s'arrête avec le message de l'outil et
aucun fichier du dépôt n'est modifié.

## Répétition générale du 18 novembre 2026

La commande a été lancée sur le dernier export RePoE (0.5.5.10). Elle a trouvé une vraie panne : depuis
0.5.5.10, RePoE ne remplit plus le texte des mods (`text` vide dans `mods.min.json`). Sans correction, les
1 742 mods de Forge auraient perdu leur texte et les 257 stats de trade auraient disparu.

Correction : `tools/gamedata/traduction_stats.py` reconstruit le texte depuis les traductions de stats du
jeu (`stat_translations/stat_descriptions.min.json` de RePoE), comme le faisait RePoE. Sur les 1 742 mods de
Forge qui avaient un texte, les 1 742 textes reconstruits sont identiques à l'ancien champ. Garde-fou :
l'import s'arrête si plus de 5 % des mods n'ont pas de texte.

Après correction, les données régénérées depuis RePoE 0.5.5.10 sont identiques au contenu précédent (seules
les versions notées changent). Elles sont embarquées.

## Calendrier

### D'ici fin novembre : préparation

- [x] Répétition générale (ci-dessus).
- [ ] Relancer `mise_a_jour.py --essai` une fois par semaine (la vérif RePoE du samedi signale déjà les
      nouvelles versions) pour repérer tôt un nouveau changement de format.
- [ ] Préparer la session de mesures 1.0 avec Max (fiche plus bas).

### 4-5 décembre : notes du patch

- [ ] Lire les notes du méga patch (la veille du jeudi tombe le 3, la veille du patch : prévoir un passage
      exceptionnel) et remplir le tableau d'impact dans `/mnt/project-files/veille/` :

| Module | Ce qui peut casser | Où regarder |
|---|---|---|
| Copilot | Sorts rééquilibrés (Arc, Lightning Bolt, Spell Totem, Zenith), pénalité de résistance, qualité de l'ES | `gemmes.json`, `tests/golden/archetype/` |
| Copilot | Nouvel arbre passif, format des codes PoB | `arbre.json`, import PoB |
| Forge | Mods, bases, Essences, règles des monnaies et des Omens, paliers Greater / Perfect | `forge.json.gz`, `crates/poe2-core/src/forge/catalogue.rs` |
| Forge | Nouvelles monnaies sans prix | `crates/poe2-core/src/forge/prix.rs`, poe.ninja |
| Ledger | Nouvelles zones, format des lignes de `Client.txt` | `crates/poe2-core/src/ledger/zones.rs` |
| Parseur | Texte des objets copiés (Ctrl+C / Ctrl+Alt+C) | `crates/poe2-core/src/parser/` |

### 11-13 décembre : sortie

1. Dès que RePoE publie l'export 1.0 (souvent 1 à 3 jours après la sortie) :
   `python tools/gamedata/mise_a_jour.py --essai`, lire le rapport, puis sans `--essai`.
   Si Path of Building a publié l'arbre `1_0`, il est pris automatiquement (sinon `--arbre 1_0` plus tard).
2. `cargo test -p poe2-core` et `python -m pytest -q tools/gamedata/tests` : tout doit passer.
   Les cas mesurés en 0.5 passent encore (ils testent les formules), mais `cas_mesures.py` les liste : ils
   attendent la fiche de mesures.
3. PR de données avec le rapport en description, puis release sur le « go » de Max.
4. Prix : choisir la nouvelle ligue dans Réglages, vérifier que poe.ninja la connaît et que les monnaies
   du catalogue de Forge ont un prix.
5. Ledger : une vraie session de 2 ou 3 maps pour vérifier le classement des zones.
6. Parseur : Max envoie quelques objets 1.0 copiés par Ctrl+C et Ctrl+Alt+C (via la session de mesures).

### 12-20 décembre : après stabilisation

- [ ] Poids : nouveau relevé quand poe2db et Craft of Exile ont mis à jour
      (`mise_a_jour.py --coe`, puis `--poe2db`, lent), puis `coe_controle.py`.
- [ ] Règles des monnaies : relire l'émulateur de Craft of Exile comme le 8 novembre.
- [ ] Fiche de mesures 1.0 avec Max (ci-dessous), puis passer les cas golden en `patch: "1.0…"`.

## Fiche de mesures 1.0 (20 à 30 minutes en jeu)

À faire avec la **session de mesures** de l'overlay ouverte (fenêtre principale > Session de mesures >
Démarrer), pour que les objets copiés et les codes PoB soient enregistrés. Les panneaux du personnage et des
compétences ne sont pas lisibles par l'overlay : une capture d'écran suffit.

| Priorité | Mesure | Cas golden concernés |
|---|---|---|
| 1 | Panneau d'information d'Arc (DPS, coup moyen, critique, cadence), avec puis sans Urgent Totems III, avec puis sans Zenith II, mana plein et mana bas | `arc-panneau-mesure.yaml` (7 cas) |
| 2 | Résistances feu et chaos du panneau, avec le code PoB du moment (pénalité de fin de campagne, toujours −60 % ?) | `resistance-feu-mesuree.yaml`, `resistance-chaos-mesuree.yaml` |
| 3 | Energy Shield de 4 objets (casque, torse, gants, bottes) : Ctrl+Alt+C sur chaque objet et valeur affichée | `es-objets-mesures.yaml` (4 cas) |
| 4 | Nombre de chaînes d'Arc aux niveaux 22 et 32, et avec Dominus' Grasp | `arc-chaines-mesurees.yaml` (3 cas) |
| 5 | Un ailment (ignite) sur le panneau, si le temps le permet | à créer |

Les inconnues I-09, I-10, I-11 et I-15 du [registre](../theorycraft/registre-des-inconnues.md) restent
secondaires ; I-17 seulement si une source d'infusion apparaît.

## Idées pas encore lancées (à valider par Max)

- Alerte dans l'appli quand le parseur ou Forge voit beaucoup de mods ou de sorts inconnus : « données
  d'avant la 1.0, mise à jour en cours » au lieu d'un chiffre faux.
- Version des données affichée dans la barre latérale.
- Plan B si RePoE tarde : extraire les tables du jeu sur le PC de Max.
