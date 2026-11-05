import { StatePanel } from "@augur/ui";
import { useEffect, useState } from "react";
import { commands, isAppError, isTauri, on, type MeasureStatus } from "../bridge";
import { describeCounts, shortTime } from "./measures";

/**
 * Session de mesures : Augur garde les objets copiés, les codes Path of
 * Building et les zones de Client.txt, puis exporte le tout en un fichier.
 */
export function MeasureCard() {
  const [status, setStatus] = useState<MeasureStatus | null>(null);
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    void commands.getMeasureStatus().then(setStatus).catch(() => undefined);
    const unlisten = on.measureSession(setStatus);
    return () => void unlisten.then((f) => f());
  }, []);

  async function run(action: () => Promise<MeasureStatus | void>) {
    setError(null);
    if (!isTauri()) {
      setError("La session de mesures fonctionne dans l'application.");
      return;
    }
    try {
      const next = await action();
      if (next) setStatus(next);
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
    }
  }

  async function addNote() {
    const text = note.trim();
    if (!text) return;
    await run(() => commands.addMeasureNote(text));
    setNote("");
  }

  async function copy() {
    setCopied(false);
    await run(commands.copyMeasureExport);
    setCopied(true);
    setTimeout(() => setCopied(false), 3_000);
  }

  const active = status?.active ?? false;
  const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold";

  return (
    <div className="space-y-3 text-sm">
      {active && status ? (
        <>
          <p className="flex items-center gap-2">
            <span className="inline-block size-2 animate-pulse rounded-full bg-signal-loss" aria-hidden />
            Enregistrement depuis {shortTime(status.startedAt)}{" "}: {describeCounts(status.counts)}
          </p>
          <div className="flex gap-2">
            <textarea
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Note libre : DPS du panneau de compétence, résistances, contexte du relevé…"
              className="h-16 min-w-0 flex-1 resize-none rounded-chip border border-ink-700 bg-ink-950 p-2 text-xs text-text-primary placeholder:text-text-subtle"
            />
            <button
              type="button"
              onClick={() => void addNote()}
              disabled={!note.trim()}
              className={`${btn} self-start border border-ink-600 text-text-primary hover:bg-ink-800 disabled:opacity-50`}
            >
              Ajouter la note
            </button>
          </div>
          <button
            type="button"
            onClick={() => void run(commands.stopMeasureSession)}
            className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong`}
          >
            Arrêter et exporter
          </button>
        </>
      ) : (
        <>
          <p className="text-text-muted">
            Pendant la session, Augur garde chaque objet copié en jeu (Ctrl+C et Ctrl+Alt+C), les codes Path of Building
            que tu copies et les zones lues dans Client.txt. Les panneaux du jeu ne sont pas lisibles : note leurs valeurs
            dans le champ prévu.
          </p>
          <button
            type="button"
            onClick={() => void run(commands.startMeasureSession)}
            className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong`}
          >
            Démarrer une session de mesures
          </button>
        </>
      )}

      {status?.lastExport && !active && (
        <div className="space-y-2 rounded-chip border border-ink-700 bg-ink-950 p-3">
          <p>
            Dernier export ({describeCounts(status.lastExport.counts)})
            {" "}:
          </p>
          <p className="font-mono text-xs text-text-primary">{status.lastExport.jsonPath}</p>
          <p className="font-mono text-xs text-text-subtle">{status.lastExport.markdownPath}</p>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={() => void copy()}
              className={`${btn} border border-ink-600 text-text-primary hover:bg-ink-800`}
            >
              Copier dans le presse-papier
            </button>
            {copied && !error && <span className="text-xs text-signal-gain">Copié.</span>}
          </div>
        </div>
      )}

      <p className="text-xs text-text-subtle">
        Rien n'est envoyé : les fichiers restent dans Documents\Augur\mesures. Aussi accessible depuis l'icône Augur de la
        barre des tâches.
      </p>
      {error && (
        <StatePanel icon="alert" title="Session de mesures" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}
