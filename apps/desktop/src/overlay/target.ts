import type { TargetProgress } from "../bridge";
import { pobSlotLabel } from "../index/gap";

/** Ligne « vers la cible » de l'overlay, et si l'objet copié rapproche de la cible. */
export function targetLine(t: TargetProgress): { text: string; better: boolean | null } {
  const slot = pobSlotLabel(t.slot).toLowerCase();
  if (t.targetUnique) {
    if (t.status === "reached") return { text: `C'est l'unique de ta cible (${slot}) : ${t.target ?? "?"}`, better: true };
    return { text: `Ta cible porte ${t.target ?? "un unique"} (${slot})`, better: null };
  }
  const base = `Vers la cible (${slot}) : ${t.reached} mod${t.reached > 1 ? "s" : ""} sur ${t.total}`;
  if (t.currentReached === null) return { text: base, better: null };
  const better = t.reached === t.currentReached ? null : t.reached > t.currentReached;
  return { text: `${base} · ton objet : ${t.currentReached} sur ${t.total}`, better };
}

/** Mods de la cible qui manquent encore à l'objet copié, en lignes courtes. */
export function targetMissing(t: TargetProgress, max = 3): string[] {
  const lines = [...t.missingMods.map((m) => `manque ${m}`), ...t.weakerMods.map((m) => `faible : ${m.current} (cible ${m.target})`)];
  return lines.length > max ? [...lines.slice(0, max), `… et ${lines.length - max} autre${lines.length - max > 1 ? "s" : ""}`] : lines;
}
