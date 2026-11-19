import "@augur/ui/styles.css";
import { cx } from "@augur/ui";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { commands, isTauri, on, type LedgerStatus } from "../bridge";
import { formatPoe1, useAffichagePoe1 } from "../index/devisePoe1";
import { segments } from "./bandeau";

/** Relecture du bilan (maps, rythme) même sans changement de zone. */
const REFRESH_MS = 10_000;

const APERCU: LedgerStatus = {
  active: true,
  startedAt: new Date(Date.now() - 4_520_000).toISOString(),
  strategy: { waystoneTier: null, tablets: [], mechanics: [], note: "" },
  summary: null,
  lastSavedId: null,
  storageError: null,
  gain: { chaos: 1240, chaosPerHour: 1030 },
};

/**
 * Bandeau Atlas Ledger : une ligne fine en haut de l'écran pendant une session. La fenêtre laisse
 * passer les clics ; le cœur Rust l'affiche seulement en jeu, pendant une session, option active.
 */
function Bandeau() {
  const [status, setStatus] = useState<LedgerStatus | null>(isTauri() ? null : APERCU);
  const [now, setNow] = useState(() => Date.now());
  // Même monnaie d'affichage que la fenêtre principale (Chaos ou Divine).
  const { affichage } = useAffichagePoe1();
  const active = status?.active ?? false;

  useEffect(() => {
    if (!isTauri()) return;
    void commands.getLedgerStatus().then(setStatus).catch(() => undefined);
    const unlisten = on.ledgerSession(setStatus);
    return () => void unlisten.then((f) => f());
  }, []);

  useEffect(() => {
    if (!active) return;
    const tick = setInterval(() => setNow(Date.now()), 1_000);
    const refresh = isTauri()
      ? setInterval(() => void commands.getLedgerStatus().then(setStatus).catch(() => undefined), REFRESH_MS)
      : undefined;
    return () => {
      clearInterval(tick);
      if (refresh) clearInterval(refresh);
    };
  }, [active]);

  const items = segments(status, now, (v) => formatPoe1(v, affichage));
  if (items.length === 0) return null;

  return (
    <div className="flex h-screen items-start justify-center pt-0.5">
      <div className="flex items-center gap-3 rounded-full border border-ink-700 bg-ink-950/85 px-3 py-1 text-xs leading-none text-text-primary shadow-lg">
        <span className="flex items-center gap-1.5 text-text-subtle">
          <span className="inline-block size-1.5 animate-pulse rounded-full bg-signal-gain" aria-hidden />
          Ledger
        </span>
        {items.map((s) => (
          <span key={s.label} className="flex items-baseline gap-1">
            <span className="text-2xs text-text-subtle">{s.label}</span>
            <span
              className={cx(
                "font-mono tabular-nums",
                s.tone === "gain" && "text-signal-gain",
                s.tone === "loss" && "text-signal-loss",
              )}
            >
              {s.value}
            </span>
          </span>
        ))}
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Bandeau />
  </StrictMode>,
);
