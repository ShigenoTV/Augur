import type { ItemDelta, PriceEstimate, ResistanceDelta, StatDelta } from "../types";

/** En dessous de ce seuil (1 %), un écart n'est pas significatif. */
export const SIGNIFICANT_RATIO = 0.01;

export type Trend = "up" | "down" | "flat";

export type VerdictKind = "upgrade" | "downgrade" | "tradeoff" | "sidegrade";

export interface Verdict {
  kind: VerdictKind;
  label: string;
  dpsRatio: number;
  ehpRatio: number;
}

export function ratio(delta: StatDelta): number {
  if (delta.before === 0) return delta.after === 0 ? 0 : Math.sign(delta.after);
  return (delta.after - delta.before) / Math.abs(delta.before);
}

/** Tendance d'une valeur relative (DPS, EHP) : écart en pourcentage. */
export function trend(delta: StatDelta, threshold = SIGNIFICANT_RATIO): Trend {
  const r = ratio(delta);
  if (r > threshold) return "up";
  if (r < -threshold) return "down";
  return "flat";
}

/** Tendance d'une valeur en points (résistances, Spirit) : écart absolu. */
export function pointsTrend(delta: StatDelta): Trend {
  const d = delta.after - delta.before;
  if (d > 0) return "up";
  if (d < 0) return "down";
  return "flat";
}

const labels: Record<VerdictKind, string> = {
  upgrade: "Amélioration",
  downgrade: "Régression",
  tradeoff: "Compromis",
  sidegrade: "Équivalent",
};

export function computeVerdict(delta: ItemDelta): Verdict {
  const dps = trend(delta.dps);
  const ehp = trend(delta.ehp);
  let kind: VerdictKind;
  if (dps === "flat" && ehp === "flat") kind = "sidegrade";
  else if (dps !== "down" && ehp !== "down") kind = "upgrade";
  else if (dps !== "up" && ehp !== "up") kind = "downgrade";
  else kind = "tradeoff";
  return { kind, label: labels[kind], dpsRatio: ratio(delta.dps), ehpRatio: ratio(delta.ehp) };
}

/**
 * Résistances qui passent sous leur plafond avec cet objet :
 * c'est l'alerte la plus coûteuse à rater en jeu.
 */
export function uncappedResistances(delta: ItemDelta): ResistanceDelta[] {
  return delta.resistances.filter((r) => r.before >= r.cap && r.after < r.cap);
}

/**
 * DPS gagné par exalt dépensé, sur le prix médian. `exaltedPerDivine`
 * convertit les prix en divines ; sans taux, on ne calcule rien plutôt
 * que d'inventer.
 */
export function dpsPerExalted(
  delta: ItemDelta,
  price: PriceEstimate | undefined,
  exaltedPerDivine?: number,
): number | undefined {
  if (!price || price.median <= 0) return undefined;
  const gain = delta.dps.after - delta.dps.before;
  if (gain <= 0) return undefined;
  let exalted: number | undefined;
  if (price.currency === "exalted") exalted = price.median;
  else if (price.currency === "divine" && exaltedPerDivine) exalted = price.median * exaltedPerDivine;
  if (exalted === undefined) return undefined;
  return gain / exalted;
}
