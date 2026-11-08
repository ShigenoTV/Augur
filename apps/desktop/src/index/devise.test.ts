import { describe, expect, it } from "vitest";
import { formatCout, lireAffichage, tauxDepuisPrix } from "./devise";

const taux = { exalt: 1, chaos: 56, divine: 635 };

describe("formatCout", () => {
  it("affiche en Exalted par défaut", () => {
    expect(formatCout(2018)).toBe(`${(2018).toLocaleString("fr-FR")} ex`);
    expect(formatCout(null)).toBe("–");
  });

  it("convertit en Chaos et en Divine", () => {
    expect(formatCout(112, { devise: "chaos", taux })).toBe(`${(2).toLocaleString("fr-FR", { minimumFractionDigits: 2 })} c`);
    expect(formatCout(6350, { devise: "divine", taux })).toBe(`${(10).toLocaleString("fr-FR", { minimumFractionDigits: 1 })} div`);
  });

  it("reste en Exalted sans taux connu", () => {
    expect(formatCout(5, { devise: "divine", taux: { exalt: 1, chaos: 0, divine: 0 } })).toMatch(/ ex$/);
  });
});

describe("taux et stockage", () => {
  it("lit les taux depuis la table de prix", () => {
    expect(tauxDepuisPrix({ chaos: 56, divine: 635, exalt: 1 })).toEqual(taux);
    expect(tauxDepuisPrix({ chaos: -1 })).toEqual({ exalt: 1, chaos: 0, divine: 0 });
  });

  it("répare un enregistrement abîmé", () => {
    expect(lireAffichage("pas du json").devise).toBe("exalt");
    expect(lireAffichage(JSON.stringify({ devise: "mirror" })).devise).toBe("exalt");
    expect(lireAffichage(JSON.stringify({ devise: "divine", taux: { divine: 635 } }))).toEqual({
      devise: "divine",
      taux: { exalt: 1, chaos: 0, divine: 635 },
    });
  });
});
