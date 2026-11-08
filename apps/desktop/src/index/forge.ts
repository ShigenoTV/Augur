/** Forge : mise en forme des données du cœur Rust pour le panneau (fonctions pures, testées). */
import type {
  ForgeActionNode,
  ForgeAffix,
  ForgeBase,
  ForgeItemSummary,
  ForgePlan,
  ForgeSlot,
  ForgeTerminalNode,
  ForgeWanted,
} from "../bridge";
import { formatCout } from "./devise";

/** Places d'un Rare ordinaire (le plafond propre à la base n'est pas encore extrait). */
export const MAX_PAR_COTE = 3;
export const MAX_VOULUS = 6;

const nombre = (v: number, dec: number) =>
  v.toLocaleString("fr-FR", { minimumFractionDigits: dec, maximumFractionDigits: dec });

/** Prix ou coût en Exalted Orbs, arrondi selon l'ordre de grandeur. */
export const formatEx = (v: number | null | undefined): string => formatCout(v);

export function formatPercent(p: number): string {
  if (!Number.isFinite(p)) return "–";
  if (p > 0 && p < 0.001) return "< 0,1 %";
  return `${nombre(p * 100, 1)} %`;
}

export function formatCount(v: number): string {
  return v >= 10 ? nombre(v, 0) : nombre(v, 1);
}

export const SLOT_LABEL: Record<ForgeSlot, string> = { prefix: "Préfixes", suffix: "Suffixes" };

/** Bases regroupées par classe, classes dans l'ordre alphabétique. */
export function basesParClasse(bases: ForgeBase[]): [string, ForgeBase[]][] {
  const m = new Map<string, ForgeBase[]>();
  for (const b of bases) m.set(b.classe, [...(m.get(b.classe) ?? []), b]);
  return [...m.entries()].sort(([a], [b]) => a.localeCompare(b));
}

export const cleAffixe = (a: { group: number; family: number }) => `${a.group}:${a.family}`;

/**
 * Peut-on encore viser cet affixe ? Faux si l'objectif est plein (6 voulus) ou si ce côté
 * (préfixe / suffixe) a déjà 3 voulus.
 */
export function peutAjouter(affixes: ForgeAffix[], voulus: ForgeWanted[], a: ForgeAffix): boolean {
  if (voulus.length >= MAX_VOULUS) return false;
  const memeCote = voulus.filter((w) => affixes.find((x) => cleAffixe(x) === cleAffixe(w))?.slot === a.slot).length;
  return memeCote < MAX_PAR_COTE;
}

/** Libellé d'un état abstrait : rareté, voulus présents, mauvais mods. */
export function decrireEtat(s: ForgeItemSummary, labels: string[]): string {
  const rarete = { normal: "Normal", magic: "Magique", rare: "Rare" }[s.rarity];
  const parts = [rarete];
  const nom = (k: number) => labels[k] ?? `#${k + 1}`;
  if (s.heldWanted.length === 0 && s.badPrefixes + s.badSuffixes === 0 && s.blockedWanted.length === 0) {
    parts.push("vierge");
  }
  for (const k of s.heldWanted) parts.push(`✓ ${nom(k)}`);
  for (const k of s.blockedWanted) parts.push(`tier trop bas : ${nom(k)}`);
  const bad = (n: number, mot: string) => (n === 0 ? null : `${n} mauvais ${mot}${n > 1 ? "s" : ""}`);
  for (const b of [bad(s.badPrefixes, "préfixe"), bad(s.badSuffixes, "suffixe")]) if (b) parts.push(b);
  return parts.join(" · ");
}

/** Libellé court d'un affixe voulu (le texte du mod, sans les fourchettes). */
export function libelleCourt(texte: string): string {
  return texte.replace(/\(([-\d.]+)-([-\d.]+)\)/g, "#").replace(/\s+/g, " ").trim();
}

/**
 * Étapes du plan, des états les plus visités aux moins visités (la racine d'abord), limitées à
 * `max`. Les états au but et ceux qu'on ne visite presque jamais sont laissés de côté.
 */
export function etapes(plan: ForgePlan, max = 12): ForgeActionNode[] {
  const actions = Object.values(plan.nodes).filter((n): n is ForgeActionNode => n.kind === "action");
  const racine = actions.find((n) => n.id === plan.rootId);
  const autres = actions
    .filter((n) => n.id !== plan.rootId && n.expectedVisits >= 0.01)
    .sort((a, b) => b.expectedVisits - a.expectedVisits);
  return [...(racine ? [racine] : []), ...autres].slice(0, max);
}

/** Lit un prix saisi (« 1,5 » ou « 1.5 ») ; `null` si ce n'est pas un nombre positif ou nul. */
export function lirePrix(v: string): number | null {
  const t = v.trim().replace(",", ".");
  if (t === "") return null;
  const n = Number(t);
  return Number.isFinite(n) && n >= 0 ? n : null;
}

/** Date d'un relevé (secondes unix) au format court. */
export function dateReleve(secs: number | null | undefined): string {
  if (!secs) return "";
  const d = new Date(secs * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getDate())}/${p(d.getMonth() + 1)} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export type NoeudArbre = { id: string; col: number; row: number; node: ForgeActionNode | ForgeTerminalNode };
export type LienArbre = { de: string; vers: string; p: number; kind: "success" | "failure"; retour: boolean };
export type Arbre = {
  noeuds: NoeudArbre[];
  liens: LienArbre[];
  /** Branches non dessinées par nœud (trop rares ou au-delà du plafond) : nombre et probabilité cumulée. */
  autres: Record<string, { n: number; p: number }>;
  colonnes: number;
  lignes: number;
};

/**
 * Arbre de décision du plan, en colonnes (une par étape depuis la base neuve). Chaque nœud garde
 * ses `maxBranches` branches les plus probables ; au-delà de `maxNoeuds`, les nouveaux états ne
 * sont plus dessinés (comptés dans `autres`). Les retours vers un état déjà placé (abandon puis
 * base neuve, Annulment…) sont marqués `retour`.
 */
export function arbre(plan: ForgePlan, maxBranches = 4, maxNoeuds = 28): Arbre {
  const place = new Map<string, NoeudArbre>();
  const parCol: number[] = [];
  const liens: LienArbre[] = [];
  const autres: Arbre["autres"] = {};
  const poser = (id: string, col: number) => {
    const node = plan.nodes[id];
    if (!node) return false;
    const row = parCol[col] ?? 0;
    parCol[col] = row + 1;
    place.set(id, { id, col, row, node });
    return true;
  };
  if (!poser(plan.rootId, 0)) return { noeuds: [], liens, autres, colonnes: 0, lignes: 0 };
  const file = [plan.rootId];
  while (file.length > 0) {
    const id = file.shift()!;
    const src = place.get(id)!;
    if (src.node.kind !== "action") continue;
    const branches = src.node.branches
      .filter((b) => !(b.loopback && b.to === id))
      .sort((a, b) => b.probability - a.probability);
    let n = 0;
    let p = src.node.mergedMinorProbability;
    let nAutres = src.node.mergedMinorCount;
    for (const b of branches) {
      const deja = place.get(b.to);
      if (n >= maxBranches || (!deja && place.size >= maxNoeuds) || !plan.nodes[b.to]) {
        nAutres += 1;
        p += b.probability;
        continue;
      }
      n += 1;
      if (!deja) {
        poser(b.to, src.col + 1);
        file.push(b.to);
      }
      const cible = place.get(b.to)!;
      liens.push({ de: id, vers: b.to, p: b.probability, kind: b.kind, retour: cible.col <= src.col });
    }
    if (nAutres > 0) autres[id] = { n: nAutres, p };
  }
  return {
    noeuds: [...place.values()],
    liens,
    autres,
    colonnes: parCol.length,
    lignes: Math.max(...parCol),
  };
}
