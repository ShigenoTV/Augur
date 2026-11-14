import { describe, expect, it } from "vitest";
import type { Settings } from "../bridge";
import { cheminJournalDuJeu, jeuActif, ligueDuJeu } from "./game";

const base: Settings = {
  betaChannel: false,
  clientLogPath: "D:\\poe2",
  league: "Standard",
  shortcuts: { closeOverlay: "Escape", upgradeSearch: "Alt+U" },
  upgradeBudget: null,
  game: "poe2",
  poe1: { clientLogPath: null, league: null },
};

describe("réglages par jeu", () => {
  it("PoE 2 par défaut, avec les réglages de la racine", () => {
    expect(jeuActif(null)).toBe("poe2");
    expect(ligueDuJeu(base)).toBe("Standard");
    expect(cheminJournalDuJeu(base)).toBe("D:\\poe2");
  });

  it("PoE 1 a ses propres réglages", () => {
    const s: Settings = { ...base, game: "poe1" };
    expect(ligueDuJeu(s)).toBe("Standard");
    expect(cheminJournalDuJeu(s)).toBeNull();
    const t: Settings = { ...s, poe1: { clientLogPath: "C:\\poe1", league: " Covens " } };
    expect(ligueDuJeu(t)).toBe("Covens");
    expect(cheminJournalDuJeu(t)).toBe("C:\\poe1");
  });

  it("relit des réglages d'avant le sélecteur", () => {
    const ancien = { ...base, game: undefined, poe1: undefined } as unknown as Settings;
    expect(jeuActif(ancien)).toBe("poe2");
    expect(ligueDuJeu(ancien)).toBe("Standard");
  });
});
