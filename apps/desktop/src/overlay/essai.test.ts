import { describe, expect, it } from "vitest";
import type { ForgeChain } from "../bridge";
import { cleBilan, texteBilan } from "./EssaiBilan";

const cout = (v: number | null) => (v == null ? "–" : `${v} ex`);
const chaine = (c: Partial<ForgeChain>): ForgeChain => ({
  verdict: "reussi",
  essais: 1,
  depense: 2,
  reste: 40,
  resteAvant: 100,
  ...c,
});

describe("bilan d'un essai Forge enchaîné", () => {
  it("dit que l'essai a marché, avec le reste avant et après", () => {
    expect(texteBilan(chaine({}), cout)).toEqual({
      titre: "Ça a marché, reste 40 ex",
      detail: "avant : 100 ex · 1 essai · ~2 ex dépensés",
    });
  });
  it("dit que l'essai a raté et ce qui reste", () => {
    expect(texteBilan(chaine({ verdict: "rate", essais: 3, depense: 6, reste: 120 }), cout).titre).toBe(
      "Raté, reste 120 ex",
    );
  });
  it("signale une copie du même objet sans compter d'essai", () => {
    const t = texteBilan(chaine({ verdict: "memeObjet", essais: 0, depense: 0 }), cout);
    expect(t).toEqual({ titre: "Même objet qu'à la copie précédente", detail: "" });
  });
  it("annonce l'objet fini", () => {
    expect(texteBilan(chaine({ verdict: "atteint", essais: 5, depense: 10 }), cout).titre).toBe("Ça a marché : objet fini");
  });
  it("change de clé à chaque essai pour rejouer l'apparition du bandeau", () => {
    expect(cleBilan(chaine({ essais: 2, verdict: "rate" }))).not.toBe(cleBilan(chaine({ essais: 3, verdict: "rate" })));
  });
});
