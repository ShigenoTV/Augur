import { Kbd, StatePanel } from "@augur/ui";
import type { AppError } from "@augur/ui";
import type { ForgeTip } from "../bridge";
import { decrireEtat, formatCount, formatEx, formatPercent, libelleCourt } from "../index/forge";

export type ForgeView = { status: "tip"; tip: ForgeTip } | { status: "error"; error: AppError };

function Shell({ children, onClose }: { children: React.ReactNode; onClose?: () => void }) {
  return (
    <div
      className="augur-panel w-[360px] p-3.5 text-text-primary motion-safe:animate-[augur-enter_120ms_var(--ease-out-quick)]"
      style={{ zoom: "var(--augur-scale)" }}
    >
      <p className="mb-2 text-2xs font-semibold uppercase tracking-wider text-accent">
        Forge
      </p>
      {children}
      <footer className="mt-3 flex items-center justify-between gap-2 border-t border-ink-700 pt-2 text-2xs text-text-subtle">
        <button type="button" onClick={onClose} className="flex items-center gap-1 hover:text-text-muted">
          <Kbd>Échap</Kbd> fermer
        </button>
        <span>Plan suivi depuis la fenêtre Augur</span>
      </footer>
    </div>
  );
}

/** Prochaine étape du plan Forge pour l'objet copié en jeu (Ctrl+Alt+C). */
export function ForgeTipPanel({ view, onClose }: { view: ForgeView; onClose?: () => void }) {
  if (view.status === "error") {
    return (
      <Shell onClose={onClose}>
        <StatePanel icon="alert" title="Objet non suivi" variant="warn">
          {view.error.message}
        </StatePanel>
      </Shell>
    );
  }
  const { tip } = view;
  const { conseil } = tip;
  const labels = tip.labels.map(libelleCourt);
  const action = conseil.action;
  return (
    <Shell onClose={onClose}>
      <p className="text-sm font-semibold">{tip.baseNom}</p>
      <p className="text-2xs text-text-subtle">{decrireEtat(conseil.state, labels)}</p>
      <ul className="mt-2 space-y-0.5 text-2xs">
        {tip.mods.map((m, i) => (
          <li
            key={i}
            className={
              m.voulu !== null ? "text-signal-gain" : m.tierTropBas ? "text-signal-warn" : "text-text-muted"
            }
          >
            {m.voulu !== null ? "✓ " : m.tierTropBas ? "▼ " : "· "}
            {m.texte}
            {m.fracture && <span className="text-text-subtle"> (fracturé)</span>}
            {!m.reconnu && <span className="text-text-subtle"> (non reconnu : compté comme mauvais mod)</span>}
          </li>
        ))}
      </ul>
      <div className="augur-divider my-3" />
      {conseil.goalReached ? (
        <StatePanel icon="check" title="Objectif atteint" variant="info">
          Tous les affixes voulus sont sur l'objet. Plus rien à dépenser.
        </StatePanel>
      ) : action ? (
        <div>
          <p className="text-2xs text-text-subtle">Prochaine étape</p>
          <p className="text-lg font-semibold text-accent">
            {action.isAbandon ? "Repartir d'une base neuve" : action.label}
          </p>
          <p className="text-2xs text-text-subtle">
            {action.isAbandon ? "Objet à revendre ou à jeter" : `${formatEx(action.unitCost)} par essai`}
            {conseil.repeat && conseil.repeat.expectedAttempts > 1.05
              ? ` · ~${formatCount(conseil.repeat.expectedAttempts)} essais attendus`
              : ""}
            {` · reste ${formatEx(conseil.costToGo)} (plan entier : ${formatEx(tip.coutPlan)})`}
          </p>
          {conseil.outcomes.length > 0 && (
            <ul className="mt-2 space-y-0.5 text-2xs">
              <li className="text-text-subtle">Résultats possibles du prochain essai :</li>
              {conseil.outcomes.slice(0, 4).map((o, i) => (
                <li key={i} className="flex justify-between gap-2">
                  <span className={o.label.startsWith("Touche") ? "text-signal-gain" : "text-text-muted"}>
                    {libelleCourt(o.label)}
                  </span>
                  <span className="font-mono text-text-subtle">{formatPercent(o.probability)}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : (
        <StatePanel icon="alert" title="Pas d'étape" variant="warn">
          Le plan ne prévoit rien dans cet état.
        </StatePanel>
      )}
      {tip.ilvlObjet !== null && tip.ilvlObjet < tip.ilvlPlan && (
        <p className="mt-2 text-2xs text-signal-warn">
          Niveau d'objet {tip.ilvlObjet} sous celui du plan ({tip.ilvlPlan}) : des paliers manquent, refais le plan.
        </p>
      )}
    </Shell>
  );
}
