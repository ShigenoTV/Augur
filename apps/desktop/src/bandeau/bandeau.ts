import type { LedgerStatus } from "../bridge";
import { formatDuration } from "../index/ledger";

/** Une case du bandeau : libellé discret et valeur. */
export type Segment = { label: string; value: string; tone?: "gain" | "loss" };

/** Secondes écoulées depuis le début de la session (RFC 3339), à `now` (ms). */
export function elapsedSecs(startedAt: string | null, now: number): number | null {
  if (!startedAt) return null;
  const t = new Date(startedAt).getTime();
  if (Number.isNaN(t)) return null;
  return Math.max(0, Math.floor((now - t) / 1000));
}

const signe = (v: number) => (v > 0 ? "+" : v < 0 ? "−" : "");
const ton = (v: number): Segment["tone"] => (v > 0 ? "gain" : v < 0 ? "loss" : undefined);

/**
 * Contenu du bandeau : durée, maps, puis gain et gain par heure quand des relevés de coffres en
 * donnent un ; sinon le rythme en maps par heure. Vide hors session.
 */
export function segments(
  status: LedgerStatus | null,
  now: number,
  montant: (chaos: number) => string,
): Segment[] {
  if (!status?.active) return [];
  const maps = status.summary?.maps.length ?? 0;
  const out: Segment[] = [
    { label: "Session", value: formatDuration(elapsedSecs(status.startedAt, now)) },
    { label: "Maps", value: String(maps) },
  ];
  const gain = status.gain;
  if (gain) {
    out.push({ label: "Gain", value: `${signe(gain.chaos)}${montant(Math.abs(gain.chaos))}`, tone: ton(gain.chaos) });
    out.push({
      label: "Par heure",
      value: gain.chaosPerHour == null ? "–" : `${signe(gain.chaosPerHour)}${montant(Math.abs(gain.chaosPerHour))}/h`,
      tone: gain.chaosPerHour == null ? undefined : ton(gain.chaosPerHour),
    });
  } else {
    const rate = status.summary?.mapsParHeure;
    out.push({
      label: "Rythme",
      value:
        rate == null || !Number.isFinite(rate)
          ? "–"
          : `${rate.toLocaleString("fr-FR", { minimumFractionDigits: 1, maximumFractionDigits: 1 })} maps/h`,
    });
  }
  return out;
}
