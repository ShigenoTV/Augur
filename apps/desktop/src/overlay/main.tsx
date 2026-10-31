import "@augur/ui/styles.css";
import { CopilotOverlay, fixtures, type CopilotView } from "@augur/ui";
import { StrictMode, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { commands, isTauri, on } from "../bridge";
import { EVALUATING_DELAY_MS, runCopilot } from "./copilot";

function Overlay() {
  const [view, setView] = useState<CopilotView>(
    isTauri() ? { status: "hidden" } : fixtures.overlayStates[0]!.state,
  );
  const panel = useRef<HTMLDivElement>(null);
  const run = useRef(0);

  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = [
      on.itemCopied(({ text }) => {
        const id = ++run.current;
        const emit = (v: CopilotView) => {
          if (run.current === id) setView(v);
        };
        // L'état « analyse » n'apparaît que si le calcul dépasse 150 ms.
        const slow = setTimeout(() => emit({ status: "evaluating" }), EVALUATING_DELAY_MS);
        void runCopilot(
          text,
          {
            parse: commands.parseClipboardItem,
            overlayState: commands.getOverlayState,
            evaluate: commands.evaluateItem,
            price: commands.estimatePrice,
          },
          (v) => {
            clearTimeout(slow);
            emit(v);
          },
        );
      }),
      on.overlayHidden(() => {
        run.current++;
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
      <CopilotOverlay state={view} onClose={() => (isTauri() ? void commands.hideOverlay() : undefined)} />
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Overlay />
  </StrictMode>,
);
