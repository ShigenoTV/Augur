/** Forge PoE 1 : coût des méthodes de spam (fonctions pures, testées). */
import type { ForgePoe1Base, ForgePoe1Method } from "../bridge";

/** Méthode chiffrée : coût d'un essai, coût moyen et nombre d'essais (médian, 9 fois sur 10). */
export type MethodeChiffree = ForgePoe1Method & {
  /** Coût d'un essai en Chaos ; `null` si une monnaie n'a pas de prix. */
  coutEssai: number | null;
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
export function prixEffectif(nom: string, ninja: Record<string, number>, saisis: Record<string, number>): number | null {
  if (saisis[nom] != null) return saisis[nom];
  return ninja[nom] ?? null;
}

/** Chiffre chaque méthode et les classe : coût moyen croissant, méthodes sans prix à la fin. */
export function chiffrer(
  methodes: ForgePoe1Method[],
  ninja: Record<string, number>,
  saisis: Record<string, number>,
): MethodeChiffree[] {
  const chiffrees = methodes.map((m) => {
    const sansPrix = m.monnaies
      .filter((c) => c.quantite > 0 && prixEffectif(c.nom, ninja, saisis) == null)
      .map((c) => c.nom);
    const coutEssai =
      sansPrix.length > 0
        ? null
        : m.monnaies.reduce((t, c) => t + c.quantite * (prixEffectif(c.nom, ninja, saisis) ?? 0), 0);
    return {
      ...m,
      coutEssai,
      coutMoyen: coutEssai == null || m.chance <= 0 ? null : coutEssai / m.chance,
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
  v.toLocaleString("fr-FR", { minimumFractionDigits: dec, maximumFractionDigits: dec });

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
