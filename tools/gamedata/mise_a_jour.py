#!/usr/bin/env python3
"""Mise à jour des données de jeu en une commande, avec rapport des différences.

Enchaîne les outils existants, dans l'ordre :

1. télécharge RePoE PoE 2 (https://repoe-fork.github.io/poe2/ : skills.json, base_items.json,
   mods.min.json, base_items.min.json) et l'arbre passif de Path of Building PoE 2
   (dernière version d'arbre annoncée par `src/GameVersions.lua`, ou celle demandée) ;
2. en option, rafraîchit les poids et les Essences :
   `--coe` (Craft of Exile, deux fichiers, rapide) et `--poe2db` (relevé poe2db, LENT :
   une page toutes les 6 s, arrêt net au premier refus) ;
3. régénère `crates/poe2-core/data/gemmes.json`, `arbre.json` et `forge.json.gz`
   (`import_repoe.py`, `extract_build_data.py`, `forge_data.py`) ;
4. compare l'ancien et le nouveau contenu et écrit un rapport Markdown lisible : sorts, supports,
   armes, passifs, mods, bases, Essences et poids ajoutés, retirés ou modifiés, avec les points
   d'attention (sort vérifié en jeu modifié, nouvelle version d'arbre, mods sans poids).

Rien n'est écrit dans le dépôt si une étape échoue. Avec `--essai`, seul le rapport est écrit.

Utilisation (depuis la racine du dépôt) :

    python tools/gamedata/mise_a_jour.py                  # met à jour et écrit le rapport
    python tools/gamedata/mise_a_jour.py --essai          # rapport seulement
    python tools/gamedata/mise_a_jour.py --coe            # + poids et Essences Craft of Exile
    python tools/gamedata/mise_a_jour.py --depuis DOSSIER # fichiers déjà téléchargés, sans réseau

Rapport par défaut : `var/mise-a-jour/rapport-AAAA-MM-JJ.md` (hors dépôt, `var/` est ignoré).

Python 3.11+, bibliothèque standard seulement.
"""

from __future__ import annotations

import argparse
import contextlib
import datetime as dt
import gzip
import io
import json
import re
import shutil
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path

ICI = Path(__file__).resolve().parent
RACINE = ICI.parents[1]
THEORYCRAFT = RACINE / "tools" / "theorycraft"
sys.path[:0] = [str(ICI), str(THEORYCRAFT)]

import extract_build_data
import forge_data
import import_repoe

REPOE = "https://repoe-fork.github.io/poe2/"
POB = "https://raw.githubusercontent.com/PathOfBuildingCommunity/PathOfBuilding-PoE2/dev/src/"
COE = "https://beta.craftofexile.com/"
USER_AGENT = "Augur-donnees/0.1 (+https://github.com/ShigenoTV/Augur)"

FICHIERS_REPOE = ("skills.json", "base_items.json", "mods.min.json", "base_items.min.json")

# Fichiers régénérés : chemin dans le dépôt -> nom dans le dossier de travail.
EMBARQUES = {
    "crates/poe2-core/data/gemmes.json": "data/gemmes.json",
    "crates/poe2-core/data/arbre.json": "data/arbre.json",
    "crates/poe2-core/data/forge.json.gz": "data/forge.json.gz",
}
POIDS = {
    "tools/theorycraft/donnees/poids-poe2db.json": "theo/poids-poe2db.json",
    "tools/theorycraft/donnees/poids-coe.json": "theo/poids-coe.json",
    "tools/theorycraft/donnees/essences-coe.json": "theo/essences-coe.json",
}

# Sorts vérifiés en jeu : une modification rend les cas golden à rejouer.
NIVEAU_REFERENCE = "20"
LIMITE = 60


class Echec(Exception):
    """Une étape a échoué : rien n'est écrit dans le dépôt."""


# --- Téléchargement ---------------------------------------------------------


def lire_url(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return r.read()
    except (urllib.error.URLError, TimeoutError) as e:
        raise Echec(f"téléchargement impossible : {url} ({e})") from e


def version_repoe(page: str) -> str:
    m = re.search(r"PoE2 version ([0-9][0-9.]*[0-9])", page)
    if not m:
        raise Echec("version RePoE introuvable dans le titre de la page")
    return m.group(1)


def derniere_version_arbre(game_versions: str) -> str:
    m = re.search(r"treeVersionList\s*=\s*\{([^}]*)\}", game_versions)
    versions = re.findall(r'"(\d+_\d+)"', m.group(1)) if m else []
    if not versions:
        raise Echec("liste des versions d'arbre introuvable dans GameVersions.lua")
    return versions[-1]


def version_coe(page: str) -> str:
    m = re.search(r"json/poe2/([0-9][0-9.]*[0-9])/data\.json", page)
    if not m:
        raise Echec("version des données Craft of Exile introuvable")
    return m.group(1)


def telecharger(dossier: Path, arbre: str, coe: bool, lire=lire_url) -> dict:
    """Télécharge les sources dans `dossier` et y note leurs versions (`versions.json`)."""
    dossier.mkdir(parents=True, exist_ok=True)
    versions = {"repoe": version_repoe(lire(REPOE).decode("utf-8", "replace"))}
    for nom in FICHIERS_REPOE:
        (dossier / nom).write_bytes(lire(REPOE + nom))
    if arbre == "auto":
        arbre = derniere_version_arbre(lire(POB + "GameVersions.lua").decode("utf-8"))
    versions["arbre"] = arbre
    (dossier / "tree.json").write_bytes(lire(f"{POB}TreeData/{arbre}/tree.json"))
    if coe:
        v = version_coe(lire(COE + "?game=poe2").decode("utf-8", "replace"))
        versions["coe"] = v
        (dossier / "coe-data.json").write_bytes(lire(f"{COE}json/poe2/{v}/data.json"))
        (dossier / "coe-english.json").write_bytes(lire(f"{COE}json/poe2/{v}/localization/english.json"))
    (dossier / "versions.json").write_text(json.dumps(versions, indent=1) + "\n", encoding="utf-8")
    return versions


# --- Régénération -----------------------------------------------------------


def _lancer(etape: str, fonction, argv: list[str]) -> str:
    """Lance la fonction principale d'un outil ; sa sortie est gardée pour le journal."""
    sortie = io.StringIO()
    try:
        with contextlib.redirect_stdout(sortie), contextlib.redirect_stderr(sortie):
            code = fonction(argv)
    except SystemExit as e:  # argparse
        code = e.code
    except Exception as e:
        raise Echec(f"{etape} : {e}") from e
    if code:
        raise Echec(f"{etape} : échec ({sortie.getvalue().strip()})")
    return sortie.getvalue().strip()


def regenerer(
    sources: Path,
    travail: Path,
    versions: dict,
    date: str,
    poe2db_cache: Path | None = None,
    racine: Path = RACINE,
) -> list[str]:
    """Produit dans `travail` les fichiers régénérés ; renvoie le journal des étapes."""
    journal = []
    theo = travail / "theo"
    theo.mkdir(parents=True, exist_ok=True)
    for depot, nom in POIDS.items():
        shutil.copyfile(racine / depot, travail / nom)
    tables = travail / "gamedata"
    journal.append(
        _lancer(
            "import RePoE",
            import_repoe.main,
            [
                "--mods", str(sources / "mods.min.json"),
                "--bases", str(sources / "base_items.min.json"),
                "--out", str(tables),
                "--repoe-version", versions["repoe"],
            ],
        )
    )  # fmt: skip
    journal.append(
        _lancer(
            "sorts et arbre",
            extract_build_data.main,
            [
                "--skills", str(sources / "skills.json"),
                "--bases", str(sources / "base_items.json"),
                "--tree", str(sources / "tree.json"),
                "--tree-version", versions["arbre"],
                "--out", str(travail / "data"),
            ],
        )
    )  # fmt: skip
    if poe2db_cache is not None:
        import poe2db_poids

        def relever(_argv):
            poe2db_poids.telecharger(poe2db_cache)
            return 0

        journal.append(_lancer("relevé poe2db", relever, []))
        journal.append(
            _lancer(
                "poids poe2db",
                poe2db_poids._principal,
                [
                    "poids",
                    "--cache", str(poe2db_cache),
                    "--repoe-mods", str(sources / "mods.min.json"),
                    "--out", str(travail / "poe2db-pages"),
                    "--fichier", str(theo / "poids-poe2db.json"),
                    "--repoe-version", versions["repoe"],
                    "--date", date,
                ],
            )
        )  # fmt: skip
    if "coe" in versions:
        import coe_essences
        import coe_poids

        commun = [
            "--data", str(sources / "coe-data.json"),
            "--langue", str(sources / "coe-english.json"),
            "--date", date,
            "--version", versions["coe"],
        ]  # fmt: skip
        journal.append(
            _lancer(
                "poids Craft of Exile",
                coe_poids.principal,
                commun + ["--poe2db", str(theo / "poids-poe2db.json"), "--out", str(theo / "poids-coe.json")],
            )
        )
        journal.append(
            _lancer(
                "Essences Craft of Exile",
                coe_essences.principal,
                commun + ["--out", str(theo / "essences-coe.json")],
            )
        )
    journal.append(
        _lancer(
            "Forge",
            forge_data.principal,
            [
                "--tables", str(tables / "tables"),
                "--poids", str(theo / "poids-poe2db.json"),
                "--complement", str(theo / "poids-coe.json"),
                "--essences", str(theo / "essences-coe.json"),
                "--out", str(travail / "data" / "forge.json.gz"),
            ],
        )
    )  # fmt: skip
    return [ligne.replace(str(travail), "…") for ligne in journal if ligne]


# --- Chargement -------------------------------------------------------------


def charger(lire_fichier) -> dict:
    """Contenu comparable des données : `lire_fichier(chemin du dépôt) -> octets | None`."""

    def json_de(chemin: str):
        octets = lire_fichier(chemin)
        if octets is None:
            return None
        if chemin.endswith(".gz"):
            octets = gzip.decompress(octets)
        return json.loads(octets.decode("utf-8-sig"))

    gemmes = json_de("crates/poe2-core/data/gemmes.json") or {}
    arbre = json_de("crates/poe2-core/data/arbre.json") or {}
    forge = json_de("crates/poe2-core/data/forge.json.gz") or {}
    releve = forge.get("releve") or {}
    return {
        "repoe": forge.get("repoe_version") or "?",
        "arbre_version": arbre.get("version") or "?",
        "poids_date": releve.get("date_releve") or "?",
        "complement_date": (releve.get("complement") or {}).get("date_releve") or "?",
        "actives": gemmes.get("actives") or {},
        "supports": gemmes.get("supports") or {},
        "armes": gemmes.get("armes") or {},
        "passifs": arbre.get("passifs") or {},
        "mods": {m["id"]: m for m in (forge.get("mods") or {}).get("mods", [])},
        "bases": {b["id"]: b for b in (forge.get("bases") or {}).get("bases", [])},
        "essences": {e["id"]: e for e in forge.get("essences", [])},
        "poids": poids_effectifs(releve),
    }


def poids_effectifs(releve: dict) -> dict[str, dict[str, int]]:
    """`{mod: {page: poids}}` d'après le relevé poe2db et son complément Craft of Exile."""
    out: dict[str, dict[str, int]] = {}
    for source in (releve, releve.get("complement") or {}):
        for page, contenu in (source.get("pages") or {}).items():
            for mod, poids in (contenu.get("poids") or {}).items():
                out.setdefault(mod, {})[page] = poids
    return out


# --- Comparaison ------------------------------------------------------------


def aplatir(obj, prefixe: str = "") -> dict:
    """`{"a": {"b": 1}}` -> `{"a.b": 1}` ; les listes restent des valeurs."""
    if isinstance(obj, dict) and obj:
        out = {}
        for k, v in obj.items():
            out.update(aplatir(v, f"{prefixe}.{k}" if prefixe else str(k)))
        return out
    return {prefixe: obj}


def differences(avant, apres) -> list[tuple[str, object, object]]:
    """Chemins modifiés entre deux valeurs : `[(chemin, avant, après)]`, `None` = absent."""
    a, b = aplatir(avant), aplatir(apres)
    return [(k, a.get(k), b.get(k)) for k in sorted(set(a) | set(b), key=_cle_chemin) if a.get(k) != b.get(k)]


def _cle_chemin(chemin: str):
    return [(0, int(p), "") if p.isdigit() else (1, 0, p) for p in chemin.split(".")]


def comparer(avant: dict, apres: dict) -> dict:
    """`{"ajouts": [ids], "retraits": [ids], "modifs": [ids]}` entre deux dictionnaires."""
    return {
        "ajouts": sorted(set(apres) - set(avant), key=_cle_chemin),
        "retraits": sorted(set(avant) - set(apres), key=_cle_chemin),
        "modifs": sorted((k for k in set(avant) & set(apres) if avant[k] != apres[k]), key=_cle_chemin),
    }


def fmt(v) -> str:
    if v is None:
        return "—"
    if isinstance(v, bool):
        return "oui" if v else "non"
    if isinstance(v, float) and v.is_integer():
        return str(int(v))
    if isinstance(v, list):
        if v and all(isinstance(x, dict) and set(x) == {"tag", "weight"} for x in v):
            return ", ".join(f"{x['tag']} {x['weight']}" for x in v)
        if len(v) == 2 and all(isinstance(x, (int, float)) for x in v):
            return f"{fmt(v[0])}–{fmt(v[1])}"
        return "[" + ", ".join(fmt(x) for x in v) + "]"
    if isinstance(v, dict):
        return json.dumps(v, ensure_ascii=False, sort_keys=True)
    texte = str(v).replace("\n", " ")
    return texte if len(texte) <= 160 else texte[:157] + "…"


def detail_sort(a: dict, b: dict) -> list[str]:
    """Changements d'un sort, niveaux résumés au niveau de référence."""
    hors_niveaux = differences(
        {k: v for k, v in a.items() if k != "niveaux"}, {k: v for k, v in b.items() if k != "niveaux"}
    )
    lignes = [f"{c} : {fmt(x)} → {fmt(y)}" for c, x, y in hors_niveaux]
    na, nb = a.get("niveaux") or {}, b.get("niveaux") or {}
    if na != nb:
        touches = [n for n in set(na) & set(nb) if na[n] != nb[n]]
        if len(na) != len(nb):
            lignes.append(f"niveaux : {len(na)} → {len(nb)}")
        if touches:
            communs = set(na) & set(nb)
            ref = NIVEAU_REFERENCE if NIVEAU_REFERENCE in communs else max(communs, key=int)
            diff_ref = differences(na[ref], nb[ref])
            if diff_ref:
                details = " ; ".join(f"{c} {fmt(x)} → {fmt(y)}" for c, x, y in diff_ref)
                lignes.append(f"{len(touches)} niveau(x) modifié(s), niveau {ref} : {details}")
            else:
                lignes.append(f"{len(touches)} niveau(x) modifié(s) (niveau {ref} identique)")
    return lignes


def detail_textes(a: list, b: list) -> list[str]:
    return [f"retiré : {fmt(t)}" for t in a if t not in b] + [f"ajouté : {fmt(t)}" for t in b if t not in a]


def detail_generique(a, b) -> list[str]:
    return [f"{c} : {fmt(x)} → {fmt(y)}" for c, x, y in differences(a, b)]


def _liste(lignes: list[str], limite: int = LIMITE) -> list[str]:
    if len(lignes) <= limite:
        return lignes
    return lignes[:limite] + [f"- … et {len(lignes) - limite} autres"]


def section(titre: str, diff: dict, nom, detail, avant: dict, apres: dict) -> list[str]:
    total = sum(len(v) for v in diff.values())
    out = [f"## {titre}", ""]
    if not total:
        return out + ["Aucun changement.", ""]
    for cle, libelle, source in (("ajouts", "Ajouts", apres), ("retraits", "Retraits", avant)):
        if diff[cle]:
            out += [f"**{libelle} ({len(diff[cle])})**", ""]
            out += _liste([f"- {nom(i, source[i])}" for i in diff[cle]])
            out.append("")
    if diff["modifs"]:
        out += [f"**Changements ({len(diff['modifs'])})**", ""]
        blocs = []
        for i in diff["modifs"]:
            bloc = [f"- {nom(i, apres[i])}"] + [f"  - {ligne}" for ligne in detail(avant[i], apres[i])[:12]]
            blocs.append("\n".join(bloc))
        out += _liste(blocs)
        out.append("")
    return out


def _nom_sort(i, s):
    return f"{s.get('nom') or i} (`{i}`)"


def _nom_mod(i, m):
    return f"`{i}` ({m.get('nom') or '?'}, {m.get('slot') or '?'}) : {fmt(m.get('texte'))}"


def _nom_base(i, b):
    return f"{b.get('nom') or i} ({b.get('classe') or '?'})"


def _nom_essence(i, e):
    return e.get("nom") or i


def _nom_passif(i, lignes):
    return f"`{i}` : {fmt(' / '.join(lignes))}"


def rapport(
    avant: dict, apres: dict, date: str, journal: list[str] | None = None, fichiers: list[str] | None = None
) -> tuple[str, dict]:
    """Rapport Markdown des différences et comptages par élément."""
    parties = {
        "actives": ("Sorts et attaques", _nom_sort, detail_sort),
        "supports": ("Supports", lambda i, _: f"`{i}`", detail_textes),
        "armes": ("Bases d'armes (Copilot)", lambda i, _: i, detail_generique),
        "passifs": ("Arbre passif", _nom_passif, detail_textes),
        "mods": ("Forge : mods", _nom_mod, detail_generique),
        "bases": ("Forge : bases d'objets", _nom_base, detail_generique),
        "essences": ("Forge : Essences et Alloys", _nom_essence, detail_generique),
    }
    diffs = {k: comparer(avant[k], apres[k]) for k in parties}
    diffs["poids"] = comparer(avant["poids"], apres["poids"])

    attention = []
    verifies = [
        i
        for i in diffs["actives"]["modifs"] + diffs["actives"]["retraits"]
        if avant["actives"][i].get("verifiee") or apres["actives"].get(i, {}).get("verifiee")
    ]
    if verifies:
        noms = ", ".join(avant["actives"][i].get("nom") or i for i in verifies)
        attention.append(
            f"Sort vérifié en jeu modifié ou retiré ({noms}) : rejouer les cas golden (`tests/golden/archetype/`) et re-mesurer en jeu."
        )
    if avant["arbre_version"] != apres["arbre_version"]:
        attention.append(
            f"Nouvelle version d'arbre ({avant['arbre_version']} → {apres['arbre_version']}) : vérifier l'import des codes PoB."
        )
    sans_poids_avant = {m for m in avant["mods"] if m not in avant["poids"]}
    sans_poids = sorted(m for m in apres["mods"] if m not in apres["poids"] and m not in sans_poids_avant)
    if sans_poids:
        attention.append(
            f"{len(sans_poids)} mod(s) de Forge sans poids connu (probabilité indisponible tant que poe2db / "
            f"Craft of Exile ne les ont pas estimés) : "
            + ", ".join(f"`{m}`" for m in sans_poids[:20])
            + (" …" if len(sans_poids) > 20 else "")
        )
    retires_utilises = sorted(
        m
        for e in apres["essences"].values()
        for ms in e.get("cibles", {}).values()
        for m in ms
        if m in diffs["mods"]["retraits"]
    )
    if retires_utilises:
        attention.append(
            "Mods retirés encore visés par une Essence : " + ", ".join(sorted(set(retires_utilises)))
        )

    lignes = [
        f"# Mise à jour des données de jeu du {date}",
        "",
        "| Source | Avant | Après |",
        "|---|---|---|",
        f"| RePoE (sorts, mods, bases) | {avant['repoe']} | {apres['repoe']} |",
        f"| Arbre passif (Path of Building) | {avant['arbre_version']} | {apres['arbre_version']} |",
        f"| Poids poe2db (relevé du) | {avant['poids_date']} | {apres['poids_date']} |",
        f"| Poids Craft of Exile (relevé du) | {avant['complement_date']} | {apres['complement_date']} |",
        "",
        "| Élément | Avant | Après | Ajouts | Retraits | Changements |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for k, (titre, *_rest) in list(parties.items()) + [("poids", ("Forge : mods avec poids",))]:
        d = diffs[k]
        lignes.append(
            f"| {titre} | {len(avant[k])} | {len(apres[k])} | {len(d['ajouts'])} | {len(d['retraits'])} | {len(d['modifs'])} |"
        )
    lignes += ["", "## Points d'attention", ""]
    lignes += [f"- {a}" for a in attention] or ["Aucun."]
    lignes.append("")
    for k, (titre, nom, detail) in parties.items():
        lignes += section(titre, diffs[k], nom, detail, avant[k], apres[k])
    lignes += section(
        "Forge : poids des mods (par page de classe)",
        diffs["poids"],
        lambda i, p: f"`{i}` : " + ", ".join(f"{page} {v}" for page, v in sorted(p.items())),
        detail_generique,
        avant["poids"],
        apres["poids"],
    )
    if fichiers is not None:
        lignes += ["## Fichiers régénérés", ""]
        lignes += [f"- `{f}`" for f in fichiers] or ["Aucun : les données sont déjà à jour."]
        lignes.append("")
    if journal:
        lignes += ["## Journal des outils", "", "```", *journal, "```", ""]
    comptes = {k: {c: len(v) for c, v in d.items()} for k, d in diffs.items()}
    return "\n".join(lignes).rstrip() + "\n", {"comptes": comptes, "attention": attention}


# --- Commande ---------------------------------------------------------------


def executer(a: argparse.Namespace, lire=lire_url, racine: Path = RACINE) -> int:
    date = a.date or dt.datetime.now(dt.UTC).date().isoformat()
    with tempfile.TemporaryDirectory(prefix="augur-maj-") as tmp:
        travail = Path(tmp)
        if a.depuis:
            sources = a.depuis
            fichier = sources / "versions.json"
            versions = json.loads(fichier.read_text(encoding="utf-8")) if fichier.exists() else {}
            versions.setdefault("repoe", a.repoe_version or "?")
            if a.arbre != "auto":
                versions["arbre"] = a.arbre
            versions.setdefault("arbre", _version_arbre_actuelle(racine))
            if not a.coe:
                versions.pop("coe", None)
        else:
            sources = a.cache
            arbre = a.arbre
            print(f"Téléchargement des sources dans {sources} …", file=sys.stderr)
            versions = telecharger(sources, arbre, a.coe, lire)
        if a.repoe_version:
            versions["repoe"] = a.repoe_version
        poe2db_cache = (a.cache / f"poe2db-{date}") if a.poe2db else None
        journal = regenerer(sources, travail, versions, date, poe2db_cache, racine)

        sortie = {**EMBARQUES, **POIDS}
        avant = charger(lambda c: _lire(racine / c))
        apres = charger(lambda c: _lire(travail / sortie[c]))
        modifies = [c for c, n in sortie.items() if _lire(racine / c) != _lire(travail / n)]
        texte, resume = rapport(avant, apres, date, journal, modifies)

        a.rapport.parent.mkdir(parents=True, exist_ok=True)
        a.rapport.write_text(texte, encoding="utf-8")
        if not a.essai:
            for c in modifies:
                (racine / c).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(travail / sortie[c], racine / c)
    comptes = resume["comptes"]
    total = sum(sum(d.values()) for d in comptes.values())
    print(f"Rapport : {a.rapport}")
    print(
        f"RePoE {avant['repoe']} → {apres['repoe']}, arbre {avant['arbre_version']} → {apres['arbre_version']}"
    )
    print(
        f"{total} différence(s) ; fichiers {'à mettre à jour' if a.essai else 'mis à jour'} : {', '.join(modifies) or 'aucun'}"
    )
    for ligne in resume["attention"]:
        print(f"ATTENTION : {ligne}")
    return 0


def _lire(chemin: Path) -> bytes | None:
    return chemin.read_bytes() if chemin.exists() else None


def _version_arbre_actuelle(racine: Path) -> str:
    arbre = racine / "crates/poe2-core/data/arbre.json"
    if not arbre.exists():
        return "?"
    return json.loads(arbre.read_text(encoding="utf-8")).get("version", "?")


def principal(argv: list[str] | None = None, lire=lire_url, racine: Path = RACINE) -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--essai", action="store_true", help="écrire le rapport sans toucher au dépôt")
    p.add_argument("--arbre", default="auto", help="version d'arbre PoB (ex. 0_5) ; auto = la dernière")
    p.add_argument("--coe", action="store_true", help="rafraîchir aussi les poids et Essences Craft of Exile")
    p.add_argument("--poe2db", action="store_true", help="refaire aussi le relevé poe2db (lent)")
    p.add_argument("--depuis", type=Path, help="dossier de sources déjà téléchargées (sans réseau)")
    p.add_argument("--repoe-version", help="version RePoE à noter (lue sur le site par défaut)")
    p.add_argument("--cache", type=Path, default=racine / "var" / "mise-a-jour" / "sources")
    p.add_argument("--rapport", type=Path)
    p.add_argument("--date", help="date du jour (AAAA-MM-JJ), pour les relevés et le rapport")
    a = p.parse_args(argv)
    a.rapport = (
        a.rapport
        or racine
        / "var"
        / "mise-a-jour"
        / f"rapport-{a.date or dt.datetime.now(dt.UTC).date().isoformat()}.md"
    )
    try:
        return executer(a, lire, racine)
    except Echec as e:
        print(f"erreur : {e}", file=sys.stderr)
        print("Aucun fichier du dépôt n'a été modifié.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(principal())
