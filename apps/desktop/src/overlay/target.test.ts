import { describe, expect, it } from "vitest";
import type { TargetProgress } from "../bridge";
import { targetLine, targetMissing } from "./target";

const t = (over: Partial<TargetProgress>): TargetProgress => ({
  slot: "Helmet",
  target: "Doom Visor, Voltaic Helm",
  targetUnique: false,
  status: "improve",
  reached: 3,
  total: 5,
  missingMods: [],
  weakerMods: [],
  currentReached: 1,
  ...over,
});

describe("ligne « vers la cible » de l'overlay", () => {
  it("compte les mods et compare à l'objet porté", () => {
    expect(targetLine(t({}))).toEqual({ text: "Vers la cible (casque) : 3 mods sur 5 · ton objet : 1 sur 5", better: true });
    expect(targetLine(t({ reached: 1, currentReached: 1 })).better).toBeNull();
    expect(targetLine(t({ reached: 0, currentReached: 2 })).better).toBe(false);
    expect(targetLine(t({ currentReached: null })).text).toBe("Vers la cible (casque) : 3 mods sur 5");
  });

  it("parle de l'unique de la cible", () => {
    expect(targetLine(t({ targetUnique: true, status: "reached", target: "Crown of Eyes" })).text).toBe(
      "C'est l'unique de ta cible (casque) : Crown of Eyes",
    );
    expect(targetLine(t({ targetUnique: true, status: "otherItem", target: "Crown of Eyes" })).text).toBe(
      "Ta cible porte Crown of Eyes (casque)",
    );
  });

  it("liste ce qui manque, en bref", () => {
    const m = targetMissing(t({ missingMods: ["a", "b", "c"], weakerMods: [{ target: "x", current: "y" }] }));
    expect(m).toEqual(["manque a", "manque b", "manque c", "… et 1 autre"]);
  });
});
