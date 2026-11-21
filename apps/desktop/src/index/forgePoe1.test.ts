import { describe, expect, it } from "vitest";
import type { ForgePoe1Base, ForgePoe1Etape, ForgePoe1Method, ForgePoe1Plan } from "../bridge";
import {
  chiffrer,
  decisions,
  decrireEtape,
  essaisPour,
  formatChance,
  arbrePoe1,
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

const directes = (branches: ForgePoe1Etape["issues"]["branches"], autresIssues = 0, nombreAutres = 0) => ({
  branches,
  autresIssues,
  nombreAutres,
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
  id: 0,
  cout: 1,
  relance: null,
  issues: { branches: [], autresIssues: 0, nombreAutres: 0 },
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

describe("arbre du plan PoE 1", () => {
  // Normal -> Alchemy -> Rare relancé au Chaos (70 % sur place), 20 % fini, 10 % vers un Rare à 1 voulu.
  const plan: ForgePoe1Plan = {
    coutMoyen: 50,
    depart: "Orb of Alchemy",
    actions: [],
    monnaies: [
      { nom: "Chaos Orb", quantite: 40 },
      { nom: "Orb of Alchemy", quantite: 1 },
    ],
    sansPrix: [],
    racine: 0,
    relances: [],
    simulation: null,
    etapes: [
      etape({
        id: 0,
        rarete: "Normal",
        action: "Orb of Alchemy",
        reste: 50,
        cout: 0.5,
        issues: directes([{ vers: 1, probabilite: 1 }]),
      }),
      etape({
        id: 1,
        rarete: "Rare",
        action: "Chaos Orb",
        reste: 49.5,
        passages: 5,
        issues: directes(
          [
            { vers: 1, probabilite: 0.7 },
            { vers: null, probabilite: 0.2 },
            { vers: 2, probabilite: 0.09 },
          ],
          0.01,
          3,
        ),
      }),
      etape({
        id: 2,
        rarete: "Rare",
        voulus: [0],
        metamod: "Prefixes Cannot Be Changed",
        action: "Chaos Orb",
        reste: 60,
      }),
    ],
  };
  const { plan: p, decrire } = arbrePoe1(plan, ["Vie"], { "Chaos Orb": 1, "Orb of Alchemy": 0.5 });

  it("met chaque situation dans une case, avec ses issues hors relance sur place", () => {
    expect(p.rootId).toBe("e0");
    const rare = p.nodes.e1!;
    expect(rare.kind).toBe("action");
    if (rare.kind !== "action") return;
    expect(rare.repeat?.expectedAttempts).toBeCloseTo(1 / 0.3);
    expect(rare.branches.map((b) => [b.to, b.kind])).toEqual([
      ["fini", "success"],
      ["e2", "failure"],
    ]);
    expect(rare.branches[0]!.probability).toBeCloseTo(0.2 / 0.3);
    expect(rare.mergedMinorProbability).toBeCloseTo(0.01 / 0.3);
    expect(rare.mergedMinorCount).toBe(3);
    expect(p.nodes.fini?.kind).toBe("terminal");
    expect(p.nodes.fini?.expectedVisits).toBeCloseTo(1);
  });

  it("décrit les cases avec le metamod et chiffre la liste de courses", () => {
    expect(decrire(p.nodes.e2!)).toBe("Rare · Vie · metamod « Prefixes Cannot Be Changed »");
    expect(decrire(p.nodes.e0!)).toBe("Normal · aucun mod voulu");
    expect(p.shopping.map((l) => [l.label, l.expectedCost])).toEqual([
      ["Chaos Orb", 40],
      ["Orb of Alchemy", 0.5],
    ]);
  });

  it("regroupe les situations à relance identique en une case qui boucle sur elle-même", () => {
    const loi = directes([
      { vers: 1, probabilite: 0.5 },
      { vers: 2, probabilite: 0.3 },
      { vers: null, probabilite: 0.2 },
    ]);
    const p2 = arbrePoe1(
      {
        ...plan,
        relances: [loi],
        etapes: [
          etape({
            id: 0,
            rarete: "Normal",
            action: "Orb of Alchemy",
            reste: 6,
            issues: directes([{ vers: 1, probabilite: 1 }]),
          }),
          etape({ id: 1, rarete: "Rare", action: "Chaos Orb", reste: 5, passages: 3, relance: 0 }),
          etape({ id: 2, rarete: "Rare", voulus: [0], action: "Chaos Orb", reste: 5, passages: 2, relance: 0 }),
        ],
      },
      ["Vie"],
      {},
    ).plan;
    expect(Object.keys(p2.nodes).sort()).toEqual(["e0", "e1", "fini"]);
    const g = p2.nodes.e1!;
    if (g.kind !== "action") throw new Error("case d'action attendue");
    expect(g.expectedVisits).toBe(5);
    expect(g.repeat?.selfLoopProbability).toBeCloseTo(0.8);
    expect(g.branches.map((b) => [b.to, b.kind])).toEqual([["fini", "success"]]);
    expect(g.branches[0]!.probability).toBeCloseTo(1);
    expect(p2.nodes.fini?.expectedVisits).toBeCloseTo(1);
  });
});

describe("crafter ou acheter (PoE 1)", () => {
  it("compare en Chaos avec une marge d'équivalence", async () => {
    const { comparerAchatPoe1, versChaos } = await import("./ForgePoe1Suivi");
    expect(comparerAchatPoe1(100, 200)).toEqual({ ecart: 100, verdict: "crafter" });
    expect(comparerAchatPoe1(200, 100)).toEqual({ ecart: -100, verdict: "acheter" });
    expect(comparerAchatPoe1(100, 105).verdict).toBe("equivalent");
    expect(versChaos(2, "divine", 150)).toBe(300);
    expect(versChaos(2, "divine", 0)).toBe(2);
    expect(versChaos(40, "chaos", 150)).toBe(40);
  });
});
