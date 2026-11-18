"""Texte des mods reconstruit depuis les traductions de stats du jeu (RePoE `stat_translations`).

Depuis RePoE 0.5.5.10 (18/11/2026), `mods.min.json` ne remplit plus le champ `text` des mods. Ce
module le reconstruit comme le faisait RePoE, à partir de `stat_translations/stat_descriptions.min.json`
(les fichiers de description de stats du client) :

- pour chaque entrée de traduction qui porte au moins une stat du mod, on prend les valeurs `[min, max]`
  de ses stats (0 pour une stat absente) ;
- on choisit la première variante anglaise dont les conditions sont remplies par les valeurs ;
- on applique les `index_handlers` (`negate`, `per_minute_to_per_second`…), puis on remplace `{i}` par
  la valeur, ou `(min-max)` pour une plage, avec le signe `+` quand le format est `+#` ;
- une ligne par entrée, dans l'ordre des stats du mod (celui de l'ancien champ `text`) ; une plage
  inversée par `negate` reste dans l'ordre min-max d'origine (`(30-15)% reduced`), comme avant.

Le texte garde les balises d'affichage (`[Cold]`, `[Attack|Attacks]`) : `import_repoe.clean_text` les
retire ensuite, exactement comme pour l'ancien champ `text`.

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import math

# Diviseur et nombre de décimales maximal de chaque transformation connue. `None` = garder les décimales
# utiles (au plus 2), un entier = arrondi à ce nombre de décimales.
_DIVISIONS: dict[str, tuple[float, int | None]] = {
    "divide_by_one_hundred": (100, None),
    "divide_by_one_hundred_1dp": (100, 1),
    "divide_by_one_hundred_2dp": (100, 2),
    "divide_by_one_hundred_2dp_if_required": (100, None),
    "per_minute_to_per_second": (60, 1),
    "per_minute_to_per_second_0dp": (60, 0),
    "per_minute_to_per_second_1dp": (60, 1),
    "per_minute_to_per_second_2dp": (60, 2),
    "per_minute_to_per_second_2dp_if_required": (60, None),
    "milliseconds_to_seconds": (1000, None),
    "milliseconds_to_seconds_0dp": (1000, 0),
    "milliseconds_to_seconds_1dp": (1000, 1),
    "milliseconds_to_seconds_2dp": (1000, 2),
    "milliseconds_to_seconds_2dp_if_required": (1000, None),
    "deciseconds_to_seconds": (10, None),
    "divide_by_ten_0dp": (10, 0),
    "divide_by_ten_1dp": (10, 1),
    "divide_by_ten_1dp_if_required": (10, None),
    "divide_by_two_0dp": (2, 0),
    "divide_by_five": (5, None),
    "divide_by_fifteen_0dp": (15, 0),
    "divide_by_fifty": (50, None),
}
_SIMPLES = {
    "negate": lambda v: -v,
    "double": lambda v: v * 2,
    "negate_and_double": lambda v: -2 * v,
    "times_twenty": lambda v: v * 20,
    "add_one": lambda v: v + 1,
    "subtract_one": lambda v: v - 1,
    "plus_two_hundred": lambda v: v + 200,
    "divide_by_one_hundred_and_negate": lambda v: -v / 100,
}


class TraductionInconnue(Exception):
    """Transformation de valeur que ce module ne sait pas appliquer."""


def _appliquer(handlers: list[str], v: float) -> tuple[float, int | None]:
    decimales: int | None = 0
    for h in handlers:
        if h in _SIMPLES:
            v = _SIMPLES[h](v)
            if h == "divide_by_one_hundred_and_negate":
                decimales = None
        elif h in _DIVISIONS:
            div, decimales = _DIVISIONS[h]
            v = v / div
        elif h == "canonical_stat":
            pass
        else:
            raise TraductionInconnue(h)
    return v, decimales


def _nombre(v: float, decimales: int | None) -> str:
    if decimales is None:
        v = round(v, 2)
    else:
        v = round(v, decimales)
    if math.isclose(v, round(v)):
        return str(round(v))
    return f"{v:.2f}".rstrip("0").rstrip(".")


def _condition_ok(cond: dict, v: float) -> bool:
    ok = ("min" not in cond or v >= cond["min"]) and ("max" not in cond or v <= cond["max"])
    return not ok if cond.get("negated") else ok


def _variante(variantes: list[dict], valeurs: list[tuple[int, int]]) -> dict | None:
    """Première variante dont les conditions tiennent pour le max (puis le min) de chaque valeur."""
    for borne in (1, 0):
        for v in variantes:
            conds = v.get("condition") or []
            if all(_condition_ok(c or {}, valeurs[i][borne]) for i, c in enumerate(conds) if i < len(valeurs)):
                return v
    return None


def _texte_valeur(fmt: str, handlers: list[str], mini: int, maxi: int) -> str:
    a, dec_a = _appliquer(handlers, mini)
    b, dec_b = _appliquer(handlers, maxi)
    sa, sb = _nombre(a, dec_a), _nombre(b, dec_b)
    texte = sa if sa == sb else f"({sa}-{sb})"
    if fmt == "+#" and a >= 0:
        texte = "+" + texte
    return texte


class Traducteur:
    """Traduit les stats d'un mod en texte, d'après une ou plusieurs tables `stat_descriptions`."""

    def __init__(self, *tables: list[dict]):
        self.entrees: list[dict] = []
        self.par_stat: dict[str, list[int]] = {}
        for table in tables:
            for entree in table:
                rang = len(self.entrees)
                self.entrees.append(entree)
                for sid in entree.get("ids") or []:
                    self.par_stat.setdefault(sid, []).append(rang)

    def texte(self, stats: list[dict]) -> str | None:
        """Texte du mod (une ligne par entrée), ou `None` si une stat n'a aucune traduction."""
        valeurs = {s["id"]: (s["min"], s["max"]) for s in stats}
        rangs: list[int] = []
        for sid in valeurs:
            trouves = self.par_stat.get(sid)
            if not trouves:
                return None
            if not any(r in rangs for r in trouves):
                rangs.append(trouves[0])
        lignes = []
        for rang in rangs:
            entree = self.entrees[rang]
            ids = entree["ids"]
            vals = [valeurs.get(i, (0, 0)) for i in ids]
            variante = _variante(entree.get("English") or [], vals)
            if variante is None:
                continue  # le jeu n'affiche rien pour ces valeurs
            ligne = variante["string"]
            formats = variante.get("format") or []
            handlers = variante.get("index_handlers") or []
            for i, (mini, maxi) in enumerate(vals):
                if i >= len(formats) or formats[i] == "ignore":
                    continue
                h = handlers[i] if i < len(handlers) else []
                try:
                    valeur = _texte_valeur(formats[i], h, mini, maxi)
                except TraductionInconnue:
                    return None
                ligne = ligne.replace("{" + str(i) + "}", valeur)
            lignes.extend(ligne.split("\n"))
        return "\n".join(lignes)
