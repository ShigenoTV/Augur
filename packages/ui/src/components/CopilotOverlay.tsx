import { useState } from "react";
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
  className?: string;
}

const defaultHotkeys = { close: "Échap", upgradeSearch: "Alt+U" };

function Shell({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div
      className={cx(
        "augur-panel w-[360px] p-3.5 text-text-primary motion-safe:animate-[augur-enter_120ms_var(--ease-out-quick)]",
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
  className,
}: CopilotOverlayProps) {
  const [showMods, setShowMods] = useState(false);

  if (state.status === "hidden") return null;

  const footer = (
    <footer className="mt-3 flex items-center justify-between gap-2 border-t border-ink-700 pt-2 text-2xs text-text-subtle">
      <button type="button" onClick={onClose} className="flex items-center gap-1 hover:text-text-muted">
        <Kbd>{hotkeys.close}</Kbd> fermer
      </button>
      {state.status === "ready" && state.delta && state.delta.support !== "unsupported" && (
        <button
          type="button"
          onClick={onUpgradeSearch}
          className="flex items-center gap-1 hover:text-text-muted"
          title="Lance une recherche de trade pondérée depuis ton PC"
        >
          <Icon name="search" size={11} /> <Kbd>{hotkeys.upgradeSearch}</Kbd> chercher mieux
        </button>
      )}
    </footer>
  );

  switch (state.status) {
    case "evaluating":
      return (
        <Shell className={className}>
          <div aria-busy="true" aria-label="Analyse de l'objet">
            {state.item ? <ItemHeader item={state.item} /> : <Shimmer className="block h-6 w-48" />}
            <div className="augur-divider my-3" />
            <Shimmer className="block h-4 w-24" />
            <Shimmer className="mt-1.5 block h-8 w-40" />
            <Shimmer className="mt-3 block h-4 w-full" />
            <Shimmer className="mt-1.5 block h-4 w-full" />
          </div>
          {footer}
        </Shell>
      );

    case "noCharacter":
      return (
        <Shell className={className}>
          <ItemHeader item={state.item} />
          <div className="augur-divider my-3" />
          <StatePanel icon="user" title="Aucun personnage importé" variant="info">
            Importe ton personnage depuis la fenêtre Augur (connexion officielle GGG ou code Path of Building) pour
            voir le gain de DPS et d'EHP.
          </StatePanel>
          {footer}
        </Shell>
      );

    case "unrecognized":
      return (
        <Shell className={className}>
          <StatePanel icon="alert" title="Objet non reconnu" variant="warn">
            {state.error.message}
            <p className="mt-1 text-2xs text-text-subtle">Code : {state.error.code}</p>
          </StatePanel>
          {footer}
        </Shell>
      );

    case "disabled":
      return (
        <Shell className={className}>
          <StatePanel icon="pause" title="Copilot en pause" variant="neutral">
            {state.reason}
          </StatePanel>
          {footer}
        </Shell>
      );

    case "ready": {
      const { item, delta, price, pricing, rateLimit, character } = state;
      return (
        <Shell className={className}>
          <div className="flex items-start justify-between gap-2">
            <ItemHeader item={item} />
            {delta && <SupportBadge level={delta.support} />}
          </div>
          {character && (
            <p className="mt-0.5 text-2xs text-text-subtle">
              comparé à{" "}
              <span className="text-text-muted">
                {character.name} ({character.class} {character.level})
              </span>
            </p>
          )}

          <div className="augur-divider my-3" />
          {delta ? (
            <DeltaPanel delta={delta} price={price} exaltedPerDivine={exaltedPerDivine} />
          ) : (
            <StatePanel icon="pause" title="Comparaison en pause" variant="neutral">
              Le calcul du gain est coupé temporairement. Le prix reste disponible.
            </StatePanel>
          )}

          {flags?.tradePriceCheck !== false && (
            <>
              <div className="augur-divider my-3" />
              <PriceTag pricing={pricing} price={price} rateLimit={rateLimit} now={now} />
            </>
          )}

          {item.mods.length > 0 && (
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
        </Shell>
      );
    }
  }
}
