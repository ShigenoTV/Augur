import { describe, expect, it } from "vitest";
import { approximateDelta, downgradeDelta, modelPrice, tradePrice, tradeoffDelta, upgradeDelta } from "../fixtures";
import { computeVerdict, dpsPerExalted, pointsTrend, uncappedResistances } from "./verdict";

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
