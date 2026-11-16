import { describe, expect, it } from "vitest";
import type { GapItem, GapReport } from "../bridge";
import { formatStat, itemSummary, percentGap, pobSlotLabel, progress, sortItems } from "./gap";

const item = (over: Partial<GapItem>): GapItem => ({
  slot: "Helmet",
  current: "A",
  target: "B",
  targetUnique: false,
  status: "improve",
  targetMods: 4,
  missingMods: [],
  weakerMods: [],
  ...over,
});

describe("écart actuel → cible", () => {
  it("traduit les emplacements de PoB", () => {
    expect(pobSlotLabel("Body Armour")).toBe("Armure");
    expect(pobSlotLabel("Ring 2")).toBe("Anneau droit");
    expect(pobSlotLabel("Flask 3")).toBe("Flacon 3");
    expect(pobSlotLabel("Jewel 26725")).toBe("Joyau");
    expect(pobSlotLabel("Inconnu")).toBe("Inconnu");
  });

  it("formate les chiffres et l'écart en %", () => {
    expect(formatStat(null)).toBe("—");
    expect(formatStat(2_500_000)).toMatch(/^2,5\sM$/);
    expect(percentGap(1000, 1350)).toBe("+35 %");
    expect(percentGap(1000, 800)).toBe("−20 %");
    expect(percentGap(1000, 1001)).toBe("=");
    expect(percentGap(null, 10)).toBeNull();
    expect(percentGap(0, 10)).toBeNull();
  });

  it("résume un emplacement", () => {
    expect(itemSummary(item({ status: "reached" }))).toBe("Atteint");
    expect(itemSummary(item({ missingMods: ["a", "b"], weakerMods: [{ target: "x", current: "y" }] }))).toBe(
      "2 mods manquants, 1 plus faible",
    );
    expect(itemSummary(item({ status: "otherItem", targetUnique: true, target: "Crown of Eyes" }))).toBe(
      "Unique à obtenir : Crown of Eyes",
    );
    expect(itemSummary(item({ status: "empty", current: null }))).toBe("Rien d'équipé");
  });

  it("trie les emplacements à travailler d'abord", () => {
    const items = [
      item({ slot: "Helmet", status: "reached" }),
      item({ slot: "Gloves", missingMods: ["a"] }),
      item({ slot: "Boots", missingMods: ["a", "b"] }),
      item({ slot: "Belt", status: "empty" }),
    ];
    expect(sortItems(items).map((i) => i.slot)).toEqual(["Belt", "Boots", "Gloves", "Helmet"]);
    const report = { items } as GapReport;
    expect(progress(report)).toEqual({ reached: 1, total: 4 });
  });
});
