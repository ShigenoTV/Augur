import { describe, expect, it } from "vitest";
import type { ForgeAffix, ForgePlan } from "../bridge";
import {
  arbre,
  basesParClasse,
  dateReleve,
  decrireEtat,
  etapes,
  formatCount,
  formatEx,
  formatPercent,
  libelleCourt,
  lirePrix,
  peutAjouter,
} from "./forge";

const affixe = (group: number, slot: "prefix" | "suffix"): ForgeAffix => ({
  group,
  family: 0,
  slot,
  libelle: "",
  part: 0.1,
  essence: false,
  paliers: [],
});

describe("Forge", () => {
  it("formate les coûts en Exalted", () => {
    expect(formatEx(null)).toBe("–");
    expect(formatEx(Number.POSITIVE_INFINITY)).toBe("–");
    expect(formatEx(1.234)).toBe("1,23 ex");
    expect(formatEx(12.34)).toBe("12,3 ex");
    expect(formatEx(24122.7).replace(/\s/g, " ")).toBe("24 123 ex");
  });

  it("formate probabilités et nombres", () => {
    expect(formatPercent(0.1234)).toBe("12,3 %");
    expect(formatPercent(0.0001)).toBe("< 0,1 %");
    expect(formatCount(2.345)).toBe("2,3");
    expect(formatCount(3075.12).replace(/\s/g, " ")).toBe("3 075");
  });

  it("regroupe les bases par classe", () => {
    const b = (nom: string, classe: string) => ({ id: nom, nom, classe, niveauDrop: 1, page: "" });
    const g = basesParClasse([b("Gold Ring", "Ring"), b("Suede Bracers", "Gloves"), b("Iron Ring", "Ring")]);
    expect(g.map(([c, l]) => [c, l.length])).toEqual([
      ["Gloves", 1],
      ["Ring", 2],
    ]);
  });

  it("limite l'objectif à 3 voulus par côté et 6 en tout", () => {
    const pool = [1, 2, 3, 4].map((g) => affixe(g, "prefix")).concat([5, 6, 7, 8].map((g) => affixe(g, "suffix")));
    const w = (group: number) => ({ group, family: 0, maxTier: 255 });
    expect(peutAjouter(pool, [w(1), w(2)], pool[2]!)).toBe(true);
    expect(peutAjouter(pool, [w(1), w(2), w(3)], pool[3]!)).toBe(false);
    expect(peutAjouter(pool, [w(1), w(2), w(3)], pool[4]!)).toBe(true);
    expect(peutAjouter(pool, [w(1), w(2), w(3), w(5), w(6), w(7)], pool[7]!)).toBe(false);
  });

  it("décrit un état du plan", () => {
    const vide = { rarity: "normal" as const, heldWanted: [], blockedWanted: [], fracturedWanted: null, badPrefixes: 0, badSuffixes: 0 };
    expect(decrireEtat(vide, ["Vie"])).toBe("Normal · vierge");
    expect(decrireEtat({ ...vide, rarity: "rare", heldWanted: [0], badSuffixes: 2 }, ["Vie"])).toBe(
      "Rare · ✓ Vie · 2 mauvais suffixes",
    );
  });

  it("raccourcit le texte d'un mod", () => {
    expect(libelleCourt("+(100-119) to maximum Life")).toBe("+# to maximum Life");
    expect(libelleCourt("(41-45)% to Fire Resistance")).toBe("#% to Fire Resistance");
  });

  it("ordonne les étapes : racine puis états les plus visités", () => {
    const node = (id: string, expectedVisits: number) => ({
      kind: "action" as const,
      id,
      stateKey: id,
      state: { rarity: "normal" as const, heldWanted: [], blockedWanted: [], fracturedWanted: null, badPrefixes: 0, badSuffixes: 0 },
      expectedVisits,
      costToGo: 1,
      action: { id: "exalt", label: "Exalted Orb", unitCost: 1, isAbandon: false },
      repeat: null,
      branches: [],
      mergedMinorProbability: 0,
      mergedMinorCount: 0,
    });
    const plan = {
      rootId: "n0",
      nodes: { n0: node("n0", 1), n1: node("n1", 0.5), n2: node("n2", 3), n3: node("n3", 0.001) },
    } as unknown as ForgePlan;
    expect(etapes(plan).map((n) => n.id)).toEqual(["n0", "n2", "n1"]);
    expect(etapes(plan, 2).map((n) => n.id)).toEqual(["n0", "n2"]);
  });

  it("lit les prix saisis", () => {
    expect(lirePrix("1,5")).toBe(1.5);
    expect(lirePrix(" 3 ")).toBe(3);
    expect(lirePrix("")).toBeNull();
    expect(lirePrix("-2")).toBeNull();
    expect(lirePrix("abc")).toBeNull();
  });

  it("date un relevé", () => {
    expect(dateReleve(null)).toBe("");
    expect(dateReleve(1_762_550_000)).toMatch(/^\d{2}\/\d{2} \d{2}:\d{2}$/);
  });
});

describe("arbre de décision", () => {
  const etat = { rarity: "normal" as const, heldWanted: [], blockedWanted: [], fracturedWanted: null, badPrefixes: 0, badSuffixes: 0 };
  const action = (id: string, branches: [string, number, boolean?][]) => ({
    kind: "action" as const,
    id,
    stateKey: id,
    state: etat,
    expectedVisits: 1,
    costToGo: 10,
    action: { id: "x", label: id, unitCost: 1, isAbandon: false },
    repeat: null,
    branches: branches.map(([to, probability, loopback = false], i) => ({
      id: `${id}-${i}`,
      kind: to === "goal" ? ("success" as const) : ("failure" as const),
      label: "",
      probability,
      to,
      loopback,
      extraCost: 0,
    })),
    mergedMinorProbability: 0,
    mergedMinorCount: 0,
  });
  const plan = {
    rootId: "r",
    nodes: {
      r: action("r", [["a", 0.5], ["b", 0.3], ["c", 0.15], ["d", 0.04], ["e", 0.01]]),
      a: action("a", [["goal", 0.6], ["ab", 0.4]]),
      ab: action("ab", [["r", 1, true]]),
      b: action("b", [["b", 0.5, true], ["goal", 0.5]]),
      c: action("c", [["a", 1]]),
      d: action("d", []),
      e: action("e", []),
      goal: { kind: "terminal", id: "goal", stateKey: "g", state: etat, expectedVisits: 1, costToGo: 0, result: "goal", note: null },
    },
  } as unknown as ForgePlan;

  it("place les états en colonnes et garde les branches les plus probables", () => {
    const t = arbre(plan, 4);
    const col = Object.fromEntries(t.noeuds.map((n) => [n.id, n.col]));
    expect(col).toEqual({ r: 0, a: 1, b: 1, c: 1, d: 1, goal: 2, ab: 2 });
    expect(t.autres.r).toEqual({ n: 1, p: 0.01 });
    // Boucle sur soi-même (essais répétés) : pas de lien dessiné.
    expect(t.liens.some((l) => l.de === "b" && l.vers === "b")).toBe(false);
    // Abandon puis base neuve, et c -> a (même colonne) : retours.
    expect(t.liens.filter((l) => l.retour).map((l) => `${l.de}>${l.vers}`)).toEqual(["c>a", "ab>r"]);
    expect(t.colonnes).toBe(3);
    expect(t.lignes).toBe(4);
  });

  it("plafonne le nombre d'états dessinés", () => {
    const t = arbre(plan, 4, 3);
    expect(t.noeuds.map((n) => n.id)).toEqual(["r", "a", "b"]);
    expect(t.autres.r).toEqual({ n: 3, p: 0.2 });
  });
});
