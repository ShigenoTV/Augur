#!/usr/bin/env bash
# Plan B du patch 1.0 : produire nous-mêmes l'export RePoE PoE 2 quand repoe-fork tarde à publier.
#
# RePoE (https://github.com/repoe-fork/repoe, licence MIT) lit les fichiers du jeu avec PyPoE
# directement sur le serveur de patch de GGG (adresse donnée par https://ggpk.exposed) : il ne faut ni
# le jeu installé, ni le PC de Max. Ce script :
#
#   1. clone repoe-fork/repoe et repoe-fork/PyPoE (ou les met à jour) dans un dossier de travail ;
#   2. exporte base_items, mods, stat_translations et skills pour PoE 2 ;
#   3. assemble un dossier prêt pour `mise_a_jour.py --depuis` (arbre PoB et stats de trade téléchargés
#      comme d'habitude), puis lance `mise_a_jour.py --essai --depuis` pour le rapport.
#
# Les stats de trade de RePoE (api/trade2/data/stats) ne servent pas à Augur et le site de trade refuse
# souvent les machines du cloud : le script les remplace par une liste vide (variable
# TRADE2_STATS_FILE, petite retouche locale de RePoE, rien n'est poussé chez eux).
#
# Essai du 22/11/2026 (jeu 0.5.5.10) : base_items.json, base_items.min.json, mods.min.json et
# skills.json identiques à l'octet près à l'export publié ; rapport de mise à jour : 0 différence.
# Durée : environ 4 minutes, dont le téléchargement des fichiers du jeu.
#
# Limite : PyPoE doit connaître le format des tables du jeu. Si la 1.0 change ce format, il faut
# attendre que repoe-fork/PyPoE mette à jour sa description des tables (souvent avant l'export
# RePoE lui-même). Régénérer cette description depuis poe-tool-dev/dat-schema brut ne suffit pas
# (essai du 22/11 : « No column BaseItemTypesKey found in ArmourTypes.dat64 »), le fork y ajoute
# ses propres corrections.
#
# Utilisation (depuis la racine du dépôt, Python 3.13) :
#
#     tools/gamedata/plan_b_repoe.sh [--travail DOSSIER] [--version 1.0.0.1]
#
# Ensuite, si le rapport est bon :
#
#     python tools/gamedata/mise_a_jour.py --depuis DOSSIER/sources --repoe-version <version>
#
set -euo pipefail

RACINE="$(cd "$(dirname "$0")/../.." && pwd)"
TRAVAIL="${TMPDIR:-/tmp}/augur-plan-b"
VERSION=""
while [ $# -gt 0 ]; do
  case "$1" in
    --travail) TRAVAIL="$2"; shift ;;
    --version) VERSION="$2"; shift ;;
    -h|--help) sed -n '2,36p' "$0"; exit 0 ;;
    *) echo "option inconnue : $1" >&2; exit 2 ;;
  esac
  shift
done
mkdir -p "$TRAVAIL"
cd "$TRAVAIL"

etape() { printf '\n== %s\n' "$*"; }

etape "1. RePoE et PyPoE"
for depot in repoe PyPoE; do
  if [ -d "$depot/.git" ]; then
    git -C "$depot" fetch -q --depth 1 origin HEAD && git -C "$depot" reset -q --hard FETCH_HEAD
  else
    git clone -q --depth 1 "https://github.com/repoe-fork/$depot" "$depot"
  fi
  echo "$depot : $(git -C "$depot" log -1 --format='%h du %ci')"
done
if [ ! -d venv ]; then python3 -m venv venv; fi
# shellcheck disable=SC1091
. venv/bin/activate
pip -q install -e ./PyPoE
pip -q install pydantic requests pillow
pip -q install -e ./repoe --no-deps

# Stats de trade : lecture d'un fichier local si TRADE2_STATS_FILE est donné.
python - <<'PY'
from pathlib import Path
p = Path("repoe/RePoE/parser/poe2/stat_translations.py")
s = p.read_text(encoding="utf-8")
if "TRADE2_STATS_FILE" not in s:
    a = '        with urlopen(\n            Request(\n                "https://www.pathofexile.com/api/trade2/data/stats",'
    b = ('        import os as _os\n        _src = _os.environ.get("TRADE2_STATS_FILE")\n'
         '        with (open(_src, "rb") if _src else urlopen(\n            Request(\n'
         '                "https://www.pathofexile.com/api/trade2/data/stats",')
    a2 = '        ) as req:\n            data = json.load(req)'
    b2 = '        )) as req:\n            data = json.load(req)'
    if s.count(a) != 1 or s.count(a2) != 1:
        raise SystemExit("RePoE a changé : retouche des stats de trade impossible, voir stat_translations.py")
    p.write_text(s.replace(a, b).replace(a2, b2), encoding="utf-8")
    print("retouche des stats de trade appliquée")
PY
echo '{"result":[]}' > trade2-vide.json

etape "2. Export RePoE PoE 2 (serveur de patch : $(curl -fsS 'https://ggpk.exposed/version?poe=2'))"
rm -rf export && mkdir export
(cd repoe && TRADE2_STATS_FILE="$TRAVAIL/trade2-vide.json" \
  python -m RePoE.run_parser -2 -o "$TRAVAIL/export" base_items mods stat_translations skills)

etape "3. Dossier de sources et rapport"
cd "$RACINE"
# Sources habituelles (arbre PoB, stats de trade d'Exiled Exchange 2…), puis remplacement des
# fichiers RePoE par notre export.
python tools/gamedata/mise_a_jour.py --essai --cache "$TRAVAIL/cache" --rapport "$TRAVAIL/rapport-publie.md" >/dev/null
rm -rf "$TRAVAIL/sources" && cp -r "$TRAVAIL/cache" "$TRAVAIL/sources"
for f in base_items.json base_items.min.json mods.min.json skills.json; do
  cp "$TRAVAIL/export/$f" "$TRAVAIL/sources/$f"
done
cp "$TRAVAIL/export/stat_translations/stat_descriptions.min.json" "$TRAVAIL/sources/"
python tools/gamedata/mise_a_jour.py --essai --depuis "$TRAVAIL/sources" \
  ${VERSION:+--repoe-version "$VERSION"} --rapport "$TRAVAIL/rapport-plan-b.md"
echo
echo "Rapport : $TRAVAIL/rapport-plan-b.md"
echo "Pour appliquer : python tools/gamedata/mise_a_jour.py --depuis $TRAVAIL/sources --repoe-version <version>"
