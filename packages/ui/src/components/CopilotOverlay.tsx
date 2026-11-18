import { useEffect, useState } from "react";
import type { CopilotView, FeatureFlags } from "../types";
import { cx } from "../lib/cx";
import { DeltaPanel } from "./DeltaPanel";
import { Icon } from "./Icon";
import { ItemHeader, ItemTooltip } from "./ItemTooltip";
import { PriceTag, Shimmer } from "./PriceTag";
import { Kbd, StatePanel } from "./StatePanel";
import { SupportBadge } from "./SupportBadge";

export interface CopilotOverlayProps {
  state: CopilotView;
  flags?: FeatureFlags;
  /** Taux de conversion pour « DPS par exalt » quand le prix est en divines. */
  exaltedPerDivine?: number;
  /** Raccourcis affichés en pied (doivent refléter les réglages). */
  hotkeys?: { close: string; upgradeSearch: string };
  /** Horloge injectable pour des captures déterministes. */
  now?: Date;
  onClose?: () => void;
  onUpgradeSearch?: () => void;
  /**
   * Version compacte (Réglages) : verdict, gain principal, prix et alertes ;
   * le bouton « détails » déplie la vue complète pour l'objet affiché.
   */
  compact?: boolean;
  /** Contrôle ajouté au pied (bouton d'épinglage de l'overlay). */
  footerExtra?: React.ReactNode;
  className?: string;
}

const defaultHotkeys = { close: "Échap", upgradeSearch: "Alt+U" };

function Shell({ children, compact, className }: { children: React.ReactNode; compact?: boolean; className?: string }) {
  return (
    <div
      className={cx(
        "augur-panel text-text-primary motion-safe:animate-[augur-enter_120ms_var(--ease-out-quick)]",
        compact ? "w-[280px] p-2.5" : "w-[360px] p-3.5",
        className,
      )}
      style={{ zoom: "var(--augur-scale)" }}
    >
      {children}
    </div>
  );
}

/**
 * Vue principale d'Upgrade Copilot. Affichée près du curseur après un Ctrl+C
 * sur un objet en jeu ; chaque état du contrat a un rendu dédié.
 */
export function CopilotOverlay({
  state,
  flags,
  exaltedPerDivine,
  hotkeys = defaultHotkeys,
  now,
  onClose,
  onUpgradeSearch,
  compact: compactSetting = false,
  footerExtra,
  className,
}: CopilotOverlayProps) {
  const [showMods, setShowMods] = useState(false);
  // « détails » en mode compact : vaut pour l'objet affiché seulement.
  const [expanded, setExpanded] = useState(false);
  const shownItem = "item" in state ? state.item : undefined;
  useEffect(() => setExpanded(false), [shownItem]);

  if (state.status === "hidden") return null;
  const compact = compactSetting && !expanded;
  const sh = (children: React.ReactNode) => (
    <Shell compact={compact} className={className}>
      {children}
    </Shell>
  );

  const footer = (
    <footer
      className={cx(
        "flex items-center justify-between gap-2 border-t border-ink-700 pt-2 text-2xs text-text-subtle",
        compact ? "mt-2" : "mt-3",
      )}
    >
      <button type="button" onClick={onClose} className="flex items-center gap-1 hover:text-text-muted">
        <Kbd>{hotkeys.close}</Kbd> fermer
      </button>
      {compactSetting && state.status === "ready" && (
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          aria-expanded={expanded}
          className="hover:text-text-muted"
        >
          {expanded ? "réduire" : "détails"}
        </button>
      )}
      {state.status === "ready" && state.delta && state.delta.support !== "unsupported" && (
        <button
          type="button"
          onClick={onUpgradeSearch}
          className="flex items-center gap-1 hover:text-text-muted"
          title="Lance une recherche de trade pondérée depuis ton PC"
        >
          <Icon name="search" size={11} /> <Kbd>{hotkeys.upgradeSearch}</Kbd> {compact ? "chercher" : "chercher mieux"}
        </button>
      )}
      {footerExtra}
    </footer>
  );
  const divider = <div className={cx("augur-divider", compact ? "my-2" : "my-3")} />;

  switch (state.status) {
    case "evaluating":
      return sh(
        <>
          <div aria-busy="true" aria-label="Analyse de l'objet">
            {state.item ? <ItemHeader item={state.item} /> : <Shimmer className="block h-6 w-48" />}
            {divider}
            <Shimmer className="block h-4 w-24" />
            <Shimmer className="mt-1.5 block h-8 w-40" />
            <Shimmer className="mt-3 block h-4 w-full" />
            <Shimmer className="mt-1.5 block h-4 w-full" />
          </div>
          {footer}
        </>,
      );

    case "noCharacter":
      return sh(
        <>
          <ItemHeader item={state.item} />
          {divider}
          <StatePanel icon="user" title="Aucun personnage importé" variant="info">
            Importe ton personnage depuis la fenêtre Augur (connexion officielle GGG ou code Path of Building) pour
            voir le gain de DPS et d'EHP.
          </StatePanel>
          {footer}
        </>,
      );

    case "unrecognized":
      return sh(
        <>
          <StatePanel icon="alert" title="Objet non reconnu" variant="warn">
            {state.error.message}
            <p className="mt-1 text-2xs text-text-subtle">Code : {state.error.code}</p>
          </StatePanel>
          {footer}
        </>,
      );

    case "disabled":
      return sh(
        <>
          <StatePanel icon="pause" title="Copilot en pause" variant="neutral">
            {state.reason}
          </StatePanel>
          {footer}
        </>,
      );

    case "ready": {
      const { item, delta, price, pricing, rateLimit, character } = state;
      return sh(
        <>
          <div className="flex items-start justify-between gap-2">
            <ItemHeader item={item} />
            {delta && <SupportBadge level={delta.support} />}
          </div>
          {character && !compact && (
            <p className="mt-0.5 text-2xs text-text-subtle">
              comparé à{" "}
              <span className="text-text-muted">
                {character.name} ({character.class} {character.level})
              </span>
            </p>
          )}

          {divider}
          {delta ? (
            <DeltaPanel delta={delta} price={price} exaltedPerDivine={exaltedPerDivine} compact={compact} />
          ) : (
            <StatePanel icon="pause" title="Comparaison en pause" variant="neutral">
              Le calcul du gain est coupé temporairement. Le prix reste disponible.
            </StatePanel>
          )}

          {flags?.tradePriceCheck !== false && (
            <>
              {divider}
              <PriceTag pricing={pricing} price={price} rateLimit={rateLimit} now={now} />
            </>
          )}

          {item.mods.length > 0 && !compact && (
            <div className="mt-3">
              <button
                type="button"
                onClick={() => setShowMods((v) => !v)}
                aria-expanded={showMods}
                className="text-2xs text-text-subtle hover:text-text-muted"
              >
                {showMods ? "Masquer" : "Afficher"} les {item.mods.length} mods
              </button>
              {showMods && <ItemTooltip item={item} className="mt-2" />}
            </div>
          )}

          {footer}
        </>,
      );
    }
  }
}
