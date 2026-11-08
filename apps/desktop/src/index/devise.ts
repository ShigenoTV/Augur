/**
 * Monnaie d'affichage des coûts (Exalted, Chaos ou Divine). Les calculs restent en Exalted : seul
 * l'affichage est converti, au taux de la table de prix (correction du joueur, sinon poe.ninja,
 * sinon valeur indicative).
 *
 * Le choix et les taux sont gardés dans le stockage local de la WebView, partagé par la fenêtre
 * principale et l'overlay (même origine) : l'overlay affiche donc les coûts dans la même monnaie.
 */
import { createContext, useContext, useEffect, useState } from "react";

export type Devise = "exalt" | "chaos" | "divine";

export const DEVISES: { id: Devise; nom: string; court: string }[] = [
  { id: "exalt", nom: "Exalted", court: "ex" },
  { id: "chaos", nom: "Chaos", court: "c" },
  { id: "divine", nom: "Divine", court: "div" },
];

/** Prix d'une unité de chaque monnaie, en Exalted. */
export type Taux = Record<Devise, number>;

export type Affichage = { devise: Devise; taux: Taux };

export const AFFICHAGE_EX: Affichage = { devise: "exalt", taux: { exalt: 1, chaos: 0, divine: 0 } };

const CLE = "augur.devise";

const nombre = (v: number, dec: number) =>
  v.toLocaleString("fr-FR", { minimumFractionDigits: dec, maximumFractionDigits: dec });

/**
 * Coût (en Exalted) affiché dans la monnaie choisie, arrondi selon l'ordre de grandeur. Sans taux
 * connu pour cette monnaie, on reste en Exalted.
 */
export function formatCout(v: number | null | undefined, a: Affichage = AFFICHAGE_EX): string {
  if (v == null || !Number.isFinite(v)) return "–";
  const taux = a.taux[a.devise];
  const devise = taux > 0 ? a.devise : "exalt";
  const x = devise === "exalt" ? v : v / taux;
  const abs = Math.abs(x);
  const dec = abs >= 100 ? 0 : abs >= 10 ? 1 : 2;
  const court = DEVISES.find((d) => d.id === devise)!.court;
  return `${nombre(x, dec)} ${court}`;
}

/** Taux tirés des prix effectifs (identifiant -> prix en Exalted). */
export function tauxDepuisPrix(prix: Record<string, number>): Taux {
  const ok = (p: number | undefined) => (p !== undefined && Number.isFinite(p) && p > 0 ? p : 0);
  return { exalt: 1, chaos: ok(prix.chaos), divine: ok(prix.divine) };
}

/** Relit un affichage enregistré ; tout ce qui est abîmé retombe sur l'Exalted. */
export function lireAffichage(brut: string | null): Affichage {
  try {
    const v = JSON.parse(brut ?? "null") as Partial<Affichage> | null;
    const devise = DEVISES.some((d) => d.id === v?.devise) ? (v!.devise as Devise) : "exalt";
    const t = (v?.taux ?? {}) as Partial<Taux>;
    const n = (x: unknown) => (typeof x === "number" && Number.isFinite(x) && x > 0 ? x : 0);
    return { devise, taux: { exalt: 1, chaos: n(t.chaos), divine: n(t.divine) } };
  } catch {
    return AFFICHAGE_EX;
  }
}

function charger(): Affichage {
  try {
    return lireAffichage(localStorage.getItem(CLE));
  } catch {
    return AFFICHAGE_EX;
  }
}

function enregistrer(a: Affichage) {
  try {
    localStorage.setItem(CLE, JSON.stringify(a));
  } catch {
    // Stockage indisponible : le choix vaut pour cette session seulement.
  }
}

/** État de l'affichage, synchronisé entre fenêtres par l'événement `storage`. */
export function useAffichage() {
  const [affichage, setAffichage] = useState<Affichage>(charger);
  useEffect(() => {
    const sync = (e: StorageEvent) => {
      if (e.key === CLE) setAffichage(lireAffichage(e.newValue));
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);
  const changer = (f: (a: Affichage) => Affichage) =>
    setAffichage((a) => {
      const b = f(a);
      enregistrer(b);
      return b;
    });
  return {
    affichage,
    choisir: (devise: Devise) => changer((a) => ({ ...a, devise })),
    majTaux: (prix: Record<string, number>) => changer((a) => ({ ...a, taux: tauxDepuisPrix(prix) })),
  };
}

export type AffichageCtx = ReturnType<typeof useAffichage>;

export const AffichageContext = createContext<AffichageCtx>({
  affichage: AFFICHAGE_EX,
  choisir: () => undefined,
  majTaux: () => undefined,
});

/** Formateur de coûts dans la monnaie choisie. */
export function useCout(): (v: number | null | undefined) => string {
  const { affichage } = useContext(AffichageContext);
  return (v) => formatCout(v, affichage);
}
