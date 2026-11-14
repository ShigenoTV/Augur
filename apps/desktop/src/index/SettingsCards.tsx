import { Kbd, StatePanel } from "@augur/ui";
import { useEffect, useState, type KeyboardEvent } from "react";
import { commands, DEFAULT_LEAGUE, isAppError, isTauri, type Settings, type ShortcutSettings } from "../bridge";
import { jeuActif, ligueDuJeu, ligueParDefaut } from "./game";
import { afficherRaccourci, lireTouche, SHORTCUTS_PAR_DEFAUT } from "./shortcuts";

const champ =
  "min-w-0 rounded-chip border border-ink-700 bg-ink-950 px-2 py-1.5 text-sm text-text-primary placeholder:text-text-subtle";
const btnSecondary = "rounded-chip border border-ink-600 px-3 py-1.5 text-sm text-text-muted hover:bg-ink-800 disabled:opacity-50";

const message = (err: unknown) => (isAppError(err) ? err.message : String(err));

/** Ligues proposées si poe.ninja ne répond pas. */
const LIGUES_SECOURS = [DEFAULT_LEAGUE, "Standard"];

/** Liste déroulante : ligues de poe.ninja, plus la ligue enregistrée si elle n'y est plus. */
export function optionsLigues(poeNinja: string[] | null, actuelle: string, secours: string[] = LIGUES_SECOURS): string[] {
  const liste = poeNinja && poeNinja.length > 0 ? [...poeNinja] : [...secours];
  if (!liste.includes(actuelle)) liste.unshift(actuelle);
  return liste;
}

/** Ligue poe.ninja utilisée pour les prix de la Forge, choisie dans une liste. */
export function LeagueCard({ settings, onSettings }: { settings: Settings | null; onSettings: (s: Settings) => void }) {
  const [ligues, setLigues] = useState<string[] | null>(null);
  const [horsLigne, setHorsLigne] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const game = jeuActif(settings);
  const actuelle = ligueDuJeu(settings);
  const parDefaut = ligueParDefaut(game);

  // La liste dépend du jeu actif (poe.ninja a une liste par jeu).
  useEffect(() => {
    if (!isTauri() || !settings) return;
    setLigues(null);
    setHorsLigne(false);
    commands
      .getLeagues()
      .then(setLigues)
      .catch(() => setHorsLigne(true));
  }, [game, settings === null]);

  async function choisir(league: string) {
    setError(null);
    setInfo(null);
    try {
      onSettings(await commands.setLeague(league === parDefaut ? null : league));
      setInfo(
        game === "poe1"
          ? "Ligue PoE 1 enregistrée."
          : "Ligue enregistrée. Actualise les prix dans la Forge pour charger son relevé.",
      );
    } catch (err) {
      setError(message(err));
    }
  }

  return (
    <div className="space-y-3 text-sm">
      <p className="text-text-muted">
        {game === "poe1"
          ? "Ligue de PoE 1 : elle servira aux prix et à la recherche sur le site de trade."
          : "Les prix de la Forge sont relevés sur poe.ninja pour cette ligue."}
      </p>
      <select
        value={actuelle}
        disabled={!settings}
        onChange={(e) => void choisir(e.target.value)}
        aria-label="Ligue souhaitée"
        className={`${champ} w-64 disabled:opacity-50`}
      >
        {optionsLigues(ligues, actuelle, game === "poe1" ? [parDefaut] : LIGUES_SECOURS).map((l) => (
          <option key={l} value={l}>
            {l}
            {l === parDefaut ? " (par défaut)" : ""}
          </option>
        ))}
      </select>
      {horsLigne && (
        <p className="text-xs text-text-subtle">poe.ninja ne répond pas : liste réduite, réessaie plus tard.</p>
      )}
      {info && <p className="text-xs text-signal-gain">{info}</p>}
      {error && (
        <StatePanel icon="alert" title="Ligue non enregistrée" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}

/** Budget de la recherche d'upgrades (Alt+U dans l'overlay), en exalts. */
export function UpgradeBudgetCard({ settings, onSettings }: { settings: Settings | null; onSettings: (s: Settings) => void }) {
  const [saisie, setSaisie] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const valeur = saisie ?? (settings?.upgradeBudget ? String(settings.upgradeBudget) : "");

  async function enregistrer() {
    setError(null);
    setInfo(null);
    const texte = valeur.trim();
    const budget = texte === "" ? null : Number(texte);
    if (budget !== null && (!Number.isInteger(budget) || budget < 0)) {
      setError("Indique un nombre entier d'exalts, ou laisse vide pour chercher sans limite.");
      return;
    }
    try {
      onSettings(await commands.setUpgradeBudget(budget || null));
      setSaisie(null);
      setInfo(budget ? `Budget enregistré : ${budget} exalts au plus.` : "Recherche sans limite de prix.");
    } catch (err) {
      setError(message(err));
    }
  }

  return (
    <div className="space-y-3 text-sm">
      <p className="text-text-muted">
        Prix maximum des objets proposés par la recherche d'upgrades (sur le site de trade, dans la ligue souhaitée).
      </p>
      <div className="flex items-center gap-2">
        <input
          inputMode="numeric"
          value={valeur}
          disabled={!settings}
          onChange={(e) => setSaisie(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void enregistrer()}
          placeholder="sans limite"
          aria-label="Budget en exalts"
          className={`${champ} w-32 disabled:opacity-50`}
        />
        <span className="text-text-muted">exalts</span>
        <button type="button" onClick={() => void enregistrer()} disabled={!settings} className={btnSecondary}>
          Enregistrer
        </button>
      </div>
      {info && <p className="text-xs text-signal-gain">{info}</p>}
      {error && (
        <StatePanel icon="alert" title="Budget non enregistré" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}

type Champ = keyof ShortcutSettings;

const LIGNES: { id: Champ; action: string }[] = [
  { id: "closeOverlay", action: "Fermer l'overlay" },
  { id: "upgradeSearch", action: "Chercher un meilleur objet (recherche de trade pondérée)" },
];

/** Raccourcis de l'overlay : Échap et Alt+U modifiables, Ctrl+C et Ctrl+Alt+C fixés par le jeu. */
export function ShortcutsCard({ settings, onSettings }: { settings: Settings | null; onSettings: (s: Settings) => void }) {
  const [capture, setCapture] = useState<Champ | null>(null);
  const [error, setError] = useState<string | null>(null);
  const actuels = settings?.shortcuts ?? SHORTCUTS_PAR_DEFAUT;
  const parDefaut =
    actuels.closeOverlay === SHORTCUTS_PAR_DEFAUT.closeOverlay && actuels.upgradeSearch === SHORTCUTS_PAR_DEFAUT.upgradeSearch;

  async function save(shortcuts: ShortcutSettings) {
    setError(null);
    try {
      onSettings(await commands.setShortcuts(shortcuts));
    } catch (err) {
      setError(message(err));
    }
  }

  function onKeyDown(champ: Champ, e: KeyboardEvent) {
    if (capture !== champ) return;
    e.preventDefault();
    e.stopPropagation();
    const touche = lireTouche(e.nativeEvent);
    if (!touche) return;
    setCapture(null);
    void save({ ...actuels, [champ]: touche });
  }

  return (
    <div className="space-y-3 text-sm">
      <ul className="divide-y divide-ink-800">
        <li className="flex items-center gap-3 py-2">
          <span className="flex-1 text-text-muted">Ouvrir l'overlay sur un objet</span>
          <Kbd>Ctrl+C</Kbd>
          <span className="w-28 text-right text-xs text-text-subtle">fixé par le jeu</span>
        </li>
        <li className="flex items-center gap-3 py-2">
          <span className="flex-1 text-text-muted">Prochaine étape du plan Forge</span>
          <Kbd>Ctrl+Alt+C</Kbd>
          <span className="w-28 text-right text-xs text-text-subtle">fixé par le jeu</span>
        </li>
        {LIGNES.map((l) => (
          <li key={l.id} className="flex items-center gap-3 py-2">
            <span className="flex-1 text-text-muted">{l.action}</span>
            {capture === l.id ? (
              <span className="text-xs text-accent">Tape la combinaison…</span>
            ) : (
              <Kbd>{afficherRaccourci(actuels[l.id])}</Kbd>
            )}
            <button
              type="button"
              disabled={!settings}
              onClick={() => {
                setError(null);
                setCapture(capture === l.id ? null : l.id);
              }}
              onKeyDown={(e) => onKeyDown(l.id, e)}
              onBlur={() => setCapture((c) => (c === l.id ? null : c))}
              className={`${btnSecondary} w-28`}
            >
              {capture === l.id ? "Annuler" : "Modifier"}
            </button>
          </li>
        ))}
      </ul>
      <div className="flex items-center gap-3">
        <p className="flex-1 text-xs text-text-subtle">
          Ces touches ne sont prises au jeu que pendant l'affichage de l'overlay. Ajoute Ctrl ou Alt à une lettre (Échap et F1 à F12
          peuvent rester seuls).
        </p>
        {!parDefaut && (
          <button type="button" onClick={() => void save(SHORTCUTS_PAR_DEFAUT)} className={btnSecondary}>
            Raccourcis par défaut
          </button>
        )}
      </div>
      {error && (
        <StatePanel icon="alert" title="Raccourci refusé" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}
