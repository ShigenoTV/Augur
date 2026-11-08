import { Kbd, StatePanel } from "@augur/ui";
import { useEffect, useState, type KeyboardEvent } from "react";
import { commands, DEFAULT_LEAGUE, isAppError, type Settings, type ShortcutSettings } from "../bridge";
import { afficherRaccourci, lireTouche, SHORTCUTS_PAR_DEFAUT } from "./shortcuts";

const champ =
  "min-w-0 rounded-chip border border-ink-700 bg-ink-950 px-2 py-1.5 text-sm text-text-primary placeholder:text-text-subtle";
const btnPrimary = "rounded-chip bg-accent px-3 py-1.5 text-sm font-semibold text-accent-ink hover:bg-accent-strong disabled:opacity-50";
const btnSecondary = "rounded-chip border border-ink-600 px-3 py-1.5 text-sm text-text-muted hover:bg-ink-800 disabled:opacity-50";

const message = (err: unknown) => (isAppError(err) ? err.message : String(err));

/** Ligue poe.ninja utilisée pour les prix de la Forge. */
export function LeagueCard({ settings, onSettings }: { settings: Settings | null; onSettings: (s: Settings) => void }) {
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const actuelle = settings?.league ?? DEFAULT_LEAGUE;

  useEffect(() => setInput(settings?.league ?? ""), [settings?.league]);

  async function save(league: string | null) {
    setError(null);
    setInfo(null);
    try {
      onSettings(await commands.setLeague(league));
      setInfo("Ligue enregistrée. Actualise les prix dans la Forge pour charger son relevé.");
    } catch (err) {
      setError(message(err));
    }
  }

  return (
    <div className="space-y-3 text-sm">
      <p className="text-text-muted">
        Prix de la Forge relevés sur poe.ninja pour la ligue <span className="font-semibold text-text-primary">{actuelle}</span>.
      </p>
      <div className="flex gap-2">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder={DEFAULT_LEAGUE}
          aria-label="Ligue poe.ninja"
          className={`${champ} w-64`}
        />
        <button
          type="button"
          disabled={!settings || input.trim() === (settings.league ?? "")}
          onClick={() => void save(input.trim() || null)}
          className={btnPrimary}
        >
          Enregistrer
        </button>
        {settings?.league && (
          <button type="button" onClick={() => void save(null)} className={btnSecondary}>
            Revenir à {DEFAULT_LEAGUE}
          </button>
        )}
      </div>
      <p className="text-xs text-text-subtle">Écris le nom exact de la ligue, comme sur poe.ninja (ex. Standard).</p>
      {info && <p className="text-xs text-signal-gain">{info}</p>}
      {error && (
        <StatePanel icon="alert" title="Ligue non enregistrée" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}

type Champ = keyof ShortcutSettings;

const LIGNES: { id: Champ; action: string }[] = [
  { id: "closeOverlay", action: "Fermer l'overlay" },
  { id: "upgradeSearch", action: "Chercher un meilleur objet (sprint 3)" },
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
