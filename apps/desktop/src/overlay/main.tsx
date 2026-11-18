import "@augur/ui/styles.css";
import { CopilotOverlay, fixtures, type CopilotView } from "@augur/ui";
import { StrictMode, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { commands, isAppError, isTauri, on } from "../bridge";
import { afficherRaccourci, SHORTCUTS_PAR_DEFAUT } from "../index/shortcuts";
import { ForgePoe1TipPanel, type ForgePoe1View } from "./ForgePoe1TipPanel";
import { ForgeTipPanel, type ForgeView } from "./ForgeTipPanel";
import { Poe1Panel, type Poe1EvalView, type Poe1View } from "./Poe1Panel";
import { TargetLine } from "./TargetLine";
import type { TargetProgress } from "../bridge";
import { EVALUATING_DELAY_MS, runCopilot, tradable } from "./copilot";

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
  const [poe1Eval, setPoe1Eval] = useState<Poe1EvalView>({ status: "none" });
  // Plan Forge PoE 1 suivi : l'objet copié est celui du craft en cours.
  const [forgePoe1, setForgePoe1] = useState<ForgePoe1View | null>(null);
  const poe1Shown = useRef<Poe1View | null>(null);
  poe1Shown.current = poe1;
  // Objet copié face à l'objet de la cible (personnage cible de Copilot).
  const [target, setTarget] = useState<TargetProgress | null>(null);
  const targetShown = useRef<TargetProgress | null>(null);
  targetShown.current = target;
  // Alt+U vise l'objet de la cible (Réglages).
  const searchTarget = useRef(false);
  const openTargetSearch = (slot: string) => {
    setSearch({ ok: true, text: "Préparation de la recherche de l'objet cible…" });
    void commands
      .openTargetSearch(slot)
      .then((r) =>
        setSearch({
          ok: true,
          text: r.unique
            ? `Recherche de ${r.unique} ouverte dans ton navigateur.`
            : `Recherche de l'objet cible ouverte dans ton navigateur : ${r.filters.length} mod${r.filters.length > 1 ? "s" : ""}${r.ignored.length > 0 ? ` (${r.ignored.length} non cherché${r.ignored.length > 1 ? "s" : ""})` : ""}.`,
        }),
      )
      .catch((error: unknown) => setSearch({ ok: false, text: isAppError(error) ? error.message : String(error) }));
  };
  const openPoe1Trade = () => {
    if (poe1Shown.current?.status !== "ready") return;
    setSearch({ ok: true, text: "Path of Building pèse les stats de cet emplacement…" });
    void commands
      .openPoe1UpgradeSearch()
      .then((r) =>
        setSearch({
          ok: true,
          text:
            r.stats.length > 0
              ? `Recherche d'upgrades ouverte dans ton navigateur : ${r.stats.slice(0, 3).join(", ")}${r.stats.length > 3 ? "…" : ""}`
              : "Recherche de l'objet ouverte dans ton navigateur.",
        }),
      )
      .catch((error: unknown) => setSearch({ ok: false, text: isAppError(error) ? error.message : String(error) }));
  };
  const openSearch = () => {
    const t = targetShown.current;
    if (searchTarget.current && t && t.status !== "reached") return openTargetSearch(t.slot);
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
  // Cote trade (Alt+D) : une recherche à la fois, résultats ouvrables dans le navigateur.
  const [trade, setTrade] = useState<{ url: string; total: number; ignored: number } | null>(null);
  const tradeBusy = useRef(false);
  const priceCheck = () => {
    const v = shown.current;
    if ((v.status !== "ready" && v.status !== "noCharacter") || !tradable(v.item) || tradeBusy.current) return;
    if (poe1Shown.current) return;
    const id = run.current;
    tradeBusy.current = true;
    setTrade(null);
    setView({ ...v, price: undefined, pricing: "pending", priceNote: undefined });
    void commands
      .tradePriceCheck(v.item)
      .then((r) => {
        if (run.current !== id) return;
        setTrade({ url: r.url, total: r.total, ignored: r.ignored.length });
        setView({
          ...v,
          price: r.price ?? undefined,
          pricing: r.price ? "done" : "unpriced",
          priceNote: r.price ? undefined : "Aucune annonce comparable en ligne.",
        });
      })
      .catch((error: unknown) => {
        if (run.current !== id) return;
        setView({ ...v, price: undefined, pricing: "unavailable", priceNote: isAppError(error) ? error.message : String(error) });
      })
      .finally(() => {
        tradeBusy.current = false;
      });
  };
  const hotkeys = {
    close: afficherRaccourci(raccourcis.closeOverlay),
    upgradeSearch: afficherRaccourci(raccourcis.upgradeSearch),
    priceCheck: afficherRaccourci(raccourcis.priceCheck ?? SHORTCUTS_PAR_DEFAUT.priceCheck),
  };

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
        setTrade(null);
        setPoe1(null);
        setForgePoe1(null);
        setTarget(null);
        const settings = commands.getSettings().catch(() => null);
        void settings.then((s) => {
          if (!s) return;
          setRaccourcis({ ...SHORTCUTS_PAR_DEFAUT, ...s.shortcuts });
          searchTarget.current = s.upgradeSearchTarget ?? false;
        });
        // Où en est cet objet par rapport à celui de la cible (rien sans cible).
        void commands
          .getTargetProgress()
          .then((t) => run.current === id && setTarget(t))
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
        const startPoe1Copilot = () => {
          setView({ status: "hidden" });
          setPoe1({ status: "loading" });
          setPoe1Eval({ status: "loading" });
          // Comparaison avec le build (Path of Building) en parallèle de la cote.
          void commands
            .evaluatePoe1Item()
            .then((evaluation) => run.current === id && setPoe1Eval({ status: "ready", evaluation }))
            .catch((error: unknown) => {
              if (run.current !== id) return;
              if (!isAppError(error)) return setPoe1Eval({ status: "error", message: String(error) });
              if (error.code === "noCharacter") return setPoe1Eval({ status: "noCharacter" });
              if (error.code === "unsupported") return setPoe1Eval({ status: "none" });
              setPoe1Eval({ status: "error", message: error.message });
            });
          void commands
            .getPoe1ItemInfo()
            .then((info) => run.current === id && setPoe1({ status: "ready", info }))
            .catch(
              (error: unknown) =>
                run.current === id && setPoe1({ status: "error", message: isAppError(error) ? error.message : String(error) }),
            );
        };
        // Un plan Forge PoE 1 suivi passe d'abord, comme en PoE 2.
        const startPoe1 = () => {
          setForgePoe1(null);
          void commands
            .getForgePoe1Advice()
            .then((tip) => {
              if (run.current !== id) return;
              if (!tip) return startPoe1Copilot();
              setView({ status: "hidden" });
              setPoe1(null);
              setForgePoe1({ status: "tip", tip });
            })
            .catch((error: unknown) => {
              if (run.current !== id) return;
              if (!isAppError(error)) return startPoe1Copilot();
              setView({ status: "hidden" });
              setPoe1(null);
              setForgePoe1({ status: "error", message: error.message });
            });
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
        setTarget(null);
        setForge(null);
        setForgePoe1(null);
        setPoe1(null);
        setView({ status: "unrecognized", error });
      }),
      on.overlayHidden(() => {
        run.current++;
        setTarget(null);
        setForge(null);
        setForgePoe1(null);
        setPoe1(null);
        setView({ status: "hidden" });
      }),
      on.upgradeSearchRequested(() => openSearch()),
      on.priceCheckRequested(() => priceCheck()),
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
      {forgePoe1 ? (
        <ForgePoe1TipPanel
          view={forgePoe1}
          fermer={hotkeys.close}
          onClose={() => (isTauri() ? void commands.hideOverlay() : undefined)}
        />
      ) : poe1 ? (
        <Poe1Panel
          view={poe1}
          evaluation={poe1Eval}
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
          onPriceCheck={priceCheck}
        />
      )}
      {target && !forge && !forgePoe1 && (poe1 || view.status !== "hidden") && (
        <TargetLine target={target} onSearch={() => openTargetSearch(target.slot)} />
      )}
      {trade && !forge && !forgePoe1 && !poe1 && view.status !== "hidden" && (
        <button
          type="button"
          onClick={() => void commands.openTradeListings(trade.url).catch(() => undefined)}
          className="mt-1 block max-w-[360px] text-left text-[11px] text-text-subtle hover:text-text-muted"
        >
          {trade.total} annonce{trade.total > 1 ? "s" : ""} comparable{trade.total > 1 ? "s" : ""} en ligne : voir sur le trade
          {trade.ignored > 0 && ` (${trade.ignored} mod${trade.ignored > 1 ? "s" : ""} non cherché${trade.ignored > 1 ? "s" : ""})`}
        </button>
      )}
      {search && !forge && !forgePoe1 && (
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
