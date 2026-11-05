#!/usr/bin/env python3
"""Poids de mods relevés sur poe2db.tw, rapprochés des identifiants RePoE.

Les poids réels ne sont pas dans les fichiers du jeu (RePoE ne donne que 0/1).
poe2db les publie (champ `DropChance` des pages « Modifiers » de chaque classe
d'objet), compilés par Krakenbul (Prohibited Library) à partir d'essais de
recombinaison, ou relevés sur le site d'échange pour les bases non recombinables
(page https://poe2db.tw/us/weightings). Ce sont donc des ESTIMATIONS d'un tiers,
sous licence CC BY-NC-SA 3.0 sauf mention contraire : elles ne sont pas versées
au dépôt. Le cache HTML et les fichiers produits restent en local (`var/`).

Deux étapes :

    # 1. Télécharger les pages (lentement : une requête toutes les 6 s par
    #    défaut, arrêt net sur un refus ou une page anti-robot).
    python3 tools/theorycraft/poe2db_poids.py telecharger --cache var/poe2db

    # 2. Extraire les poids et les rapprocher de RePoE (mods.min.json), une
    #    sortie par page : var/poids/Body_Armours_int.json, etc.
    python3 tools/theorycraft/poe2db_poids.py poids --cache var/poe2db \\
        --repoe-mods mods.min.json --out var/poids

Chaque sortie est `{"source", "page", "poids": {id_RePoE: poids}, "inconnus",
"non_rapproches"}` ; `craft_proba.py` la lit directement. Un `DropChance`
entier égal à 1 (au lieu d'une chaîne) est le marqueur poe2db d'un poids non
mesuré : le mod va dans `inconnus`, jamais dans `poids`.

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

BASE_URL = "https://poe2db.tw/us/"
USER_AGENT = "Augur-theorycraft/0.1 (+https://github.com/ShigenoTV/Augur)"
DELAI_S = 6.0

# Indices d'une page de vérification anti-robot : on s'arrête, sans contourner.
ANTI_ROBOT = (
    "cf-challenge",
    "challenge-platform",
    "Just a moment...",
    "Attention Required",
    "anubis",
    "captcha",
)

GENRES = {"1": "prefix", "2": "suffix"}
PAGE_MODS = re.compile(r'href="/us/([A-Za-z0-9_]+)#ModifiersCalc"')
BALISE = re.compile(r"<[^>]+>")
CROCHET = re.compile(r"\[([^\]|]+)(?:\|([^\]]+))?\]")


class Arret(RuntimeError):
    """Refus du site ou page anti-robot : on arrête tout et on prévient."""


# --- Téléchargement ---------------------------------------------------------


def pages_de_mods(html_modifiers: str) -> list[str]:
    """Pages « Modifiers » des classes d'objets, dans l'ordre de la page."""
    vues, res = set(), []
    for slug in PAGE_MODS.findall(html_modifiers):
        if slug not in vues:
            vues.add(slug)
            res.append(slug)
    return res


def verifier_page(statut: int, texte: str, url: str) -> None:
    if statut != 200:
        raise Arret(f"{url} : statut HTTP {statut}, arrêt")
    if "ModsView(" in texte or "#ModifiersCalc" in texte:
        return  # vraie page poe2db
    bas = texte[:20000].lower()
    for indice in ANTI_ROBOT:
        if indice.lower() in bas:
            raise Arret(f"{url} : vérification anti-robot détectée ({indice!r}), arrêt")
    raise Arret(f"{url} : page inattendue (format poe2db changé ?), arrêt")


def lire_url(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            texte = r.read().decode("utf-8")
            statut = r.status
    except urllib.error.HTTPError as e:
        raise Arret(f"{url} : statut HTTP {e.code}, arrêt") from e
    verifier_page(statut, texte, url)
    return texte


def telecharger(cache: Path, delai: float = DELAI_S, lire=lire_url, dormir=time.sleep):
    """Télécharge la page d'index puis chaque page de classe absente du cache."""
    cache.mkdir(parents=True, exist_ok=True)
    index = cache / "Modifiers.html"
    if not index.exists():
        index.write_text(lire(BASE_URL + "Modifiers"), encoding="utf-8")
        dormir(delai)
    slugs = pages_de_mods(index.read_text(encoding="utf-8"))
    for i, slug in enumerate(slugs):
        f = cache / f"{slug}.html"
        if f.exists():
            continue
        texte = lire(BASE_URL + slug)
        if "ModsView(" not in texte:
            raise Arret(f"{slug} : pas de bloc ModsView, format poe2db changé ? arrêt")
        f.write_text(texte, encoding="utf-8")
        print(f"[{i + 1}/{len(slugs)}] {slug}", file=sys.stderr)
        dormir(delai)
    return slugs


# --- Extraction et rapprochement --------------------------------------------


def extraire_modsview(texte: str) -> dict:
    """Objet JSON passé à `ModsView(...)` dans une page de classe poe2db."""
    i = texte.index("ModsView({") + len("ModsView(")
    obj, _ = json.JSONDecoder().raw_decode(texte[i:])
    return obj


def texte_poe2db(s: str) -> str:
    t = html.unescape(BALISE.sub("", s)).replace("—", "-")
    return re.sub(r"\s+", " ", t).strip()


def texte_repoe(s: str) -> str:
    t = CROCHET.sub(lambda m: m.group(2) or m.group(1), s or "")
    return re.sub(r"\s+", " ", t.replace("\n", " ")).strip()


def cle_repoe(mid: str, m: dict) -> tuple:
    return (
        m.get("name", ""),
        int(m.get("required_level", 0)),
        m.get("generation_type"),
        tuple(m.get("groups", [])),
        tuple(s["tag"] for s in m.get("spawn_weights", [])),
    )


def cle_poe2db(e: dict) -> tuple:
    return (
        e["Name"],
        int(e["Level"]),
        GENRES.get(str(e["ModGenerationTypeID"])),
        tuple(e.get("ModFamilyList", [])),
        tuple(e.get("spawn_no", [])),
    )


def index_repoe(mods: dict) -> dict[tuple, list[str]]:
    idx: dict[tuple, list[str]] = {}
    for mid, m in mods.items():
        if m.get("generation_type") in ("prefix", "suffix"):
            idx.setdefault(cle_repoe(mid, m), []).append(mid)
    return idx


def rapprocher(entrees: list[dict], mods: dict, idx=None) -> dict:
    """Associe chaque entrée poe2db à un identifiant RePoE et sépare les inconnus."""
    idx = idx if idx is not None else index_repoe(mods)
    poids, inconnus, non_rapproches = {}, [], []
    for e in entrees:
        ids = idx.get(cle_poe2db(e), [])
        if len(ids) > 1:
            t = texte_poe2db(e.get("str", ""))
            ids = [i for i in ids if texte_repoe(mods[i].get("text", "")) == t]
        if len(ids) != 1:
            non_rapproches.append(
                {
                    "nom": e["Name"],
                    "niveau": int(e["Level"]),
                    "texte": texte_poe2db(e.get("str", "")),
                }
            )
            continue
        mid = ids[0]
        dc = e.get("DropChance")
        if not isinstance(dc, str):  # entier 1 : poids non mesuré par poe2db
            inconnus.append(mid)
            continue
        poids[mid] = int(dc)
    return {
        "poids": dict(sorted(poids.items())),
        "inconnus": sorted(inconnus),
        "non_rapproches": non_rapproches,
    }


def poids_page(texte: str, mods: dict, idx=None, sections=("normal",)) -> dict:
    vue = extraire_modsview(texte)
    entrees = [e for s in sections for e in vue.get(s, [])]
    res = rapprocher(entrees, mods, idx)
    res["etiquette"] = vue.get("opt", {}).get("tags")
    return res


# --- Ligne de commande ------------------------------------------------------


def _principal(argv=None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sp = p.add_subparsers(dest="cmd", required=True)
    t = sp.add_parser("telecharger", help="télécharger les pages poe2db (lent)")
    t.add_argument("--cache", type=Path, required=True)
    t.add_argument("--delai", type=float, default=DELAI_S)
    w = sp.add_parser("poids", help="extraire et rapprocher les poids")
    w.add_argument("--cache", type=Path, required=True)
    w.add_argument("--repoe-mods", type=Path, required=True)
    w.add_argument("--out", type=Path, required=True)
    a = p.parse_args(argv)

    if a.cmd == "telecharger":
        if a.delai < 3:
            p.error("--delai : 3 s minimum, pour ménager poe2db")
        try:
            telecharger(a.cache, a.delai)
        except Arret as e:
            print(f"ARRÊT : {e}", file=sys.stderr)
            return 2
        return 0

    mods = json.loads(a.repoe_mods.read_text(encoding="utf-8"))
    idx = index_repoe(mods)
    a.out.mkdir(parents=True, exist_ok=True)
    total = {"poids": 0, "inconnus": 0, "non_rapproches": 0}
    for f in sorted(a.cache.glob("*.html")):
        if f.stem == "Modifiers":
            continue
        res = poids_page(f.read_text(encoding="utf-8"), mods, idx)
        sortie = {
            "source": "poe2db.tw (estimations Krakenbul)",
            "page": BASE_URL + f.stem,
            **res,
        }
        (a.out / f"{f.stem}.json").write_text(
            json.dumps(sortie, ensure_ascii=False, indent=1, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        for k in total:
            total[k] += len(res[k])
        print(
            f"{f.stem}: {len(res['poids'])} poids, {len(res['inconnus'])} inconnus, "
            f"{len(res['non_rapproches'])} non rapprochés"
        )
    print(f"TOTAL : {total}")
    return 0


if __name__ == "__main__":
    sys.exit(_principal())
