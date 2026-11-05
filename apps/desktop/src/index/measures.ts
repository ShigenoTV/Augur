import type { MeasureCounts } from "../bridge";

function plural(n: number, one: string, many: string): string {
  return `${n} ${n > 1 ? many : one}`;
}

/** « 3 objets, 1 code PoB, 2 zones, 0 note » : ce que la session a gardé. */
export function describeCounts(c: MeasureCounts): string {
  return [
    plural(c.items, "objet", "objets"),
    plural(c.pobCodes, "code PoB", "codes PoB"),
    plural(c.zones, "zone", "zones"),
    plural(c.notes, "note", "notes"),
  ].join(", ");
}

/** Heure locale « 15:30 » d'un horodatage RFC 3339. */
export function shortTime(rfc3339: string | null): string {
  if (!rfc3339) return "";
  const d = new Date(rfc3339);
  if (Number.isNaN(d.getTime())) return "";
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}
