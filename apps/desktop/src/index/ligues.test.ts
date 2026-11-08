import { describe, expect, it } from "vitest";
import { optionsLigues } from "./SettingsCards";

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
