import { Kbd, StatePanel } from "@augur/ui";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  commands,
  DEFAULT_LEAGUE,
  isAppError,
  isTauri,
  on,
  OVERLAY_PAR_DEFAUT,
  OVERLAY_SCALE_MAX,
  OVERLAY_SCALE_MIN,
  type OverlaySettings,
  type Settings,
  type ShortcutSettings,
} from "../bridge";
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
/** Devise et montant du budget de recherche du jeu actif. */
export function budgetDuJeu(settings: Settings | null): { montant: number | null; devise: "exalts" | "chaos" | "divine" } {
  if (!settings) return { montant: null, devise: "exalts" };
  if (settings.game === "poe1") {
    return {
      montant: settings.poe1.upgradeBudget ?? null,
      devise: settings.poe1.upgradeBudgetCurrency === "divine" ? "divine" : "chaos",
    };
  }
  return { montant: settings.upgradeBudget, devise: "exalts" };
}

const NOM_DEVISE = { exalts: "exalts", chaos: "chaos", divine: "divines" } as const;

export function UpgradeBudgetCard({ settings, onSettings }: { settings: Settings | null; onSettings: (s: Settings) => void }) {
  const [saisie, setSaisie] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const actuel = budgetDuJeu(settings);
  const poe1 = settings?.game === "poe1";
  const [devisePoe1, setDevisePoe1] = useState<"chaos" | "divine" | null>(null);
  const devise = poe1 ? (devisePoe1 ?? (actuel.devise as "chaos" | "divine")) : "exalts";
  const valeur = saisie ?? (actuel.montant ? String(actuel.montant) : "");

  async function choisirMode(cible: boolean) {
    setError(null);
    setInfo(null);
    try {
      onSettings(await commands.setUpgradeSearchTarget(cible));
      setInfo(cible ? "Alt+U cherchera l'objet de ta cible." : "Alt+U cherchera mieux que ton objet.");
    } catch (err) {
      setError(message(err));
    }
  }

  async function enregistrer() {
    setError(null);
    setInfo(null);
    const texte = valeur.trim();
    const budget = texte === "" ? null : Number(texte);
    if (budget !== null && (!Number.isInteger(budget) || budget < 0)) {
      setError(`Indique un nombre entier de ${NOM_DEVISE[devise]}, ou laisse vide pour chercher sans limite.`);
      return;
    }
    try {
      onSettings(await commands.setUpgradeBudget(budget || null, poe1 ? (devise as "chaos" | "divine") : null));
      setSaisie(null);
      setDevisePoe1(null);
      setInfo(budget ? `Budget enregistré : ${budget} ${NOM_DEVISE[devise]} au plus.` : "Recherche sans limite de prix.");
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
          aria-label={`Budget en ${NOM_DEVISE[devise]}`}
          className={`${champ} w-32 disabled:opacity-50`}
        />
        {poe1 ? (
          <select
            value={devise}
            onChange={(e) => setDevisePoe1(e.target.value as "chaos" | "divine")}
            aria-label="Devise du budget"
            className={champ}
          >
            <option value="chaos">chaos</option>
            <option value="divine">divines</option>
          </select>
        ) : (
          <span className="text-text-muted">exalts</span>
        )}
        <button type="button" onClick={() => void enregistrer()} disabled={!settings} className={btnSecondary}>
          Enregistrer
        </button>
      </div>
      <div className="space-y-1 pt-1">
        <p className="text-text-muted">Ce que cherche le raccourci de recherche (Alt+U par défaut) :</p>
        <div role="radiogroup" aria-label="Mode de la recherche d'upgrades" className="flex gap-2">
          {(
            [
              [false, "Mieux que mon objet"],
              [true, "L'objet de ma cible"],
            ] as const
          ).map(([cible, nom]) => {
            const actif = (settings?.upgradeSearchTarget ?? false) === cible;
            return (
              <button
                key={nom}
                type="button"
                role="radio"
                aria-checked={actif}
                disabled={!settings}
                onClick={() => void choisirMode(cible)}
                className={`rounded-chip border px-3 py-1.5 text-sm disabled:opacity-50 ${actif ? "border-accent bg-accent text-accent-ink" : "border-ink-600 text-text-primary hover:bg-ink-800"}`}
              >
                {nom}
              </button>
            );
          })}
        </div>
        <p className="text-xs text-text-subtle">
          « L'objet de ma cible » cherche l'objet du personnage cible pour l'emplacement de l'objet copié (l'unique, ou ses mods à
          80 % de leur valeur). Sans objet cible à cet emplacement, Alt+U cherche mieux que ton objet.
        </p>
      </div>
      {info && <p className="text-xs text-signal-gain">{info}</p>}
      {error && (
        <StatePanel icon="alert" title="Réglage non enregistré" variant="warn">
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

/** Version compacte, taille et épinglage de l'overlay (communs aux deux jeux). */
export function OverlayCard({ settings, onSettings }: { settings: Settings | null; onSettings: (s: Settings) => void }) {
  const [error, setError] = useState<string | null>(null);
  const actuel: OverlaySettings = settings?.overlay ?? OVERLAY_PAR_DEFAUT;
  const [taille, setTaille] = useState<number | null>(null);
  const affichee = taille ?? actuel.scale;

  // Épinglé ou libéré depuis l'overlay : la case suit.
  const reglages = useRef(settings);
  reglages.current = settings;
  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = on.overlaySettings((overlay) => {
      if (reglages.current) onSettings({ ...reglages.current, overlay });
    });
    return () => void unlisten.then((f) => f());
  }, [onSettings]);

  async function enregistrer(change: () => Promise<Settings>) {
    setError(null);
    try {
      onSettings(await change());
    } catch (err) {
      setError(message(err));
    }
  }
  const affichage = (compact: boolean, scale: number) =>
    void enregistrer(() => commands.setOverlayDisplay(compact, scale)).then(() => setTaille(null));

  return (
    <div className="space-y-3 text-sm">
      <label className="flex items-start gap-2 text-text-muted">
        <input
          type="checkbox"
          checked={actuel.compact}
          disabled={!settings}
          onChange={(e) => affichage(e.target.checked, actuel.scale)}
          className="mt-1 accent-[var(--color-accent)]"
        />
        <span>
          Overlay compact
          <span className="block text-xs text-text-subtle">
            Verdict, gain principal, prix et alertes seulement. Le bouton « détails » de l'overlay déplie le reste
            pour l'objet affiché.
          </span>
        </span>
      </label>
      <label className="flex items-start gap-2 text-text-muted">
        <input
          type="checkbox"
          checked={actuel.pinned}
          disabled={!settings}
          onChange={(e) => void enregistrer(() => commands.setOverlayPinned(e.target.checked))}
          className="mt-1 accent-[var(--color-accent)]"
        />
        <span>
          Épingler l'overlay
          <span className="block text-xs text-text-subtle">
            Il reste ouvert quand le curseur s'éloigne et garde sa place : glisse son bord du haut pour le déplacer.
            Les objets copiés suivants s'y affichent. {afficherRaccourci(settings?.shortcuts.closeOverlay ?? "Escape")}{" "}
            le ferme. Aussi réglable depuis l'overlay (bouton « épingler »).
          </span>
        </span>
      </label>
      <div className="flex flex-wrap items-center gap-3 text-text-muted">
        <label htmlFor="overlay-taille">Taille</label>
        <input
          id="overlay-taille"
          type="range"
          min={OVERLAY_SCALE_MIN}
          max={OVERLAY_SCALE_MAX}
          step={10}
          value={affichee}
          disabled={!settings}
          onChange={(e) => setTaille(Number(e.target.value))}
          onPointerUp={() => taille !== null && affichage(actuel.compact, taille)}
          onKeyUp={() => taille !== null && affichage(actuel.compact, taille)}
          className="w-48 accent-[var(--color-accent)]"
        />
        <span className="w-12 tabular-nums text-text-primary">{affichee} %</span>
        {affichee !== 100 && (
          <button type="button" onClick={() => affichage(actuel.compact, 100)} className={btnSecondary}>
            Taille d'origine
          </button>
        )}
      </div>
      {error && (
        <StatePanel icon="alert" title="Réglage non enregistré" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}
