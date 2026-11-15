import { describe, expect, it } from "vitest";
import { ecart, formatCote, ligneGagnante } from "./Poe1Panel";

describe("formatCote", () => {
  it("affiche les Chaos, et les Divines au-delà d'une demie", () => {
    expect(formatCote({ chaos: 157, chaosMax: null, divine: 2, annonces: 46, liens: null })).toBe("≈ 157 c · 2 div");
    expect(formatCote({ chaos: 7.2, chaosMax: null, divine: 0.09, annonces: null, liens: null })).toBe("≈ 7,2 c");
    expect(formatCote({ chaos: 10, chaosMax: 25, divine: null, annonces: 3, liens: null })).toBe("≈ 10 à 25 c");
  });
});

describe("lignes de Path of Building", () => {
  it("compte une baisse comme un gain quand la stat vaut mieux en baisse", () => {
    expect(ligneGagnante({ label: "Life", before: 4000, after: 4072, lowerIsBetter: false })).toBe(true);
    expect(ligneGagnante({ label: "Cast Time", before: 0.5, after: 0.4, lowerIsBetter: true })).toBe(true);
    expect(ligneGagnante({ label: "Mana", before: 600, after: 500, lowerIsBetter: false })).toBe(false);
  });

  it("affiche l'écart signé", () => {
    expect(ecart({ label: "Life", before: 4000, after: 4072, lowerIsBetter: false })).toBe("+72");
    expect(ecart({ label: "Chaos", before: -53, after: -56.5, lowerIsBetter: false })).toBe("−3,5");
    expect(ecart({ label: "DPS", before: 1000, after: 2500, lowerIsBetter: false })).toMatch(/^\+1\s?500$/);
  });
});
