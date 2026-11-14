import { describe, expect, it } from "vitest";
import { formatPoe1, lireAffichagePoe1 } from "./devisePoe1";

const n = (s: string) => s.replace(/\s/g, " ");

describe("affichage des coûts PoE 1", () => {
  it("affiche en Chaos ou en Divine au cours connu", () => {
    expect(n(formatPoe1(1500))).toBe("1 500 c");
    expect(formatPoe1(1500, { devise: "divine", divine: 150 })).toBe("10,0 div");
    expect(formatPoe1(15, { devise: "divine", divine: 150 })).toBe("0,10 div");
    expect(formatPoe1(0.5, { devise: "divine", divine: 150 })).toBe("< 0,01 div");
    expect(formatPoe1(null)).toBe("–");
  });

  it("reste en Chaos sans cours de la Divine", () => {
    expect(formatPoe1(12, { devise: "divine", divine: 0 })).toBe("12,0 c");
  });

  it("relit un choix enregistré, abîmé ou non", () => {
    expect(lireAffichagePoe1('{"devise":"divine","divine":180}')).toEqual({ devise: "divine", divine: 180 });
    expect(lireAffichagePoe1('{"devise":"exalt","divine":-1}')).toEqual({ devise: "chaos", divine: 0 });
    expect(lireAffichagePoe1("pas du json")).toEqual({ devise: "chaos", divine: 0 });
    expect(lireAffichagePoe1(null)).toEqual({ devise: "chaos", divine: 0 });
  });
});
