import { describe, expect, it } from "vitest";
import { budgetDuJeu, optionsLigues } from "./SettingsCards";

describe("optionsLigues", () => {
  it("reprend la liste de poe.ninja", () => {
    expect(optionsLigues(["Forbidden Rites", "Standard"], "Standard")).toEqual(["Forbidden Rites", "Standard"]);
  });

  it("garde la ligue enregistrée et se replie hors ligne", () => {
    expect(optionsLigues(["Runes of Aldur"], "Forbidden Rites")).toEqual(["Forbidden Rites", "Runes of Aldur"]);
    expect(optionsLigues(null, "Forbidden Rites")).toEqual(["Forbidden Rites", "Standard"]);
    expect(optionsLigues([], "Hardcore")).toEqual(["Hardcore", "Forbidden Rites", "Standard"]);
  });
});

describe("budgetDuJeu", () => {
  const base = {
    betaChannel: false,
    clientLogPath: null,
    league: null,
    shortcuts: { closeOverlay: "Escape", upgradeSearch: "Alt+U", priceCheck: "Alt+D" },
    upgradeBudget: 40,
    poe1: { clientLogPath: null, league: null, upgradeBudget: 3, upgradeBudgetCurrency: "divine" },
  };

  it("lit le budget du jeu actif, dans sa devise", () => {
    expect(budgetDuJeu({ ...base, game: "poe2" })).toEqual({ montant: 40, devise: "exalts" });
    expect(budgetDuJeu({ ...base, game: "poe1" })).toEqual({ montant: 3, devise: "divine" });
    expect(budgetDuJeu({ ...base, game: "poe1", poe1: { clientLogPath: null, league: null } })).toEqual({
      montant: null,
      devise: "chaos",
    });
    expect(budgetDuJeu(null)).toEqual({ montant: null, devise: "exalts" });
  });
});
