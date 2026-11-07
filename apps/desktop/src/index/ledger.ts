import type { LedgerStrategy, LedgerTime } from "../bridge";

/** « 4 min 05 », « 1 h 02 », « 42 s » : durée en secondes, lisible. */
export function formatDuration(secs: number | null | undefined): string {
  if (secs == null || !Number.isFinite(secs) || secs < 0) return "–";
  const s = Math.round(secs);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ${String(s % 60).padStart(2, "0")}`;
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, "0")}`;
}

/** « 6,4 maps/h » (une décimale, format français). */
export function formatRate(perHour: number | null | undefined): string {
  if (perHour == null || !Number.isFinite(perHour)) return "–";
  return `${perHour.toLocaleString("fr-FR", { minimumFractionDigits: 1, maximumFractionDigits: 1 })} maps/h`;
}

/** Part de chaque activité dans la session, en pourcentages entiers. */
export function timeShares(t: LedgerTime): { map: number; hideout: number; ville: number; autre: number } {
  const total = t.map + t.hideout + t.ville + t.autre;
  const pct = (v: number) => (total > 0 ? Math.round((v * 100) / total) : 0);
  return { map: pct(t.map), hideout: pct(t.hideout), ville: pct(t.ville), autre: pct(t.autre) };
}

/** « Breach, Ritual » → ["Breach", "Ritual"] (virgules ou points-virgules). */
export function parseTags(text: string): string[] {
  const out: string[] = [];
  for (const raw of text.split(/[,;]/)) {
    const tag = raw.trim();
    if (tag && !out.some((t) => t.toLowerCase() === tag.toLowerCase())) out.push(tag);
  }
  return out;
}

/** Résumé d'une stratégie sur une ligne, ou « Sans étiquette ». */
export function describeStrategy(s: LedgerStrategy): string {
  const parts: string[] = [];
  if (s.waystoneTier) parts.push(`T${s.waystoneTier}`);
  if (s.mechanics.length) parts.push(s.mechanics.join(", "));
  if (s.tablets.length) parts.push(`tablettes : ${s.tablets.join(", ")}`);
  return parts.length ? parts.join(" · ") : "Sans étiquette";
}

/** Date et heure locales courtes « 07/11 21:35 » d'un horodatage RFC 3339. */
export function shortDateTime(rfc3339: string | null): string {
  if (!rfc3339) return "";
  const d = new Date(rfc3339);
  if (Number.isNaN(d.getTime())) return "";
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(d.getDate())}/${two(d.getMonth() + 1)} ${two(d.getHours())}:${two(d.getMinutes())}`;
}
