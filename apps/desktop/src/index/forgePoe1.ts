/** Forge PoE 1 : coût des méthodes de spam (fonctions pures, testées). */
import type {
  ForgeActionNode,
  ForgeBranch,
  ForgeItemSummary,
  ForgePlan,
  ForgePoe1Base,
  ForgePoe1Conso,
  ForgePoe1Etape,
  ForgePoe1Method,
  ForgePoe1Plan,
  ForgeTerminalNode,
} from "../bridge";

/** Méthode chiffrée : coût d'un essai, coût moyen et nombre d'essais (médian, 9 fois sur 10). */
export type MethodeChiffree = ForgePoe1Method & {
  /** Coût d'un essai en Chaos ; `null` si une monnaie n'a pas de prix. */
  coutEssai: number | null;
  /** Coût du craft d'établi final (0 sans finition). */
  coutFinition: number | null;
  coutMoyen: number | null;
  essaisMoyens: number;
  essaisP50: number;
  essaisP90: number;
  /** Monnaies sans prix (ni poe.ninja ni saisie). */
  sansPrix: string[];
};

/** Nombre d'essais pour réussir avec la probabilité `q` (loi géométrique), au moins 1. */
export function essaisPour(chance: number, q: number): number {
  if (chance >= 1) return 1;
  if (chance <= 0) return Infinity;
  return Math.max(1, Math.ceil(Math.log(1 - q) / Math.log(1 - chance)));
}

/** Prix effectif : saisie du joueur, sinon poe.ninja. */
export function prixEffectif(
  nom: string,
  ninja: Record<string, number>,
  saisis: Record<string, number>,
): number | null {
  if (saisis[nom] != null) return saisis[nom];
  return ninja[nom] ?? null;
}

/** Prix envoyés au solveur : cote du marché, remplacée par les saisies du joueur. */
export function prixPourPlan(ninja: Record<string, number>, saisis: Record<string, number>): Record<string, number> {
  return { ...ninja, ...saisis };
}

/** Le solveur ne propose un plan que jusqu'à ce nombre de mods voulus. */
export const VOULUS_MAX_PLAN = 6;

/** Chiffre chaque méthode et les classe : coût moyen croissant, méthodes sans prix à la fin. */
export function chiffrer(
  methodes: ForgePoe1Method[],
  ninja: Record<string, number>,
  saisis: Record<string, number>,
): MethodeChiffree[] {
  const somme = (cs: ForgePoe1Conso[]) =>
    cs.reduce((t, c) => t + c.quantite * (prixEffectif(c.nom, ninja, saisis) ?? 0), 0);
  const chiffrees = methodes.map((m) => {
    const toutes = [...m.monnaies, ...(m.finition?.monnaies ?? [])];
    const sansPrix = [
      ...new Set(toutes.filter((c) => c.quantite > 0 && prixEffectif(c.nom, ninja, saisis) == null).map((c) => c.nom)),
    ];
    const coutEssai = sansPrix.length > 0 ? null : somme(m.monnaies);
    const coutFinition = sansPrix.length > 0 ? null : somme(m.finition?.monnaies ?? []);
    return {
      ...m,
      coutEssai,
      coutFinition,
      coutMoyen:
        coutEssai == null || coutFinition == null || m.chance <= 0 ? null : coutEssai / m.chance + coutFinition,
      essaisMoyens: m.chance > 0 ? 1 / m.chance : Infinity,
      essaisP50: essaisPour(m.chance, 0.5),
      essaisP90: essaisPour(m.chance, 0.9),
      sansPrix,
    };
  });
  return chiffrees.sort((a, b) => {
    if (a.coutMoyen == null && b.coutMoyen == null) return b.chance - a.chance;
    if (a.coutMoyen == null) return 1;
    if (b.coutMoyen == null) return -1;
    return a.coutMoyen - b.coutMoyen;
  });
}

const nombre = (v: number, dec: number) =>
  v.toLocaleString("fr-FR", {
    minimumFractionDigits: dec,
    maximumFractionDigits: dec,
  });

/** Coût en Chaos, avec l'équivalent en Divine au-delà d'une Divine quand son cours est connu. */
export function formatChaos(v: number | null, divine: number | null = null): string {
  if (v == null || !Number.isFinite(v)) return "–";
  const c = v >= 100 ? nombre(v, 0) : v >= 10 ? nombre(v, 1) : nombre(v, 2);
  if (divine && divine > 0 && v >= divine) return `${c} c (${nombre(v / divine, 1)} div)`;
  return `${c} c`;
}

/** Nombre d'essais lisible. */
export function formatEssais(n: number): string {
  if (!Number.isFinite(n)) return "–";
  if (n >= 1e6) return `${nombre(n / 1e6, 1)} M`;
  return n >= 10 ? nombre(n, 0) : nombre(n, 1);
}

/** Chance par essai : « 1 sur N » sous 1 %, pourcentage sinon. */
export function formatChance(p: number): string {
  if (p <= 0) return "0 %";
  if (p < 0.01) return `1 sur ${formatEssais(Math.round(1 / p))}`;
  return `${nombre(p * 100, 1)} %`;
}

/** Libellé d'une base : nom, implicite s'il départage les homonymes, niveau d'apparition. */
export function libelleBase(b: ForgePoe1Base, bases: ForgePoe1Base[]): string {
  const homonymes = bases.filter((x) => x.nom === b.nom && x.classe === b.classe).length > 1;
  const imp = homonymes && b.implicites[0] ? ` : ${b.implicites[0]}` : "";
  return `${b.nom}${imp} (niv. ${b.niveauDrop})`;
}

/** Situation de l'objet du joueur, telle qu'il la décrit dans le pas à pas. */
export type SituationPoe1 = {
  rarete: ForgePoe1Etape["rarete"];
  /** Par mod voulu : absent, présent, ou son groupe pris par un tier trop bas. */
  voulus: ("absent" | "present" | "bas")[];
  autresPrefixes: number;
  autresSuffixes: number;
  metamod: string | null;
};

const memes = (a: number[], b: number[]) => a.length === b.length && a.every((x) => b.includes(x));

/** Étape du plan qui correspond à la situation ; `null` si le plan n'y passe (presque) jamais. */
export function trouverEtape(etapes: ForgePoe1Etape[], s: SituationPoe1): ForgePoe1Etape | null {
  const indices = (v: SituationPoe1["voulus"][number]) => s.voulus.flatMap((x, i) => (x === v ? [i] : []));
  const voulus = indices("present");
  const bas = indices("bas");
  return (
    etapes.find(
      (e) =>
        e.rarete === s.rarete &&
        memes(e.voulus, voulus) &&
        memes(e.mauvaisTiers, bas) &&
        e.autresPrefixes === s.autresPrefixes &&
        e.autresSuffixes === s.autresSuffixes &&
        (e.metamod ?? null) === s.metamod,
    ) ?? null
  );
}

/** Règle du pas à pas : une action conseillée et les situations où elle s'applique. */
export type DecisionPoe1 = {
  rarete: ForgePoe1Etape["rarete"];
  action: string;
  /** Description des situations, de la plus fréquente à la moins fréquente. */
  situations: string[];
  /** Action habituelle de cette rareté (« dans les autres cas »). */
  parDefaut: boolean;
  passages: number;
};

const pluriel = (n: number, mot: string) => `${n} ${n > 1 ? mot.replace(/(\S+) (\S+)/, "$1s $2s") : mot}`;

/** Description courte d'une situation : mods voulus présents, tiers trop bas, autres mods, metamod. */
export function decrireEtape(e: ForgePoe1Etape, noms: string[]): string {
  const nom = (i: number) => noms[i] ?? `mod ${i + 1}`;
  const morceaux: string[] = [];
  morceaux.push(e.voulus.length > 0 ? e.voulus.map(nom).join(" + ") : "aucun mod voulu");
  if (e.mauvaisTiers.length > 0) morceaux.push(`tier trop bas : ${e.mauvaisTiers.map(nom).join(", ")}`);
  const autres: string[] = [];
  if (e.autresPrefixes > 0) autres.push(pluriel(e.autresPrefixes, "autre préfixe"));
  if (e.autresSuffixes > 0) autres.push(pluriel(e.autresSuffixes, "autre suffixe"));
  if (autres.length > 0) morceaux.push(autres.join(", "));
  if (e.metamod) morceaux.push(`metamod « ${e.metamod} »`);
  return morceaux.join(" · ");
}

/**
 * Règles du pas à pas, par rareté : l'action la plus courante (« dans les autres cas »), puis les
 * situations où le plan fait autre chose. L'objet Normal n'a qu'une action, celle du départ.
 */
export function decisions(etapes: ForgePoe1Etape[], noms: string[]): DecisionPoe1[] {
  const sortie: DecisionPoe1[] = [];
  for (const rarete of ["Normal", "Magique", "Rare"] as const) {
    const ici = etapes.filter((e) => e.rarete === rarete);
    if (ici.length === 0) continue;
    const parAction = new Map<string, ForgePoe1Etape[]>();
    for (const e of ici) parAction.set(e.action, [...(parAction.get(e.action) ?? []), e]);
    const total = (l: ForgePoe1Etape[]) => l.reduce((t, e) => t + e.passages, 0);
    const groupes = [...parAction.entries()].sort((a, b) => total(b[1]) - total(a[1]));
    const defaut = groupes[0]?.[0];
    for (const [action, liste] of groupes) {
      const parDefaut = action === defaut && rarete !== "Normal";
      sortie.push({
        rarete,
        action,
        situations: parDefaut
          ? []
          : [...liste].sort((a, b) => b.passages - a.passages).map((e) => decrireEtape(e, noms)),
        parDefaut,
        passages: total(liste),
      });
    }
  }
  return sortie;
}

const RARETE: Record<ForgePoe1Etape["rarete"], ForgeItemSummary["rarity"]> = {
  Normal: "normal",
  Magique: "magic",
  Rare: "rare",
};
const FINI = "fini";
const idEtape = (id: number) => `e${id}`;

/** Plan PoE 1 mis à la forme d'un plan PoE 2, pour l'arbre de décision, les étapes et les courses. */
export type ArbrePoe1 = {
  plan: ForgePlan;
  /** Description d'une case : rareté, mods voulus, autres mods, metamod. */
  decrire: (n: ForgeActionNode | ForgeTerminalNode) => string;
};

/**
 * Convertit le plan conseillé PoE 1 (montants en Chaos) en plan au format de la Forge PoE 2, pour
 * l'arbre, les étapes et la liste de courses aux prix utilisés.
 *
 * Les situations où le plan fait la même relance (même action, même loi de relance : Chaos, Essence ou
 * Alteration qui refont l'objet) sont regroupées en une case : leurs issues entre elles deviennent
 * la relance sur place (« ~N× »). Une issue est un progrès quand il reste moins à dépenser après.
 */
export function arbrePoe1(plan: ForgePoe1Plan, noms: string[], prix: Record<string, number>): ArbrePoe1 {
  // Groupes de situations à relance identique ; le représentant est la plus visitée.
  const signature = (e: ForgePoe1Etape) => (e.relance == null ? `#${e.id}` : `${e.action}|${e.relance}`);
  const issuesDe = (e: ForgePoe1Etape) => (e.relance == null ? e.issues : (plan.relances[e.relance] ?? e.issues));
  const groupes = new Map<string, ForgePoe1Etape[]>();
  for (const e of plan.etapes) groupes.set(signature(e), [...(groupes.get(signature(e)) ?? []), e]);
  const groupeDe = new Map<number, ForgePoe1Etape[]>();
  for (const g of groupes.values()) {
    g.sort((a, b) => b.passages - a.passages);
    for (const e of g) groupeDe.set(e.id, g);
  }
  const caseDe = (id: number | null) => {
    if (id == null) return FINI;
    const g = groupeDe.get(id);
    return g ? idEtape(g[0]!.id) : null;
  };
  const resume = (e: ForgePoe1Etape | null): ForgeItemSummary => ({
    rarity: e ? RARETE[e.rarete] : "rare",
    heldWanted: e ? e.voulus : noms.map((_, i) => i),
    blockedWanted: e?.mauvaisTiers ?? [],
    fracturedWanted: null,
    badPrefixes: e?.autresPrefixes ?? 0,
    badSuffixes: e?.autresSuffixes ?? 0,
  });
  const resteDe = new Map(plan.etapes.map((e) => [idEtape(e.id), e.reste]));
  const descriptions = new Map<string, string>();
  const nodes: ForgePlan["nodes"] = {};
  let fini = 0;
  for (const g of groupes.values()) {
    const e = g[0]!;
    const id = idEtape(e.id);
    const passages = g.reduce((t, x) => t + x.passages, 0);
    // Issues vers les cases (situations hors plan dessiné regroupées avec les issues rares).
    const versCase = new Map<string, number>();
    let surPlace = 0;
    const issues = issuesDe(e);
    let rares = issues.autresIssues;
    let nRares = issues.nombreAutres;
    for (const b of issues.branches) {
      const c = caseDe(b.vers);
      if (c === id) surPlace += b.probabilite;
      else if (c == null) {
        rares += b.probabilite;
        nRares += 1;
      } else versCase.set(c, (versCase.get(c) ?? 0) + b.probabilite);
    }
    fini += passages * (versCase.get(FINI) ?? 0);
    const reste = 1 - surPlace;
    const branches: ForgeBranch[] = [...versCase.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([to, p]) => {
        const resteCible = to === FINI ? 0 : resteDe.get(to);
        return {
          id: `${id}-${to}`,
          kind: resteCible != null && resteCible < e.reste ? "success" : "failure",
          label: "",
          probability: reste > 0 ? p / reste : 0,
          to,
          loopback: false,
          extraCost: 0,
        };
      });
    nodes[id] = {
      kind: "action",
      id,
      stateKey: id,
      state: resume(e),
      expectedVisits: passages,
      costToGo: e.reste,
      action: { id: e.action, label: e.action, unitCost: e.cout, isAbandon: false },
      repeat:
        surPlace > 0
          ? {
              selfLoopProbability: surPlace,
              expectedAttempts: reste > 0 ? 1 / reste : Infinity,
              p90Attempts: surPlace < 1 ? Math.max(1, Math.ceil(Math.log(0.1) / Math.log(surPlace))) : Infinity,
            }
          : null,
      branches,
      mergedMinorProbability: reste > 0 ? rares / reste : 0,
      mergedMinorCount: nRares,
    };
    descriptions.set(
      id,
      g.length > 1
        ? `${(["Normal", "Magique", "Rare"] as const).filter((r) => g.some((x) => x.rarete === r)).join(" ou ")} · ${g.length} situations, surtout ${decrireEtape(e, noms)}`
        : `${e.rarete} · ${decrireEtape(e, noms)}`,
    );
  }
  nodes[FINI] = {
    kind: "terminal",
    id: FINI,
    stateKey: FINI,
    state: resume(null),
    expectedVisits: fini > 0 ? fini : 1,
    costToGo: 0,
    result: "Objet fini",
    note: null,
  };
  const shopping = plan.monnaies.map((c) => {
    const unitCost = prix[c.nom] ?? 0;
    return { id: c.nom, label: c.nom, expectedCount: c.quantite, unitCost, expectedCost: unitCost * c.quantite };
  });
  shopping.sort((a, b) => b.expectedCost - a.expectedCost);
  return {
    plan: {
      baseId: "",
      ilvl: 0,
      goal: noms.map((label) => ({ label, slot: "prefix", group: 0, familyId: 0, maxTier: 0 })),
      rootId: caseDe(plan.racine) ?? idEtape(plan.racine),
      nodes,
      expectedCost: plan.coutMoyen,
      baseCost: 0,
      shopping,
      solver: { states: plan.etapes.length, sweeps: 0, converged: true, millis: 0, costFromVisits: plan.coutMoyen },
      mc: null,
      pricesSource: "",
    },
    decrire: (n) => (n.kind === "terminal" ? "Objet fini" : (descriptions.get(n.id) ?? "")),
  };
}
