# Corpus d'objets

Textes d'objets tels que copiés en jeu (client anglais), utilisés par
`cargo test -p poe2-core --test corpus`.

- `*.txt` : un objet par fichier. Le suffixe `-avance` désigne une copie
  Ctrl+Alt+C.
- `invalides/` : textes qui **ne sont pas** des objets ; le parseur doit les
  refuser proprement.

> **Provenance.** Les exemples actuels sont **reconstitués** d'après le format
> public du jeu, pas copiés d'une session réelle : noms, valeurs et paliers
> sont plausibles mais non vérifiés. L'objectif du sprint (≥ 500 objets réels,
> ≥ 99 % de réussite) demande une collecte en jeu : ajoutez simplement les
> fichiers `.txt` ici, le test calcule le taux de réussite sur tout le dossier.
> Ne collez jamais d'information personnelle (notes privées, noms de compte).
