import type { CharacterInfo } from "../bridge";

/** « Arc niveau 21, qualité 20 % » : la compétence comparée par l'overlay. */
export function describeSkill(info: CharacterInfo): string {
  const s = info.skill;
  if (!s) return "Aucune compétence active dans le build";
  const quality = s.quality > 0 ? `, qualité ${s.quality} %` : "";
  return `Compétence comparée : ${s.name} niveau ${s.level}${quality}`;
}
