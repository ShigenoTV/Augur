import { describe, expect, it } from "vitest";
import { describeCounts, shortTime } from "./measures";

describe("session de mesures", () => {
  it("résume les relevés au singulier et au pluriel", () => {
    expect(describeCounts({ items: 3, pobCodes: 1, zones: 0, notes: 2 })).toBe(
      "3 objets, 1 code PoB, 0 zone, 2 notes",
    );
  });

  it("affiche l'heure de début, ou rien si elle manque", () => {
    expect(shortTime(null)).toBe("");
    expect(shortTime("pas une date")).toBe("");
    expect(shortTime("2026-11-05T15:30:12Z")).toMatch(/^\d{2}:\d{2}$/);
  });
});
