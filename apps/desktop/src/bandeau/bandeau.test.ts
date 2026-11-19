import { describe, expect, it } from "vitest";
import type { LedgerStatus } from "../bridge";
import { elapsedSecs, segments } from "./bandeau";

const base: LedgerStatus = {
  active: true,
  startedAt: "2026-11-19T20:00:00Z",
  strategy: { waystoneTier: null, tablets: [], mechanics: [], note: "" },
  summary: null,
  lastSavedId: null,
  storageError: null,
};
const t = (iso: string) => new Date(iso).getTime();
const chaos = (v: number) => `${v} c`;

describe("bandeau Ledger", () => {
  it("compte la durée depuis le début de la session", () => {
    expect(elapsedSecs("2026-11-19T20:00:00Z", t("2026-11-19T21:02:30Z"))).toBe(3750);
    expect(elapsedSecs(null, 0)).toBeNull();
    expect(elapsedSecs("pas une date", 0)).toBeNull();
    // Horloge en retard sur le début : jamais de durée négative.
    expect(elapsedSecs("2026-11-19T20:00:00Z", t("2026-11-19T19:59:00Z"))).toBe(0);
  });

  it("vide hors session", () => {
    expect(segments(null, 0, chaos)).toEqual([]);
    expect(segments({ ...base, active: false }, 0, chaos)).toEqual([]);
  });

  it("durée, maps et rythme sans relevé de coffres", () => {
    const s = segments(
      { ...base, summary: { maps: [{}, {}, {}], mapsParHeure: 6.4 } as unknown as LedgerStatus["summary"] },
      t("2026-11-19T20:30:05Z"),
      chaos,
    );
    expect(s.map((x) => x.value)).toEqual(["30 min 05", "3", "6,4 maps/h"]);
  });

  it("gain et gain par heure avec deux relevés", () => {
    const s = segments({ ...base, gain: { chaos: 600, chaosPerHour: 1200 } }, t("2026-11-19T20:30:00Z"), chaos);
    expect(s.slice(2)).toEqual([
      { label: "Gain", value: "+600 c", tone: "gain" },
      { label: "Par heure", value: "+1200 c/h", tone: "gain" },
    ]);
    const perte = segments({ ...base, gain: { chaos: -40, chaosPerHour: null } }, 0, chaos);
    expect(perte.slice(2)).toEqual([
      { label: "Gain", value: "−40 c", tone: "loss" },
      { label: "Par heure", value: "–", tone: undefined },
    ]);
  });
});
