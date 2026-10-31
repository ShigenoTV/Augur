import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import "../src/styles/index.css";
import { accent, CopilotOverlay, fixtures, ink, ItemTooltip, mod, rarity, signal, text, UpdateBanner } from "../src";

/**
 * Fonds de jeu simulés. Ils ne remplacent pas les captures réelles (tâche UX S2),
 * mais exposent l'overlay aux deux extrêmes : zone sombre et zone très claire.
 */
const backgrounds = {
  sombre: "radial-gradient(circle at 30% 20%, #2a2219 0%, #0d0b09 60%), #0d0b09",
  clair: "radial-gradient(circle at 70% 30%, #fff6dc 0%, #d9cfb8 45%, #a99f8a 100%)",
  chargé:
    "repeating-linear-gradient(45deg, #6b5b3e 0 14px, #c9b98f 14px 28px, #2d4a3a 28px 42px, #e8e1cf 42px 56px)",
} as const;

type Bg = keyof typeof backgrounds;

function Swatch({ name, value }: { name: string; value: string }) {
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className="size-6 rounded-chip border border-ink-600" style={{ background: value }} />
      <span className="text-text-muted">{name}</span>
      <code className="ml-auto font-mono text-text-subtle">{value}</code>
    </div>
  );
}

function TokenGroup({ title, values }: { title: string; values: Record<string, string> }) {
  return (
    <div className="augur-panel space-y-1.5 p-3">
      <h3 className="mb-2 text-sm font-semibold">{title}</h3>
      {Object.entries(values).map(([k, v]) => (
        <Swatch key={k} name={k} value={v} />
      ))}
    </div>
  );
}

function App() {
  const [bg, setBg] = useState<Bg>("sombre");
  const view = new URLSearchParams(location.search).get("view");
  const only = new URLSearchParams(location.search).get("state");
  const forcedBg = new URLSearchParams(location.search).get("bg") as Bg | null;
  const activeBg = forcedBg ?? bg;

  if (view === "updates") {
    return (
      <main className="min-h-screen space-y-4 bg-ink-950 p-6">
        <h1 className="font-display text-lg font-semibold text-accent">Augur · mise à jour dans l'application</h1>
        <UpdateBanner update={{ phase: "available", info: fixtures.updateInfo }} />
        <UpdateBanner
          update={{ phase: "installing", info: fixtures.updateInfo, progress: { downloaded: 7_340_032, total: 12_582_912 } }}
        />
        <UpdateBanner update={{ phase: "error", message: "Impossible de vérifier les mises à jour pour le moment (hors ligne)." }} />
      </main>
    );
  }

  if (view === "tokens") {
    return (
      <main className="min-h-screen bg-ink-950 p-6">
        <h1 className="mb-4 font-display text-lg font-semibold text-accent">Augur · tokens</h1>
        <div className="grid grid-cols-3 gap-4">
          <TokenGroup title="Surfaces" values={Object.fromEntries(Object.entries(ink).map(([k, v]) => [`ink-${k}`, v]))} />
          <TokenGroup title="Textes et accent" values={{ ...text, accent: accent.DEFAULT, "accent-strong": accent.strong }} />
          <TokenGroup title="Signal du verdict" values={signal} />
          <TokenGroup title="Rareté (jeu)" values={rarity} />
          <TokenGroup title="Mods (jeu)" values={mod} />
          <div className="space-y-3">
            <ItemTooltip item={fixtures.rareHelmet} />
            <ItemTooltip item={fixtures.uniqueGloves} />
          </div>
        </div>
      </main>
    );
  }

  const states = only ? fixtures.overlayStates.filter((s) => s.id === only) : fixtures.overlayStates;

  return (
    <main className="min-h-screen p-6" style={{ background: backgrounds[activeBg] }}>
      {!only && (
        <header className="augur-panel mb-6 flex items-center gap-4 p-3">
          <h1 className="font-display text-lg font-semibold text-accent">Augur · Upgrade Copilot</h1>
          <span className="text-xs text-text-subtle">Maquettes avec données fictives</span>
          <div className="ml-auto flex gap-1">
            {(Object.keys(backgrounds) as Bg[]).map((b) => (
              <button
                key={b}
                type="button"
                onClick={() => setBg(b)}
                className={`rounded-chip px-2 py-1 text-xs ${b === activeBg ? "bg-accent text-accent-ink" : "bg-ink-800 text-text-muted"}`}
              >
                fond {b}
              </button>
            ))}
            <a href="?view=tokens" className="rounded-chip bg-ink-800 px-2 py-1 text-xs text-text-muted">
              tokens
            </a>
            <a href="?view=updates" className="rounded-chip bg-ink-800 px-2 py-1 text-xs text-text-muted">
              mises à jour
            </a>
          </div>
        </header>
      )}
      <div className="flex flex-wrap items-start gap-6">
        {states.map(({ id, title, state }) => (
          <figure key={id} id={id} className="space-y-2">
            {!only && <figcaption className="augur-panel inline-block px-2 py-1 text-xs text-text-muted">{title}</figcaption>}
            <CopilotOverlay state={state} now={fixtures.NOW} exaltedPerDivine={110} />
          </figure>
        ))}
      </div>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
