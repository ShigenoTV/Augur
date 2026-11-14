import "@augur/ui/styles.css";
import { CopilotOverlay, fixtures, type CopilotView } from "@augur/ui";
import { StrictMode, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { commands, isAppError, isTauri, on } from "../bridge";
import { afficherRaccourci, SHORTCUTS_PAR_DEFAUT } from "../index/shortcuts";
import { ForgeTipPanel, type ForgeView } from "./ForgeTipPanel";
import { Poe1Panel, type Poe1View } from "./Poe1Panel";
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
  // Recherche d'upgrades (Alt+U) : objet affiché et résultat de la dernière demande.
  const shown = useRef<CopilotView>(view);
  shown.current = view;
  const [search, setSearch] = useState<{ ok: boolean; text: string } | null>(null);
  // PoE 1 : objet copié, cote poe.ninja et recherche de trade.
  const [poe1, setPoe1] = useState<Poe1View | null>(null);
  const poe1Shown = useRef<Poe1View | null>(null);
  poe1Shown.current = poe1;
  const openPoe1Trade = () => {
    if (poe1Shown.current?.status !== "ready") return;
    setSearch({ ok: true, text: "Ouverture de la recherche…" });
    void commands
      .openPoe1TradeSearch()
      .then(() => setSearch({ ok: true, text: "Recherche ouverte dans ton navigateur." }))
      .catch((error: unknown) => setSearch({ ok: false, text: isAppError(error) ? error.message : String(error) }));
  };
  const openSearch = () => {
    if (poe1Shown.current) return openPoe1Trade();
    const v = shown.current;
    if (v.status !== "ready" || !v.delta || v.delta.support === "unsupported") return;
    setSearch({ ok: true, text: "Préparation de la recherche…" });
    void commands
      .openUpgradeSearch(v.item)
      .then((r) =>
        setSearch({
          ok: true,
          text: `Recherche ouverte dans ton navigateur${r.withDps ? "" : " (défense seule)"} : ${r.stats.slice(0, 3).join(", ")}${r.stats.length > 3 ? "…" : ""}`,
        }),
      )
      .catch((error: unknown) => setSearch({ ok: false, text: isAppError(error) ? error.message : String(error) }));
  };
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
        setSearch(null);
        setPoe1(null);
        const settings = commands.getSettings().catch(() => null);
        void settings.then((s) => s && setRaccourcis(s.shortcuts));
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
        const startPoe1 = () => {
          setView({ status: "hidden" });
          setPoe1({ status: "loading" });
          void commands
            .getPoe1ItemInfo()
            .then((info) => run.current === id && setPoe1({ status: "ready", info }))
            .catch(
              (error: unknown) =>
                run.current === id && setPoe1({ status: "error", message: isAppError(error) ? error.message : String(error) }),
            );
        };
        const startPoe2 = () => void commands
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
        void settings.then((s) => {
          if (run.current !== id) return;
          if (s?.game === "poe1") return startPoe1();
          startPoe2();
        });
      }),
      on.itemUnrecognized((error) => {
        run.current++;
        setForge(null);
        setPoe1(null);
        setView({ status: "unrecognized", error });
      }),
      on.overlayHidden(() => {
        run.current++;
        setForge(null);
        setPoe1(null);
        setView({ status: "hidden" });
      }),
      on.upgradeSearchRequested(() => openSearch()),
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
      {measuring && !poe1 && (view.status !== "hidden" || forge) && (
        <p className="mb-1 flex items-center gap-1.5 text-[11px] text-text-subtle">
          <span className="inline-block size-1.5 rounded-full bg-signal-loss" aria-hidden />
          Mesures en cours : objet enregistré
        </p>
      )}
      {poe1 ? (
        <Poe1Panel
          view={poe1}
          fermer={hotkeys.close}
          trade={hotkeys.upgradeSearch}
          onClose={() => (isTauri() ? void commands.hideOverlay() : undefined)}
          onTrade={openPoe1Trade}
        />
      ) : forge ? (
        <ForgeTipPanel view={forge} fermer={hotkeys.close} onClose={() => (isTauri() ? void commands.hideOverlay() : undefined)} />
      ) : (
        <CopilotOverlay
          state={view}
          hotkeys={hotkeys}
          onClose={() => (isTauri() ? void commands.hideOverlay() : undefined)}
          onUpgradeSearch={openSearch}
        />
      )}
      {search && !forge && (
        <p className={`mt-1 max-w-[360px] text-[11px] ${search.ok ? "text-text-subtle" : "text-signal-loss"}`}>{search.text}</p>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Overlay />
  </StrictMode>,
);
