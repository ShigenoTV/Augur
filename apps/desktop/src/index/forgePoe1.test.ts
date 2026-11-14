import { describe, expect, it } from "vitest";
import type { ForgePoe1Base, ForgePoe1Method } from "../bridge";
import { chiffrer, essaisPour, formatChance, formatChaos, libelleBase, prixEffectif } from "./forgePoe1";

const m = (id: string, chance: number, monnaies: [string, number][]): ForgePoe1Method => ({
  id,
  nom: id,
  genre: "chaos",
  chance,
  monnaies: monnaies.map(([nom, quantite]) => ({ nom, quantite })),
  note: "",
});

describe("forge PoE 1", () => {
  it("compte les essais d'une loi géométrique", () => {
    expect(essaisPour(1, 0.9)).toBe(1);
    expect(essaisPour(0.5, 0.5)).toBe(1);
    expect(essaisPour(0.1, 0.9)).toBe(22);
    expect(essaisPour(0, 0.5)).toBe(Infinity);
  });

  it("préfère le prix saisi à poe.ninja", () => {
    expect(prixEffectif("Chaos Orb", { "Chaos Orb": 1 }, {})).toBe(1);
    expect(prixEffectif("Orb of Alteration", { "Orb of Alteration": 0.1 }, { "Orb of Alteration": 0.2 })).toBe(0.2);
    expect(prixEffectif("Inconnue", {}, {})).toBeNull();
  });

  it("classe les méthodes par coût moyen, sans prix à la fin", () => {
    const ninja = { "Chaos Orb": 1, "Orb of Alteration": 0.1, "Orb of Augmentation": 0.05 };
    const r = chiffrer(
      [
        m("chaos", 0.01, [["Chaos Orb", 1]]),
        m("alt", 0.02, [
          ["Orb of Alteration", 1],
          ["Orb of Augmentation", 0.4],
        ]),
        m("essence", 0.5, [["Deafening Essence of Greed", 1]]),
      ],
      ninja,
      {},
    );
    expect(r.map((x) => x.id)).toEqual(["alt", "chaos", "essence"]);
    const [alt, chaos, essence] = r;
    expect(alt?.coutEssai).toBeCloseTo(0.12);
    expect(alt?.coutMoyen).toBeCloseTo(6);
    expect(chaos?.coutMoyen).toBeCloseTo(100);
    expect(essence?.coutMoyen).toBeNull();
    expect(essence?.sansPrix).toEqual(["Deafening Essence of Greed"]);
  });

  it("affiche les coûts et les chances", () => {
    expect(formatChaos(5)).toBe("5,00 c");
    expect(formatChaos(1500, 150).replace(/\s/g, " ")).toBe("1 500 c (10,0 div)");
    expect(formatChaos(null)).toBe("–");
    expect(formatChance(0.25)).toBe("25,0 %");
    expect(formatChance(0.001).replace(/\s/g, " ")).toBe("1 sur 1 000");
  });

  it("départage les bases homonymes par leur implicite", () => {
    const b = (id: string, imp: string): ForgePoe1Base => ({
      id,
      nom: "Two-Stone Ring",
      classe: "Ring",
      niveauDrop: 20,
      implicites: [imp],
      influences: [],
    });
    const bases = [b("a", "+12% feu"), b("b", "+12% froid")];
    expect(libelleBase(bases[0]!, bases)).toBe("Two-Stone Ring : +12% feu (niv. 20)");
  });
});
