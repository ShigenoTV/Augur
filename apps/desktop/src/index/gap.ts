import type { GapItem, GapReport } from "../bridge";

/** Libellés français des emplacements de Path of Building (« Helmet » → « Casque »). */
const SLOTS: Record<string, string> = {
  "Weapon 1": "Arme",
  "Weapon 2": "Main gauche",
  Helmet: "Casque",
  "Body Armour": "Armure",
  Gloves: "Gants",
  Boots: "Bottes",
  Amulet: "Amulette",
  "Ring 1": "Anneau gauche",
  "Ring 2": "Anneau droit",
  "Ring 3": "Troisième anneau",
  Belt: "Ceinture",
};

export function pobSlotLabel(slot: string): string {
  if (SLOTS[slot]) return SLOTS[slot];
  const flacon = /^Flask (\d+)$/.exec(slot);
  if (flacon) return `Flacon ${flacon[1]}`;
  const charme = /^Charm (\d+)$/.exec(slot);
  if (charme) return `Charme ${charme[1]}`;
  if (slot.startsWith("Jewel")) return "Joyau";
  return slot;
}

/** 12 345 ; 1,2 M au-delà du million ; « — » si inconnu. */
export function formatStat(v: number | null): string {
  if (v === null || !Number.isFinite(v)) return "—";
  if (Math.abs(v) >= 1_000_000) {
    return `${(v / 1_000_000).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} M`;
  }
  return Math.round(v).toLocaleString("fr-FR");
}

/** Écart en % de la cible par rapport à l'actuel (« +35 % »), ou null. */
export function percentGap(current: number | null, target: number | null): string | null {
  if (current === null || target === null || current <= 0) return null;
  const p = Math.round(((target - current) / current) * 100);
  if (p === 0) return "=";
  return `${p > 0 ? "+" : "−"}${Math.abs(p)} %`;
}

/** Ce qu'il reste à faire sur un emplacement, en une ligne. */
export function itemSummary(item: GapItem): string {
  switch (item.status) {
    case "reached":
      return "Atteint";
    case "otherItem":
      return `Unique à obtenir : ${item.target ?? "?"}`;
    case "empty":
      return item.targetUnique ? `Rien d'équipé, unique à obtenir : ${item.target ?? "?"}` : "Rien d'équipé";
    case "improve": {
      const parts: string[] = [];
      if (item.missingMods.length > 0) parts.push(`${item.missingMods.length} mod${item.missingMods.length > 1 ? "s" : ""} manquant${item.missingMods.length > 1 ? "s" : ""}`);
      if (item.weakerMods.length > 0) parts.push(`${item.weakerMods.length} plus faible${item.weakerMods.length > 1 ? "s" : ""}`);
      return parts.join(", ");
    }
  }
}

/** Emplacements atteints sur le total, pour le titre de la liste. */
export function progress(report: GapReport): { reached: number; total: number } {
  return { reached: report.items.filter((i) => i.status === "reached").length, total: report.items.length };
}

/** Les emplacements à travailler d'abord : vides, autre unique, puis le plus de mods manquants. */
export function sortItems(items: GapItem[]): GapItem[] {
  const rank = (i: GapItem) => (i.status === "empty" ? 0 : i.status === "otherItem" ? 1 : i.status === "improve" ? 2 : 3);
  return items
    .map((item, index) => ({ item, index }))
    .sort(
      (a, b) =>
        rank(a.item) - rank(b.item) ||
        b.item.missingMods.length - a.item.missingMods.length ||
        b.item.weakerMods.length - a.item.weakerMods.length ||
        a.index - b.index,
    )
    .map((x) => x.item);
}
