import "@augur/ui/styles.css";
import "./main.css";
import {
  Icon,
  ItemTooltip,
  Kbd,
  StatePanel,
  UpdateBanner,
  type OverlayState,
  type ParsedItem,
  type ZoneChanged,
} from "@augur/ui";
import { StrictMode, useEffect, useState, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { commands, isAppError, isTauri, on, type Settings } from "../bridge";
import { useUpdates } from "./useUpdates";

/** Pas de canal bêta tant que les mises à jour passent par GitHub Releases (voir src-tauri/src/updates.rs). */
const BETA_AVAILABLE = false;

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-panel border border-ink-700 bg-ink-900 p-4">
      <h2 className="mb-3 text-xs font-semibold uppercase tracking-wider text-text-subtle">{title}</h2>
      {children}
    </section>
  );
}

function Status({ ok, label }: { ok: boolean; label: string }) {
  return (
    <p className="flex items-center gap-2 text-sm">
      <span className={ok ? "text-signal-gain" : "text-text-subtle"}>
        <Icon name={ok ? "check" : "flat"} size={14} />
      </span>
      {label}
    </p>
  );
}

/**
 * Fenêtre principale (V0) : état de l'application, raccourcis et banc d'essai
 * du parseur. Le planificateur complet est hors périmètre du sprint 1.
 */
function App() {
  const [state, setState] = useState<OverlayState | null>(null);
  const [zone, setZone] = useState<ZoneChanged | null>(null);
  const [text, setText] = useState("");
  const [parsed, setParsed] = useState<ParsedItem | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const updates = useUpdates();

  useEffect(() => {
    if (isTauri()) void commands.getSettings().then(setSettings).catch(() => undefined);
  }, []);

  async function toggleBeta(beta: boolean) {
    try {
      setSettings(await commands.setUpdateChannel(beta));
      void updates.check();
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
    }
  }

  useEffect(() => {
    if (!isTauri()) return;
    const refresh = () => void commands.getOverlayState().then(setState).catch(() => undefined);
    refresh();
    const timer = setInterval(refresh, 2_000);
    const unlisten = on.zoneChanged(setZone);
    return () => {
      clearInterval(timer);
      void unlisten.then((f) => f());
    };
  }, []);

  async function tryParse() {
    setError(null);
    setParsed(null);
    if (!isTauri()) {
      setError("Le banc d'essai du parseur fonctionne dans l'application.");
      return;
    }
    try {
      setParsed(await commands.parseClipboardItem(text));
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
    }
  }

  return (
    <main className="mx-auto max-w-5xl space-y-6 p-8">
      <header className="flex items-baseline gap-3">
        <h1 className="font-display text-2xl font-semibold text-accent">Augur</h1>
        <span className="text-sm text-text-subtle">
          {state ? `v${state.appVersion} · contrat ${state.contractVersion}` : "Upgrade Copilot V0"}
        </span>
      </header>

      <UpdateBanner update={updates.visible} onInstall={() => void updates.install()} onLater={updates.later} />

      <div className="grid grid-cols-3 gap-4">
        <Card title="État">
          <div className="space-y-2">
            <Status ok={Boolean(state?.gameFocused)} label={state?.gameFocused ? "Jeu au premier plan" : "Jeu non détecté"} />
            <Status ok={Boolean(zone)} label={zone ? `Zone\u00a0: ${zone.zone}` : "Client.txt : en attente"} />
            <Status ok={Boolean(state?.dataPatch)} label={state?.dataPatch ? `Données ${state.dataPatch}` : "Données de jeu : à venir"} />
          </div>
        </Card>
        <Card title="Personnage">
          {state?.character ? (
            <p className="text-sm">
              {state.character.name} · {state.character.class} {state.character.level}
            </p>
          ) : (
            <StatePanel icon="user" title="Aucun personnage importé" variant="info">
              L'import par connexion GGG ou code Path of Building arrive au sprint 3.
            </StatePanel>
          )}
        </Card>
        <Card title="Raccourcis">
          <ul className="space-y-2 text-sm text-text-muted">
            <li>
              <Kbd>Ctrl+C</Kbd> sur un objet en jeu : ouvre l'overlay
            </li>
            <li>
              <Kbd>Échap</Kbd> ferme l'overlay (actif seulement quand il est affiché)
            </li>
            <li>
              <Kbd>Alt+U</Kbd> cherche un meilleur objet (sprint 3)
            </li>
          </ul>
          <p className="mt-3 text-xs text-text-subtle">
            Le jeu doit être en mode « fenêtré sans bordure » pour que l'overlay s'affiche par-dessus.
          </p>
        </Card>
      </div>

      <Card title="Mises à jour">
        <div className="flex flex-wrap items-center gap-4 text-sm">
          <button
            type="button"
            onClick={() => void updates.check()}
            disabled={updates.update.phase === "checking" || updates.update.phase === "installing"}
            className="rounded-chip border border-ink-600 px-3 py-1.5 text-text-primary hover:bg-ink-800 disabled:opacity-50"
          >
            Rechercher des mises à jour
          </button>
          <span className="text-text-subtle">
            {updates.update.phase === "checking" && "Vérification…"}
            {updates.update.phase === "upToDate" && "Augur est à jour."}
            {updates.update.phase === "idle" && "Vérification automatique au démarrage puis toutes les 6 heures."}
          </span>
          {BETA_AVAILABLE && (
            <label className="ml-auto flex items-center gap-2 text-text-muted">
              <input
                type="checkbox"
                checked={settings?.betaChannel ?? false}
                disabled={!settings}
                onChange={(e) => void toggleBeta(e.target.checked)}
                className="accent-[var(--color-accent)]"
              />
              Rejoindre la bêta
            </label>
          )}
        </div>
        <p className="mt-2 text-xs text-text-subtle">
          Les mises à jour sont signées et vérifiées avant installation. Rien ne s'installe sans ton accord.
        </p>
      </Card>

      <Card title="Banc d'essai du parseur">
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-2">
            <textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="Colle ici le texte d'un objet copié en jeu (Ctrl+C)."
              className="h-64 w-full resize-none rounded-chip border border-ink-700 bg-ink-950 p-2 font-mono text-xs text-text-primary placeholder:text-text-subtle"
            />
            <button
              type="button"
              onClick={() => void tryParse()}
              className="rounded-chip bg-accent px-3 py-1.5 text-sm font-semibold text-accent-ink hover:bg-accent-strong"
            >
              Analyser
            </button>
          </div>
          <div>
            {parsed && <ItemTooltip item={parsed} />}
            {error && (
              <StatePanel icon="alert" title="Objet non reconnu" variant="warn">
                {error}
              </StatePanel>
            )}
          </div>
        </div>
      </Card>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
