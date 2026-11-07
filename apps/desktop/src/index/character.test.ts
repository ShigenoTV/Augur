import { describe, expect, it } from "vitest";
import type { CharacterInfo } from "../bridge";
import { describeSkill } from "./character";

const base: CharacterInfo = {
  summary: { name: "Build Arc", class: "Stormweaver", level: 90, league: "", source: "pobCode" },
  importedAt: "2026-11-07T10:00:00Z",
  skill: { name: "Arc", level: 21, quality: 20 },
  supports: [],
  skillSupported: true,
  equippedSlots: [],
  passives: 0,
  treeVersion: null,
};

describe("describeSkill", () => {
  it("décrit la gemme comparée", () => {
    expect(describeSkill(base)).toBe("Compétence comparée : Arc niveau 21, qualité 20 %");
    expect(describeSkill({ ...base, skill: { name: "Arc", level: 20, quality: 0 } })).toBe(
      "Compétence comparée : Arc niveau 20",
    );
    expect(describeSkill({ ...base, skill: null })).toBe("Aucune compétence active dans le build");
  });
});
