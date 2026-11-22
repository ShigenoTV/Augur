#!/usr/bin/env python3
"""Contrôle des monnaies de Forge (PoE 2) contre poe.ninja, avec rapport Markdown.

À lancer après une mise à jour du jeu (en particulier la 1.0) ou un changement de ligue. Le
contrôle lit les pages « échange » de poe.ninja que Forge interroge déjà (Currency, Ritual,
Essences, Verisium, Abyss) pour une ligue, et signale :

1. les monnaies, Omens et os de Forge **sans cote** dans la ligue (identifiant poe.ninja disparu
   ou renommé, ou objet pas encore échangé) : Forge retombe alors sur le prix indicatif ou sur la
   saisie du joueur ;
2. les Essences et Alloys de Forge sans cote : sans prix, Forge ne les propose pas ;
3. les **nouveaux objets** cotés par poe.ninja que Forge ne connaît pas et qui ne sont pas dans la
   liste des objets déjà vus et écartés (`monnaies-hors-forge.json`) : une nouvelle monnaie de
   craft à modéliser, ou un objet à ajouter à cette liste.

Il ne modifie rien dans le dépôt, sauf avec `--referencer`, qui remplace la liste des objets déjà
vus par ceux de la ligue contrôlée (après avoir relu le rapport).

Utilisation (depuis la racine du dépôt) :

    python tools/gamedata/controle_monnaies.py                     # ligue courante de poe.ninja
    python tools/gamedata/controle_monnaies.py --ligue "Standard"
    python tools/gamedata/controle_monnaies.py --depuis DOSSIER    # Currency.json… déjà téléchargés
    python tools/gamedata/controle_monnaies.py --referencer        # met à jour la liste des déjà vus

Code de sortie : 0 sans point d'attention, 1 s'il y en a, 2 si poe.ninja est injoignable.
Requêtes : la liste des ligues puis une page par catégorie, à une seconde d'intervalle.

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import datetime as dt
import gzip
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ICI = Path(__file__).resolve().parent
RACINE = ICI.parents[1]
NINJA_RS = RACINE / "crates" / "poe2-core" / "src" / "forge" / "ninja.rs"
FORGE = RACINE / "crates" / "poe2-core" / "data" / "forge.json.gz"
DEJA_VUS = ICI / "monnaies-hors-forge.json"

API = "https://poe.ninja/poe2/api/economy"
USER_AGENT = "Augur-donnees/0.1 (+https://github.com/ShigenoTV/Augur)"
# Catégories lues par Forge (`ninja::CATEGORIES`) ; Essences et Alloys sont repris en bloc.
CATEGORIES = ["Currency", "Ritual", "Essences", "Verisium", "Abyss"]
CATEGORIES_EN_BLOC = {"Essences", "Verisium"}


class Injoignable(Exception):
    pass


def sources_forge(texte: str) -> dict[str, list[tuple[str, str]]]:
    """Table `SOURCES` de ninja.rs : catégorie -> [(identifiant Forge, identifiant poe.ninja)]."""
    constantes = dict(re.findall(r'pub const (CATEGORIE_\w+): &str = "([^"]+)";', texte))
    debut = texte.index("pub const SOURCES")
    fin = texte.index("];", debut)
    bloc = texte[debut:fin]
    out: dict[str, list[tuple[str, str]]] = {}
    motif = r'\(\s*(?:"([^"]+)"|super::prix::DIVINE)\s*,\s*(?:"([^"]+)"|(CATEGORIE_\w+))\s*,\s*"([^"]+)"\s*,?\s*\)'
    for id_forge, cat, const, id_ninja in re.findall(motif, bloc):
        categorie = cat or constantes[const]
        out.setdefault(categorie, []).append((id_forge or "divine", id_ninja))
    if not out:
        raise ValueError("table SOURCES introuvable dans ninja.rs")
    return out


def essences_forge(chemin: Path) -> dict[str, str]:
    """Essences et Alloys de Forge : identifiant (celui de poe.ninja) -> nom."""
    donnees = json.loads(gzip.decompress(chemin.read_bytes()))
    return {e["id"]: e.get("nom", e["id"]) for e in donnees["essences"]}


def _get(url: str) -> object:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return json.loads(r.read())
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as e:
        raise Injoignable(f"{url} : {e}") from e


def ligue_courante() -> str:
    """Première ligue de poe.ninja, comme l'appli (`ninja::premiere_ligue`)."""
    ligues = _get(f"{API}/leagues")
    if not isinstance(ligues, list) or not ligues:
        raise Injoignable("liste des ligues vide")
    return ligues[0]["id"]


def telecharger(ligue: str) -> dict[str, dict]:
    pages = {}
    for i, cat in enumerate(CATEGORIES):
        if i:
            time.sleep(1)
        q = urllib.parse.urlencode({"league": ligue, "type": cat})
        pages[cat] = _get(f"{API}/exchange/current/overview?{q}")
    return pages


def lire_dossier(dossier: Path) -> dict[str, dict]:
    return {cat: json.loads((dossier / f"{cat}.json").read_text(encoding="utf-8")) for cat in CATEGORIES}


def _cotes(page: dict) -> set[str]:
    """Identifiants qui ont une cote exploitable (valeur > 0), comme `ninja::extraire`."""
    out = set()
    for l in page.get("lines") or []:
        v = l.get("primaryValue")
        if isinstance(v, (int, float)) and v > 0 and l.get("id"):
            out.add(l["id"])
    return out


def _noms(page: dict) -> dict[str, str]:
    items = page.get("items") or (page.get("core") or {}).get("items") or []
    return {i["id"]: i.get("name", i["id"]) for i in items if i.get("id")}


def controler(pages: dict[str, dict], sources: dict, essences: dict[str, str], deja_vus: dict,
              sans_cote_connus: set[str] = frozenset()) -> dict:
    """Compare les pages poe.ninja à Forge. Ne fait aucune entrée-sortie.

    `sans_cote_connus` : identifiants Forge que poe.ninja ne cote déjà pas (prix à saisir), rangés
    à part pour ne pas les signaler à chaque contrôle."""
    res = {"sans_cote": [], "sans_cote_connus": [], "essences_sans_cote": [], "nouveaux": [],
           "pages_vides": []}
    noms: dict[str, str] = {}
    for cat in CATEGORIES:
        noms.update(_noms(pages[cat]))
        if not pages[cat].get("lines"):
            res["pages_vides"].append(cat)
    # 1. Monnaies, Omens, os de Forge sans cote.
    for cat, paires in sources.items():
        cotes = _cotes(pages.get(cat, {}))
        for id_forge, id_ninja in paires:
            if id_ninja not in cotes:
                cle = "sans_cote_connus" if id_forge in sans_cote_connus else "sans_cote"
                res[cle].append((cat, id_forge, id_ninja))
    # 2. Essences et Alloys sans cote (poe.ninja range des Alloys dans Verisium).
    cotes_bloc = set().union(*(_cotes(pages[c]) for c in CATEGORIES_EN_BLOC))
    res["essences_sans_cote"] = sorted((i, n) for i, n in essences.items() if i not in cotes_bloc)
    # 3. Objets cotés inconnus de Forge et jamais vus.
    for cat in CATEGORIES:
        connus = set(essences) if cat in CATEGORIES_EN_BLOC else {n for _, n in sources.get(cat, [])}
        vus = set(deja_vus.get(cat, []))
        for id_ninja in sorted(_cotes(pages[cat])):
            if id_ninja not in connus and id_ninja not in vus:
                res["nouveaux"].append((cat, id_ninja, noms.get(id_ninja, id_ninja)))
    return res


def hors_forge(pages: dict[str, dict], sources: dict, essences: dict[str, str]) -> dict[str, list[str]]:
    """Objets cotés que Forge ne connaît pas, par catégorie (pour `--referencer`)."""
    out = {}
    for cat in CATEGORIES:
        connus = set(essences) if cat in CATEGORIES_EN_BLOC else {n for _, n in sources.get(cat, [])}
        out[cat] = sorted(i for i in _cotes(pages[cat]) if i not in connus)
    return out


def nb_points(res: dict) -> int:
    return len(res["sans_cote"]) + len(res["nouveaux"]) + len(res["pages_vides"])


def rapport(res: dict, ligue: str, date: str) -> str:
    l = [f"# Contrôle des monnaies de Forge du {date}", "", f"Ligue poe.ninja : **{ligue}**", ""]
    l += ["| Contrôle | Nombre |", "|---|---:|"]
    l += [f"| Monnaies, Omens et os de Forge sans cote | {len(res['sans_cote'])} |"]
    l += [f"| Essences et Alloys de Forge sans cote | {len(res['essences_sans_cote'])} |"]
    l += [f"| Nouveaux objets cotés inconnus de Forge | {len(res['nouveaux'])} |", ""]
    l += ["## Points d'attention", ""]
    if not nb_points(res):
        l += ["Aucun.", ""]
    for cat in res["pages_vides"]:
        l.append(f"- Page **{cat}** vide : la ligue n'a peut-être pas encore de prix (début de ligue).")
    if res["sans_cote"]:
        l.append("- Monnaies de Forge sans cote : Forge utilise le prix indicatif ou la saisie du joueur. "
                 "Si l'objet existe toujours, vérifier son identifiant poe.ninja dans `forge/ninja.rs` (`SOURCES`).")
    if res["nouveaux"]:
        l.append("- Nouveaux objets : une monnaie de craft à ajouter au catalogue de Forge (règle, prix), "
                 "ou un objet sans rapport avec le craft d'affixes à ajouter à `monnaies-hors-forge.json` "
                 "(`--referencer`).")
    l.append("")
    if res["sans_cote"]:
        l += ["## Monnaies, Omens et os sans cote", "", "| Catégorie | Forge | poe.ninja |", "|---|---|---|"]
        l += [f"| {c} | `{f}` | `{n}` |" for c, f, n in res["sans_cote"]] + [""]
    if res["nouveaux"]:
        l += ["## Nouveaux objets cotés", "", "| Catégorie | Objet | Identifiant poe.ninja |", "|---|---|---|"]
        l += [f"| {c} | {nom} | `{i}` |" for c, i, nom in res["nouveaux"]] + [""]
    if res["sans_cote_connus"]:
        l += ["## Déjà sans cote au contrôle précédent", "",
              "Prix à saisir par le joueur (poe.ninja ne les cote pas) : "
              + ", ".join(f"`{f}`" for _, f, _ in res["sans_cote_connus"]) + ".", ""]
    if res["essences_sans_cote"]:
        l += ["## Essences et Alloys sans cote", "",
              "Pas de prix, donc pas proposés par Forge tant que le joueur ne saisit pas de prix. "
              "Souvent des objets peu échangés : à surveiller seulement si la liste grossit d'un coup.", ""]
        l += [", ".join(n for _, n in res["essences_sans_cote"]), ""]
    return "\n".join(l)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--ligue", help="ligue poe.ninja (par défaut : la ligue courante)")
    p.add_argument("--depuis", type=Path, help="dossier avec Currency.json, Ritual.json… (sans réseau)")
    p.add_argument("--rapport", type=Path, help="fichier du rapport (sinon affiché)")
    p.add_argument("--referencer", action="store_true",
                   help="remplace la liste des objets déjà vus par ceux de cette ligue")
    p.add_argument("--date", default=dt.date.today().isoformat())
    a = p.parse_args(argv)

    sources = sources_forge(NINJA_RS.read_text(encoding="utf-8"))
    essences = essences_forge(FORGE)
    deja = json.loads(DEJA_VUS.read_text(encoding="utf-8")) if DEJA_VUS.exists() else {}
    try:
        if a.depuis:
            ligue, pages = a.ligue or "(fichiers locaux)", lire_dossier(a.depuis)
        else:
            ligue = a.ligue or ligue_courante()
            pages = telecharger(ligue)
    except Injoignable as e:
        print(f"poe.ninja injoignable : {e}", file=sys.stderr)
        return 2

    res = controler(pages, sources, essences, deja.get("categories", {}),
                    set(deja.get("sans_cote_connus", [])))
    texte = rapport(res, ligue, a.date)
    if a.rapport:
        a.rapport.parent.mkdir(parents=True, exist_ok=True)
        a.rapport.write_text(texte + "\n", encoding="utf-8")
        print(f"Rapport : {a.rapport}")
    else:
        print(texte)
    if a.referencer:
        DEJA_VUS.write_text(json.dumps({
            "commentaire": "Objets cotés par poe.ninja que Forge ne modélise pas, déjà relus. "
                           "Régénéré par controle_monnaies.py --referencer.",
            "ligue": ligue, "date": a.date,
            "categories": hors_forge(pages, sources, essences),
            "sans_cote_connus": sorted(f for _, f, _ in res["sans_cote"] + res["sans_cote_connus"]),
        }, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"Liste des objets déjà vus mise à jour : {DEJA_VUS}")
    print(f"{nb_points(res)} point(s) d'attention", file=sys.stderr)
    return 1 if nb_points(res) else 0


if __name__ == "__main__":
    sys.exit(main())
