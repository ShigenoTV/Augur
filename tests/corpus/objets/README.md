# Corpus d'objets

Textes d'objets tels que copiés en jeu (client anglais), utilisés par
`cargo test -p poe2-core --test corpus`.

- `*.txt` : un objet par fichier. Le suffixe `-avance` désigne une copie
  Ctrl+Alt+C.
- `invalides/` : textes qui **ne sont pas** des objets ; le parseur doit les
  refuser proprement.

> **Provenance.** Les fichiers `reel-*.txt` sont de **vrais objets** copiés
> en jeu par Max (client anglais, repris de poe2-craft-assistant le
> 31/10/2026). Sur ce client, Ctrl+C et Ctrl+Alt+C donnent le même texte,
> avec en-têtes de mods. Les autres exemples sont **reconstitués** d'après le format
> public du jeu, pas copiés d'une session réelle : noms, valeurs et paliers
> sont plausibles mais non vérifiés. L'objectif du sprint (≥ 500 objets réels,
> ≥ 99 % de réussite) demande une collecte en jeu : ajoutez simplement les
> fichiers `.txt` ici, le test calcule le taux de réussite sur tout le dossier.
> Ne collez jamais d'information personnelle (notes privées, noms de compte).

## PoE 1 (`poe1/`)

Objets de Path of Exile 1, **reconstitués** d'après le format public du jeu
(sockets, influences, en-têtes « Master Crafted » et « Shaper's », qualité de
catalyseur, cartes de divination). Ils servent au test `poe1` de
`tests/corpus.rs`. À compléter par de vraies copies faites en jeu sur PoE 1.
