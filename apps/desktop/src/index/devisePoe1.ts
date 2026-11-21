/**
 * Monnaie d'affichage des coûts en PoE 1 (Chaos ou Divine). Les calculs restent en Chaos : seul
 * l'affichage est converti, au cours de la Divine Orb en Chaos (prix saisi, sinon poe.ninja).
 *
 * Le choix et le cours sont gardés dans le stockage local, à part de celui de PoE 2.
 */
import { createContext, useContext, useEffect, useState } from "react";

export type DevisePoe1 = "chaos" | "divine";

export const DEVISES_POE1: { id: DevisePoe1; nom: string }[] = [
  { id: "chaos", nom: "Chaos" },
  { id: "divine", nom: "Divine" },
];

/** Monnaie choisie et cours d'une Divine Orb en Chaos (0 : inconnu). */
export type AffichagePoe1 = { devise: DevisePoe1; divine: number };

export const AFFICHAGE_CHAOS: AffichagePoe1 = { devise: "chaos", divine: 0 };

const CLE = "augur.devisePoe1";

const nombre = (v: number, dec: number) =>
  v.toLocaleString("fr-FR", { minimumFractionDigits: dec, maximumFractionDigits: dec });

/**
 * Coût en Chaos affiché dans la monnaie choisie, arrondi selon l'ordre de grandeur. Sans cours de
 * la Divine connu, on reste en Chaos.
 */
export function formatPoe1(v: number | null | undefined, a: AffichagePoe1 = AFFICHAGE_CHAOS): string {
  if (v == null || !Number.isFinite(v)) return "–";
  const enDivine = a.devise === "divine" && a.divine > 0;
  const x = enDivine ? v / a.divine : v;
  const abs = Math.abs(x);
  if (enDivine && abs > 0 && abs < 0.005) return "< 0,01 div";
  const dec = abs >= 100 ? 0 : abs >= 10 ? 1 : 2;
  return `${nombre(x, dec)} ${enDivine ? "div" : "c"}`;
}

/** Relit un affichage enregistré ; tout ce qui est abîmé retombe sur le Chaos. */
export function lireAffichagePoe1(brut: string | null): AffichagePoe1 {
  try {
    const v = JSON.parse(brut ?? "null") as Partial<AffichagePoe1> | null;
    const devise = DEVISES_POE1.some((d) => d.id === v?.devise) ? (v!.devise as DevisePoe1) : "chaos";
    const d = v?.divine;
    return { devise, divine: typeof d === "number" && Number.isFinite(d) && d > 0 ? d : 0 };
  } catch {
    return AFFICHAGE_CHAOS;
  }
}

function charger(): AffichagePoe1 {
  try {
    return lireAffichagePoe1(localStorage.getItem(CLE));
  } catch {
    return AFFICHAGE_CHAOS;
  }
}

function enregistrer(a: AffichagePoe1) {
  try {
    localStorage.setItem(CLE, JSON.stringify(a));
  } catch {
    // Stockage indisponible : le choix vaut pour cette session seulement.
  }
}

/** État de l'affichage PoE 1, synchronisé entre fenêtres par l'événement `storage`. */
export function useAffichagePoe1() {
  const [affichage, setAffichage] = useState<AffichagePoe1>(charger);
  useEffect(() => {
    const sync = (e: StorageEvent) => {
      if (e.key === CLE) setAffichage(lireAffichagePoe1(e.newValue));
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);
  const changer = (f: (a: AffichagePoe1) => AffichagePoe1) =>
    setAffichage((a) => {
      const b = f(a);
      enregistrer(b);
      return b;
    });
  return {
    affichage,
    choisir: (devise: DevisePoe1) => changer((a) => ({ ...a, devise })),
    /** Cours de la Divine Orb en Chaos ; ignoré s'il est inconnu ou inchangé. */
    majDivine: (divine: number | null) => {
      if (divine == null || !Number.isFinite(divine) || divine <= 0) return;
      changer((a) => (a.divine === divine ? a : { ...a, divine }));
    },
  };
}

export type AffichagePoe1Ctx = ReturnType<typeof useAffichagePoe1>;

export const AffichagePoe1Context = createContext<AffichagePoe1Ctx>({
  affichage: AFFICHAGE_CHAOS,
  choisir: () => undefined,
  majDivine: () => undefined,
});

/** Formateur de coûts PoE 1 (montants en Chaos) dans la monnaie choisie. */
export function useCoutPoe1(): (v: number | null | undefined) => string {
  const { affichage } = useContext(AffichagePoe1Context);
  return (v) => formatPoe1(v, affichage);
}

/** Montant saisi dans la monnaie d'affichage, ramené en Chaos (sans cours de la Divine : Chaos). */
export function versChaos(v: number, a: AffichagePoe1): number {
  return a.devise === "divine" && a.divine > 0 ? v * a.divine : v;
}

/** Nom court de la monnaie dans laquelle un montant PoE 1 se saisit. */
export function deviseSaisiePoe1(a: AffichagePoe1): string {
  return a.devise === "divine" && a.divine > 0 ? "div" : "c";
}
