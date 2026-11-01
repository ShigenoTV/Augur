#!/usr/bin/env python3
"""Vérificateur de cohérence des cas golden Augur.

Recalcule chaque cas `type: unitaire` de tests/formules/*.yaml avec les formules
de référence des fiches (docs/theorycraft/fiches/) et vérifie le schéma de tous
les cas. Ce n'est PAS le moteur : c'est un oracle qui garantit que les valeurs
attendues écrites par le Theorycrafter sont cohérentes avec les fiches.

Usage : python3 tools/theorycraft/verifier_golden.py [dossier]
Code de sortie non nul si un cas est invalide ou incohérent.
"""
from __future__ import annotations

import math
import sys
from pathlib import Path

import yaml

TYPES = ("physique", "feu", "froid", "foudre", "chaos")
ELEMENTAIRES = {"feu", "froid", "foudre"}
CHAMPS_OBLIGATOIRES = ("id", "type", "nature", "patch", "confiance", "statut", "description", "attendu", "preuve")
CONFIANCES = {"confirmé", "mesuré", "supposé"}
STATUTS = {"brouillon", "à-mesurer", "validé"}
NATURES = {"nominal", "limite", "interaction", "discriminant"}


def _moyenne(intervalle):
    mn, mx = intervalle
    return (mn + mx) / 2


def _produit_more(liste):
    p = 1.0
    for m in liste or []:
        p *= 1 + m / 100
    return p


def _etape_conversion(montants, conversions, gains):
    """Une étape (compétence ou secondaire), d'après poe2wiki « Damage conversion » :
    1. les gains se calculent sur les montants d'avant l'étape (source `degats` = total
       de tous les types) et s'ajoutent ;
    2. les conversions s'appliquent ensuite aux montants obtenus, gains compris
       (exemple Explosive Grenade + Blueflame Bracers du wiki), normalisées à 100 %
       par type source, sans enchaînement à l'intérieur de l'étape."""
    apres_gains = dict(montants)
    total_avant = sum(montants.values())
    for source, cibles in (gains or {}).items():
        depart = total_avant if source == "degats" else montants.get(source, 0.0)
        for cible, pct in cibles.items():
            apres_gains[cible] = apres_gains.get(cible, 0.0) + depart * pct / 100
    sortie = dict(apres_gains)
    for source, cibles in (conversions or {}).items():
        total = sum(cibles.values())
        echelle = 100 / total if total > 100 else 1.0
        depart = apres_gains.get(source, 0.0)
        for cible, pct in cibles.items():
            transfert = depart * pct * echelle / 100
            sortie[source] = sortie.get(source, 0.0) - transfert
            sortie[cible] = sortie.get(cible, 0.0) + transfert
    return sortie


def _increased_pour(type_, increased):
    total = 0.0
    for etiquette, pct in (increased or {}).items():
        if etiquette == type_ or etiquette in ("sort", "tous") or (etiquette == "elementaire" and type_ in ELEMENTAIRES):
            total += pct
    return total


def degats_coup_moyen(e):
    eff = e.get("efficacite_ajoutes", 1.0)
    montants = {}
    for t in TYPES:
        b = _moyenne(e.get("base", {}).get(t, [0, 0]))
        a = _moyenne(e.get("ajoutes", {}).get(t, [0, 0])) * eff
        if b or a:
            montants[t] = b + a
    montants = _etape_conversion(montants, e.get("conversion_competence"), e.get("gain_competence"))
    montants = _etape_conversion(montants, e.get("conversion_secondaire"), e.get("gain_secondaire"))
    more = _produit_more(e.get("more"))
    par_type = {}
    for t, v in montants.items():
        inc = max(0.0, 1 + _increased_pour(t, e.get("increased")) / 100)
        par_type[t] = v * inc * more
    return {"par_type": par_type, "total": sum(par_type.values())}


def critique_multiplicateur(e):
    chance = min(100.0, (e["crit_base"] + e.get("crit_plat", 0)) * (1 + e.get("increased_crit", 0) / 100))
    bonus = (100 + e.get("bonus_plat", 0)) * (1 + e.get("increased_bonus", 0) / 100)
    return {"chance_pct": chance, "bonus_pct": bonus, "multiplicateur_moyen": 1 + chance / 100 * bonus / 100}


def vitesse_incantation(e):
    cadence = (1 / e["temps_base"]) * max(0.0, 1 + e.get("increased", 0) / 100) * _produit_more(e.get("more"))
    return {"cadence": cadence, "temps": 1 / cadence}


def _res_cible(res, pen):
    eff = res if res <= 0 else max(0.0, res - pen)
    return eff, 1 - eff / 100


def resistances_cible(e):
    eff, mult = _res_cible(e["res_cible"], e.get("penetration", 0))
    return {"effective": eff, "multiplicateur": mult}


def dps_sort(e):
    total = 0.0
    for t, v in e["coup_moyen_par_type"].items():
        _, mult = _res_cible(e.get("res_cible", {}).get(t, 0), e.get("penetration", {}).get(t, 0))
        total += v * mult
    return {"dps": total * e.get("multiplicateur_crit", 1.0) * e["cadence"]}


def resistances_joueur(e):
    max_eff = min(90.0, 75 + e.get("bonus_max", 0))
    penalite = 0 if e["type"] == "chaos" else e.get("penalite_campagne", 60)
    apres = e["brute"] - penalite
    finale = min(max_eff, apres)
    return {
        "max_effectif": max_eff,
        "finale": finale,
        "surplus": max(0.0, apres - max_eff),
        "multiplicateur_subi": 1 - finale / 100,
    }


def _reduction_armure(a, d, add=0.0):
    base = a / (a + 10 * d) if (a + 10 * d) > 0 else 0.0
    return min(0.90, base + add / 100)


def armure_reduction(e):
    red = _reduction_armure(e["armure"], e["degats"], e.get("reduction_additionnelle_pct", 0))
    return {"reduction_pct": red * 100, "degats_subis": e["degats"] * (1 - red)}


def evasion_chance(e):
    p, ev = e["precision"], e["evasion"]
    toucher = 1.25 * p / (p + 0.3 * ev)
    return {"chance_eviter_pct": min(95.0, max(0.0, (1 - toucher) * 100))}


def deflexion_chance(e):
    p, d = e["precision"], e["deflexion"]
    return {"chance_deflexion_pct": min(95.0, max(0.0, 150 * (1 - p / (p + 0.12 * d))))}


def es_maximum(e):
    objets = sum(o["base"] * (1 + o.get("increased_local", 0) / 100) * (1 + o.get("qualite", 0) / 100) for o in e.get("objets", []))
    total = (objets + e.get("plat_global", 0)) * (1 + e.get("increased_global", 0) / 100) * _produit_more(e.get("more"))
    return {"es_max": total}


def es_recharge(e):
    return {
        "delai_s": 4 / (1 + e.get("demarrage_plus_rapide", 0) / 100),
        "par_seconde": 0.125 * e["es_max"] * (1 + e.get("increased_recharge", 0) / 100),
    }


def ehp_coup_max(e):
    t = e["type"]
    reserve = e.get("vie", 0) + (e.get("es", 0) / 2 if t == "chaos" else e.get("es", 0))
    m = (1 + e.get("increased_subi", 0) / 100) * _produit_more(e.get("more_subi"))
    if t == "physique":
        a = e.get("armure", 0)
        d = (10 * reserve + math.sqrt(100 * reserve**2 + 40 * m * reserve * a)) / (20 * m)
        if a > 0 and a / (a + 10 * d) > 0.90:
            d = reserve / (0.10 * m)
        return {"coup_max": d}
    return {"coup_max": reserve / ((1 - e.get("resistance", 0) / 100) * m)}


CALCULS = {
    "degats.coup_moyen": degats_coup_moyen,
    "critique.multiplicateur": critique_multiplicateur,
    "vitesse.incantation": vitesse_incantation,
    "dps.sort": dps_sort,
    "resistances.joueur": resistances_joueur,
    "resistances.cible": resistances_cible,
    "armure.reduction": armure_reduction,
    "evasion.chance": evasion_chance,
    "deflexion.chance": deflexion_chance,
    "es.maximum": es_maximum,
    "es.recharge": es_recharge,
    "ehp.coup_max": ehp_coup_max,
}


def _comparer(chemin, attendu, obtenu, tol_pct, erreurs):
    if isinstance(attendu, dict):
        if not isinstance(obtenu, dict):
            erreurs.append(f"{chemin}: dictionnaire attendu")
            return
        for cle, val in attendu.items():
            _comparer(f"{chemin}.{cle}", val, obtenu.get(cle, 0.0), tol_pct, erreurs)
        return
    ecart = abs(obtenu - attendu)
    limite = max(abs(attendu) * tol_pct / 100, 1e-9)
    if ecart > limite:
        erreurs.append(f"{chemin}: attendu {attendu}, recalculé {obtenu:.6f}")


def verifier(dossier: Path) -> int:
    ids = set()
    erreurs: list[str] = []
    nb_unitaires = nb_attente = 0
    for fichier in sorted(dossier.glob("*.yaml")):
        doc = yaml.safe_load(fichier.read_text(encoding="utf-8"))
        for cas in doc.get("cas", []):
            cid = cas.get("id", "?")
            prefixe = f"{fichier.name}:{cid}"
            for champ in CHAMPS_OBLIGATOIRES:
                if champ not in cas:
                    erreurs.append(f"{prefixe}: champ manquant `{champ}`")
            if cid in ids:
                erreurs.append(f"{prefixe}: id en double")
            ids.add(cid)
            if cas.get("confiance") not in CONFIANCES:
                erreurs.append(f"{prefixe}: confiance invalide")
            if cas.get("statut") not in STATUTS:
                erreurs.append(f"{prefixe}: statut invalide")
            if cas.get("nature") not in NATURES:
                erreurs.append(f"{prefixe}: nature invalide")
            if cas.get("statut") == "validé" and not cas.get("preuve"):
                erreurs.append(f"{prefixe}: un cas validé doit référencer une preuve")
            if cas.get("type") == "personnage":
                nb_attente += cas.get("statut") != "validé"
                continue
            if cas.get("type") != "unitaire":
                erreurs.append(f"{prefixe}: type invalide")
                continue
            fonction = CALCULS.get(cas.get("calcul"))
            if fonction is None:
                erreurs.append(f"{prefixe}: calcul inconnu `{cas.get('calcul')}`")
                continue
            nb_unitaires += 1
            _comparer(prefixe, cas["attendu"], fonction(cas.get("entrees", {})), cas.get("tolerance_pct", 0.01), erreurs)
    for err in erreurs:
        print(f"ÉCHEC {err}")
    print(f"{nb_unitaires} cas unitaires vérifiés, {nb_attente} cas personnage en attente de mesure, {len(erreurs)} erreur(s).")
    return 1 if erreurs else 0


if __name__ == "__main__":
    racine = Path(__file__).resolve().parents[2]
    sys.exit(verifier(Path(sys.argv[1]) if len(sys.argv) > 1 else racine / "tests" / "formules"))
