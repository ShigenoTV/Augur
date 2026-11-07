import { describe, expect, it } from "vitest";
import { describeStrategy, formatDuration, formatRate, parseTags, shortDateTime, timeShares } from "./ledger";

describe("Atlas Ledger", () => {
  it("formate les durées", () => {
    expect(formatDuration(null)).toBe("–");
    expect(formatDuration(-1)).toBe("–");
    expect(formatDuration(42)).toBe("42 s");
    expect(formatDuration(245)).toBe("4 min 05");
    expect(formatDuration(3_720)).toBe("1 h 02");
  });

  it("formate le rythme en maps par heure", () => {
    expect(formatRate(null)).toBe("–");
    expect(formatRate(6.4)).toBe("6,4 maps/h");
    expect(formatRate(12)).toBe("12,0 maps/h");
  });

  it("répartit le temps en pourcentages", () => {
    expect(timeShares({ map: 0, hideout: 0, ville: 0, autre: 0 })).toEqual({ map: 0, hideout: 0, ville: 0, autre: 0 });
    expect(timeShares({ map: 540, hideout: 60, ville: 0, autre: 0 })).toEqual({ map: 90, hideout: 10, ville: 0, autre: 0 });
  });

  it("lit les étiquettes saisies", () => {
    expect(parseTags(" Breach, ritual ;  ; breach")).toEqual(["Breach", "ritual"]);
    expect(parseTags("")).toEqual([]);
  });

  it("résume une stratégie", () => {
    expect(describeStrategy({ waystoneTier: null, tablets: [], mechanics: [], note: "" })).toBe("Sans étiquette");
    expect(describeStrategy({ waystoneTier: 15, tablets: ["Breach"], mechanics: ["Breach", "Ritual"], note: "x" })).toBe(
      "T15 · Breach, Ritual · tablettes : Breach",
    );
  });

  it("affiche date et heure courtes", () => {
    expect(shortDateTime(null)).toBe("");
    expect(shortDateTime("n'importe quoi")).toBe("");
    expect(shortDateTime("2026-11-07T19:35:00Z")).toMatch(/^\d{2}\/\d{2} \d{2}:\d{2}$/);
  });
});
