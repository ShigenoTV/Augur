import "@augur/ui/styles.css";
import { CopilotOverlay, fixtures, type CopilotView } from "@augur/ui";
import { StrictMode, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { commands, isAppError, isTauri, on } from "../bridge";
import { afficherRaccourci, SHORTCUTS_PAR_DEFAUT } from "../index/shortcuts";
import { ForgeTipPanel, type ForgeView } from "./ForgeTipPanel";
import { EVALUATING_DELAY_MS, runCopilot } from "./copilot";

function Overlay() {
  const [view, setView] = useState<CopilotView>(
    isTauri() ? { status: "hidden" } : fixtures.overlayStates[0]!.state,
  );
  const panel = useRef<HTMLDivElement>(null);
  const run = useRef(0);
  const [measuring, setMeasuring] = useState(false);
  // Plan Forge suivi : l'objet copié est celui du craft en cours.
  const [forge, setForge] = useState<ForgeView | null>(null);
  // Raccourcis choisis dans Réglages, relus à chaque objet copié.
  const [raccourcis, setRaccourcis] = useState(SHORTCUTS_PAR_DEFAUT);
  const hotkeys = { close: afficherRaccourci(raccourcis.closeOverlay), upgradeSearch: afficherRaccourci(raccourcis.upgradeSearch) };

  useEffect(() => {
    if (!isTauri()) return;
    void commands
      .getMeasureStatus()
      .then((s) => setMeasuring(s.active))
      .catch(() => undefined);
    const unlisten = on.measureSession((s) => setMeasuring(s.active));
    return () => void unlisten.then((f) => f());
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = [
      on.itemCopied((item) => {
        const id = ++run.current;
        void commands
          .getSettings()
          .then((s) => setRaccourcis(s.shortcuts))
          .catch(() => undefined);
        const emit = (v: CopilotView) => {
          if (run.current === id) setView(v);
        };
        const startCopilot = () => {
          // L'état « analyse » n'apparaît que si le calcul dépasse 150 ms.
          const slow = setTimeout(() => emit({ status: "evaluating" }), EVALUATING_DELAY_MS);
          void runCopilot(
            item,
            {
              overlayState: commands.getOverlayState,
              evaluate: commands.evaluateItem,
              price: commands.estimatePrice,
            },
            (v) => {
              clearTimeout(slow);
              emit(v);
            },
          );
        };
        // Un plan Forge suivi passe d'abord : l'objet copié peut être le craft en cours.
        setForge(null);
        void commands
          .getForgeAdvice()
          .then((tip) => {
            if (run.current !== id) return;
            if (!tip) return startCopilot();
            setView({ status: "hidden" });
            setForge({ status: "tip", tip });
          })
          .catch((error: unknown) => {
            if (run.current !== id) return;
            if (!isAppError(error)) return startCopilot();
            setView({ status: "hidden" });
            setForge({ status: "error", error });
          });
      }),
      on.itemUnrecognized((error) => {
        run.current++;
        setForge(null);
        setView({ status: "unrecognized", error });
      }),
      on.overlayHidden(() => {
        run.current++;
        setForge(null);
        setView({ status: "hidden" });
      }),
      on.upgradeSearchRequested(() => {
        // Recherche d'upgrades sous budget : sprint 3 (trade depuis le client).
      }),
    ];
    return () => unlisten.forEach((p) => void p.then((f) => f()));
  }, []);

  // La fenêtre épouse le panneau : aucune zone transparente ne vole les clics au jeu.
  useLayoutEffect(() => {
    if (!isTauri() || !panel.current) return;
    const el = panel.current;
    const observer = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) void commands.fitOverlay(Math.ceil(r.width), Math.ceil(r.height));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={panel} className="inline-block">
      {measuring && (view.status !== "hidden" || forge) && (
        <p className="mb-1 flex items-center gap-1.5 text-[11px] text-text-subtle">
          <span className="inline-block size-1.5 rounded-full bg-signal-loss" aria-hidden />
          Mesures en cours : objet enregistré
        </p>
      )}
      {forge ? (
        <ForgeTipPanel view={forge} fermer={hotkeys.close} onClose={() => (isTauri() ? void commands.hideOverlay() : undefined)} />
      ) : (
        <CopilotOverlay state={view} hotkeys={hotkeys} onClose={() => (isTauri() ? void commands.hideOverlay() : undefined)} />
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Overlay />
  </StrictMode>,
);
