import { StatePanel } from "@augur/ui";
import { useState } from "react";
import { commands, isAppError, type ClientLogStatus, type Settings } from "../bridge";

/** Réglage manuel du chemin de Client.txt, et diagnostic de la recherche. */
export function ClientLogCard({
  status,
  settings,
  onSettings,
}: {
  status: ClientLogStatus | null;
  settings: Settings | null;
  onSettings: (s: Settings) => void;
}) {
  const [input, setInput] = useState(settings?.clientLogPath ?? "");
  const [error, setError] = useState<string | null>(null);

  async function save(path: string | null) {
    setError(null);
    try {
      onSettings(await commands.setClientLogPath(path));
      if (path === null) setInput("");
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
    }
  }

  return (
    <div className="space-y-3 text-sm">
      <p className="text-text-muted">
        {status?.watching && status.path ? (
          <>
            Fichier suivi{"\u00a0"}: <code className="font-mono text-xs text-text-primary">{status.path}</code>
          </>
        ) : (
          "Augur n'a pas encore trouvé Client.txt."
        )}
      </p>
      <div className="flex gap-2">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Dossier du jeu, ex. D:\SteamLibrary\steamapps\common\Path of Exile 2"
          className="min-w-0 flex-1 rounded-chip border border-ink-700 bg-ink-950 px-2 py-1.5 font-mono text-xs text-text-primary placeholder:text-text-subtle"
        />
        <button
          type="button"
          onClick={() => void save(input)}
          disabled={!input.trim()}
          className="rounded-chip bg-accent px-3 py-1.5 font-semibold text-accent-ink hover:bg-accent-strong disabled:opacity-50"
        >
          Utiliser ce chemin
        </button>
        {settings?.clientLogPath && (
          <button
            type="button"
            onClick={() => void save(null)}
            className="rounded-chip border border-ink-600 px-3 py-1.5 text-text-muted hover:bg-ink-800"
          >
            Détection automatique
          </button>
        )}
      </div>
      <p className="text-xs text-text-subtle">
        Tu peux coller le dossier du jeu, son dossier « logs » ou le chemin complet de Client.txt.
      </p>
      {error && (
        <StatePanel icon="alert" title="Chemin refusé" variant="warn">
          {error}
        </StatePanel>
      )}
      {!status?.watching && status && status.searched.length > 0 && (
        <details className="text-xs text-text-subtle">
          <summary className="cursor-pointer">Emplacements essayés ({status.searched.length})</summary>
          <ul className="mt-1 max-h-40 overflow-auto font-mono">
            {status.searched.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}
