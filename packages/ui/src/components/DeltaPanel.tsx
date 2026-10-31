import type { Confidence, ItemDelta, PriceEstimate, StatDelta } from "../types";
import { cx } from "../lib/cx";
import { formatNumber, formatSignedNumber, formatSignedPercent } from "../lib/format";
import { elementLabel, mechanicLabel, slotLabel } from "../lib/labels";
import {
  computeVerdict,
  dpsPerExalted,
  pointsTrend,
  ratio,
  trend,
  uncappedResistances,
  type Trend,
  type VerdictKind,
} from "../lib/verdict";
import { Icon } from "./Icon";

const trendColor: Record<Trend, string> = {
  up: "text-signal-gain",
  down: "text-signal-loss",
  flat: "text-signal-neutral",
};

const trendIcon = { up: "up", down: "down", flat: "flat" } as const;
const trendWord: Record<Trend, string> = { up: "hausse", down: "baisse", flat: "stable" };

const verdictStyle: Record<VerdictKind, string> = {
  upgrade: "text-signal-gain",
  downgrade: "text-signal-loss",
  tradeoff: "text-signal-warn",
  sidegrade: "text-signal-neutral",
};

const confidenceLabel: Record<Confidence, { text: string; className: string }> = {
  confirmed: { text: "confirmé", className: "text-signal-gain" },
  measured: { text: "mesuré", className: "text-signal-info" },
  assumed: { text: "supposé", className: "text-signal-warn" },
};

type RowKind = "relative" | "points";

function StatRow({
  label,
  delta,
  kind,
  cap,
  emphasis = false,
}: {
  label: string;
  delta: StatDelta;
  kind: RowKind;
  cap?: number;
  emphasis?: boolean;
}) {
  const t = kind === "relative" ? trend(delta) : pointsTrend(delta);
  const diff = delta.after - delta.before;
  const capped = cap !== undefined && delta.after >= cap;
  return (
    <div
      className={cx("grid grid-cols-[1fr_auto_4.5rem] items-baseline gap-x-3", emphasis ? "text-base" : "text-sm")}
      aria-label={`${label} : ${trendWord[t]}`}
    >
      <dt className="truncate text-text-muted">{label}</dt>
      <dd className="text-right tabular-nums text-text-subtle">
        {formatNumber(delta.before)} → {formatNumber(delta.after)}
        {capped && <span className="ml-1 text-2xs">(max)</span>}
      </dd>
      <dd className={cx("flex items-center justify-end gap-1 font-semibold tabular-nums", trendColor[t])}>
        <Icon name={trendIcon[t]} size={10} />
        {kind === "relative" ? formatSignedPercent(ratio(delta)) : `${formatSignedNumber(diff)} pts`}
      </dd>
    </div>
  );
}

export interface DeltaPanelProps {
  delta: ItemDelta;
  price?: PriceEstimate;
  exaltedPerDivine?: number;
}

/**
 * Le cœur de l'overlay : « cet objet améliore-t-il MON personnage ? ».
 * Le verdict et le delta de DPS sont lisibles en un coup d'œil ; le détail
 * suit, puis les alertes (résistances qui décrochent, calcul approximatif).
 */
export function DeltaPanel({ delta, price, exaltedPerDivine }: DeltaPanelProps) {
  if (delta.support === "unsupported") {
    return (
      <div className="flex gap-2 rounded-panel border border-signal-loss/30 bg-signal-loss/5 p-3 text-sm text-text-muted">
        <Icon name="ban" className="mt-0.5 shrink-0 text-signal-loss" />
        <p>
          Ton archétype n'est pas encore pris en charge{" "}: Augur préfère ne rien afficher plutôt qu'un chiffre
          faux. Le prix reste disponible.
        </p>
      </div>
    );
  }

  const verdict = computeVerdict(delta);
  const dpsTrend = trend(delta.dps);
  const warnings = uncappedResistances(delta);
  const perEx = dpsPerExalted(delta, price, exaltedPerDivine);
  const approx = delta.support === "approximate";
  const notes = delta.mechanics.filter((m) => m.confidence !== "confirmed");

  return (
    <section aria-label="Comparaison avec ton équipement" className="space-y-3">
      <div className="flex items-end justify-between gap-3">
        <div>
          <p className={cx("text-xs font-semibold uppercase tracking-wider", verdictStyle[verdict.kind])}>
            {verdict.label}
          </p>
          <p
            className={cx("flex items-center gap-1.5 text-verdict font-bold tabular-nums", trendColor[dpsTrend])}
            aria-label={`DPS ${formatSignedPercent(verdict.dpsRatio)}`}
          >
            <Icon name={trendIcon[dpsTrend]} size={18} />
            {approx && <span className="text-signal-warn">≈</span>}
            {formatSignedPercent(verdict.dpsRatio)}
            <span className="self-end pb-1 text-sm font-medium text-text-muted">DPS</span>
          </p>
        </div>
        {perEx !== undefined && (
          <div className="text-right">
            <p className="text-lg font-semibold tabular-nums text-accent">{formatSignedNumber(perEx)}</p>
            <p className="text-2xs text-text-subtle">DPS par exalt</p>
          </div>
        )}
      </div>

      <p className="flex flex-wrap items-center gap-x-2 text-xs text-text-subtle">
        <span>
          {slotLabel(delta.slot)}
          {delta.weaponSet && ` · set d'armes ${delta.weaponSet === 1 ? "I" : "II"}`}
        </span>
        {delta.equippedName && (
          <span className="truncate">
            remplace <span className="text-text-muted">{delta.equippedName}</span>
          </span>
        )}
      </p>

      <dl className="space-y-1">
        <StatRow label="DPS total" delta={delta.dps} kind="relative" emphasis />
        <StatRow label="EHP" delta={delta.ehp} kind="relative" emphasis />
        {delta.resistances.map((r) => (
          <StatRow key={r.element} label={elementLabel(r.element)} delta={r} kind="points" cap={r.cap} />
        ))}
        {delta.spirit && <StatRow label="Spirit disponible" delta={delta.spirit} kind="points" />}
      </dl>

      {warnings.length > 0 && (
        <ul className="space-y-1" aria-label="Alertes">
          {warnings.map((w) => (
            <li
              key={w.element}
              className="flex items-center gap-2 rounded-chip bg-signal-loss/10 px-2 py-1 text-xs font-medium text-signal-loss"
            >
              <Icon name="alert" size={12} className="shrink-0" />
              {elementLabel(w.element)} sous le plafond ({w.after} / {w.cap})
            </li>
          ))}
        </ul>
      )}

      {notes.length > 0 && (
        <div
          className={cx(
            "rounded-chip border px-2 py-1.5 text-xs text-text-muted",
            approx ? "border-signal-warn/30 bg-signal-warn/5" : "border-ink-700",
          )}
        >
          <p className={cx("mb-0.5 flex items-center gap-1 font-semibold", approx ? "text-signal-warn" : "text-text-muted")}>
            <Icon name="approx" size={12} /> {approx ? "Calcul approximatif" : "Mécaniques à confirmer"}
          </p>
          <ul className="space-y-0.5">
            {notes.map((n) => (
              <li key={n.mechanic} className="flex justify-between gap-2">
                <span>{mechanicLabel(n.mechanic)}</span>
                <span className={cx("shrink-0", confidenceLabel[n.confidence].className)}>
                  {confidenceLabel[n.confidence].text}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
