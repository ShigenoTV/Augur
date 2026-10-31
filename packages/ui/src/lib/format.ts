import { currencyShort } from "./labels";

const NNBSP = " ";

const intFmt = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 0 });
const oneDecimal = new Intl.NumberFormat("fr-FR", {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});
const compactFmt = new Intl.NumberFormat("fr-FR", {
  notation: "compact",
  maximumFractionDigits: 1,
});

/** Signe explicite : le signe porte l'information autant que la couleur. */
function sign(value: number): string {
  if (value > 0) return "+";
  if (value < 0) return "−"; // vrai signe moins, plus lisible que le tiret
  return "±";
}

/** « +12,4 % », « −3,0 % », « ±0,0 % ». */
export function formatSignedPercent(ratio: number): string {
  const pct = ratio * 100;
  const rounded = Math.round(pct * 10) / 10;
  return `${sign(rounded)}${oneDecimal.format(Math.abs(rounded))}${NNBSP}%`;
}

/** « 1 234 » ; au-delà de 100 000 : « 123,4 k ». */
export function formatNumber(value: number): string {
  if (Math.abs(value) >= 100_000) return compactFmt.format(value);
  return intFmt.format(value);
}

/** « +1 234 », « −56 ». */
export function formatSignedNumber(value: number): string {
  const rounded = Math.round(value);
  return `${sign(rounded)}${formatNumber(Math.abs(rounded))}`;
}

/** « 12 ex », « 1,5 div ». */
export function formatPrice(value: number, currency: string): string {
  const fmt = value < 10 && !Number.isInteger(value) ? oneDecimal : intFmt;
  return `${fmt.format(value)}${NNBSP}${currencyShort(currency)}`;
}

/** Âge d'une observation : « à l'instant », « il y a 4 min », « il y a 2 h ». */
export function formatAge(observedAt: string, now: Date = new Date()): string {
  const ms = now.getTime() - new Date(observedAt).getTime();
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 1) return "à l'instant";
  if (minutes < 60) return `il y a ${minutes}${NNBSP}min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 48) return `il y a ${hours}${NNBSP}h`;
  return `il y a ${Math.floor(hours / 24)}${NNBSP}j`;
}

/** « 4 s », « 1 min 05 ». */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  if (total < 60) return `${total}${NNBSP}s`;
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}${NNBSP}min${NNBSP}${String(s).padStart(2, "0")}`;
}
