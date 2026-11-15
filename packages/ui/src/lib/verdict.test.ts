import { describe, expect, it } from "vitest";
import { approximateDelta, downgradeDelta, modelPrice, tradePrice, tradeoffDelta, upgradeDelta } from "../fixtures";
import { computeVerdict, dpsPerExalted, headline, pointsTrend, uncappedResistances } from "./verdict";

describe("verdict", () => {
  it("classe les quatre cas", () => {
    expect(computeVerdict(upgradeDelta).kind).toBe("upgrade");
    expect(computeVerdict(tradeoffDelta).kind).toBe("tradeoff");
    expect(computeVerdict(downgradeDelta).kind).toBe("downgrade");
    expect(
      computeVerdict({ ...upgradeDelta, dps: { before: 1000, after: 1005 } }).kind,
    ).toBe("sidegrade");
    expect(computeVerdict(approximateDelta).kind).toBe("upgrade");
  });

  it("compte les résistances en points", () => {
    expect(pointsTrend({ before: 75, after: 61 })).toBe("down");
    expect(pointsTrend({ before: 30, after: 30 })).toBe("flat");
  });

  it("signale une résistance qui décroche du plafond", () => {
    expect(uncappedResistances(tradeoffDelta).map((r) => r.element)).toEqual(["cold"]);
    expect(uncappedResistances(upgradeDelta)).toEqual([]);
  });

  it("calcule le DPS par exalt, sans inventer de taux de change", () => {
    expect(dpsPerExalted(upgradeDelta, tradePrice)).toBeCloseTo((61_870 - 48_210) / 14);
    expect(dpsPerExalted(upgradeDelta, modelPrice)).toBeUndefined();
    expect(dpsPerExalted(upgradeDelta, modelPrice, 100)).toBeCloseTo((61_870 - 48_210) / 150);
    expect(dpsPerExalted(downgradeDelta, tradePrice)).toBeUndefined();
  });
});

describe("headline", () => {
  it("met le DPS en avant quand il bouge", () => {
    expect(headline(upgradeDelta).stat).toBe("dps");
  });

  it("met l'EHP en avant sur une pièce défensive (DPS stable)", () => {
    const d = { ...upgradeDelta, dps: { before: 0, after: 0 }, ehp: { before: 67.6, after: 68.4 } };
    const h = headline(d);
    expect(h.stat).toBe("ehp");
    expect(h.trend).toBe("up");
    expect(h.ratio).toBeCloseTo(0.8 / 67.6, 6);
  });

  it("garde le DPS quand rien ne bouge ou que l'EHP n'est pas calculé", () => {
    expect(headline({ ...upgradeDelta, dps: { before: 10, after: 10 }, ehp: { before: 50, after: 50 } }).stat).toBe(
      "dps",
    );
    expect(headline({ ...upgradeDelta, dps: { before: 10, after: 10 }, ehp: { before: 0, after: 0 } }).stat).toBe(
      "dps",
    );
  });
});
