import { describe, expect, it } from "vitest";
import type { ForgePoe1Base, ForgePoe1Etape, ForgePoe1Method } from "../bridge";
import {
  chiffrer,
  decisions,
  decrireEtape,
  essaisPour,
  formatChance,
  formatChaos,
  libelleBase,
  prixEffectif,
  prixPourPlan,
  trouverEtape,
} from "./forgePoe1";

const m = (id: string, chance: number, monnaies: [string, number][]): ForgePoe1Method => ({
  id,
  nom: id,
  genre: "chaos",
  chance,
  monnaies: monnaies.map(([nom, quantite]) => ({ nom, quantite })),
  note: "",
  finition: null,
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

  it("envoie au solveur la cote, corrigée par les saisies", () => {
    expect(prixPourPlan({ "Chaos Orb": 1, "Exalted Orb": 0.4 }, { "Exalted Orb": 0.5, "Divine Orb": 90 })).toEqual({
      "Chaos Orb": 1,
      "Exalted Orb": 0.5,
      "Divine Orb": 90,
    });
  });

  it("classe les méthodes par coût moyen, sans prix à la fin", () => {
    const ninja = {
      "Chaos Orb": 1,
      "Orb of Alteration": 0.1,
      "Orb of Augmentation": 0.05,
    };
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

  it("ajoute le craft d'établi une seule fois", () => {
    const avec = {
      ...m("chaos+etabli", 0.1, [["Chaos Orb", 1]]),
      finition: {
        texte: "+(41-55) to maximum Life",
        monnaies: [{ nom: "Regal Orb", quantite: 2 }],
      },
    };
    const [r] = chiffrer([avec], { "Chaos Orb": 1, "Regal Orb": 0.5 }, {});
    expect(r?.coutFinition).toBeCloseTo(1);
    expect(r?.coutMoyen).toBeCloseTo(11);
    const [sans] = chiffrer([avec], { "Chaos Orb": 1 }, {});
    expect(sans?.coutMoyen).toBeNull();
    expect(sans?.sansPrix).toEqual(["Regal Orb"]);
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

const etape = (p: Partial<ForgePoe1Etape>): ForgePoe1Etape => ({
  rarete: "Magique",
  voulus: [],
  mauvaisTiers: [],
  autresPrefixes: 0,
  autresSuffixes: 0,
  metamod: null,
  action: "Orb of Alteration",
  reste: 100,
  passages: 1,
  ...p,
});

describe("pas à pas PoE 1", () => {
  const noms = ["Vie", "Feu"];
  const etapes = [
    etape({ rarete: "Normal", action: "Orb of Transmutation", passages: 1 }),
    etape({ autresSuffixes: 1, passages: 40 }),
    etape({ autresPrefixes: 1, passages: 30 }),
    etape({ voulus: [0], action: "Orb of Augmentation", passages: 5 }),
    etape({ voulus: [1], action: "Orb of Augmentation", passages: 6 }),
    etape({ voulus: [0, 1], action: "Regal Orb", passages: 2 }),
    etape({
      rarete: "Rare",
      voulus: [1, 0],
      mauvaisTiers: [],
      metamod: "Prefixes Cannot Be Changed",
      action: "Orb of Scouring",
      reste: 42,
    }),
  ];

  it("retrouve la situation de l'objet, ordre des mods indifférent", () => {
    const e = trouverEtape(etapes, {
      rarete: "Rare",
      voulus: ["present", "present"],
      autresPrefixes: 0,
      autresSuffixes: 0,
      metamod: "Prefixes Cannot Be Changed",
    });
    expect(e?.action).toBe("Orb of Scouring");
    expect(
      trouverEtape(etapes, {
        rarete: "Rare",
        voulus: ["present", "bas"],
        autresPrefixes: 0,
        autresSuffixes: 0,
        metamod: null,
      }),
    ).toBeNull();
  });

  it("résume en règles : action par défaut, puis les exceptions les plus fréquentes d'abord", () => {
    const r = decisions(etapes, noms);
    expect(r.map((d) => [d.rarete, d.action, d.parDefaut])).toEqual([
      ["Normal", "Orb of Transmutation", false],
      ["Magique", "Orb of Alteration", true],
      ["Magique", "Orb of Augmentation", false],
      ["Magique", "Regal Orb", false],
      ["Rare", "Orb of Scouring", true],
    ]);
    expect(r[2]?.situations).toEqual(["Feu", "Vie"]);
    expect(r[3]?.situations).toEqual(["Vie + Feu"]);
  });

  it("décrit une situation", () => {
    expect(
      decrireEtape(
        etape({
          voulus: [0],
          mauvaisTiers: [1],
          autresPrefixes: 1,
          autresSuffixes: 2,
          metamod: "Suffixes Cannot Be Changed",
        }),
        noms,
      ),
    ).toBe("Vie · tier trop bas : Feu · 1 autre préfixe, 2 autres suffixes · metamod « Suffixes Cannot Be Changed »");
    expect(decrireEtape(etape({}), noms)).toBe("aucun mod voulu");
  });
});
