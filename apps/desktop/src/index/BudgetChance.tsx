/**
 * « Avec mon budget » : le joueur saisit ce qu'il peut dépenser et voit sa chance de finir le craft
 * en suivant le plan (lecture des crafts simulés). Commun aux Forges PoE 2 et PoE 1.
 */
import { useState } from "react";
import { formatChanceBudget, lirePrix } from "./forge";

const input =
  "rounded-chip border border-ink-700 bg-ink-950 px-2 py-1 text-xs text-text-primary placeholder:text-text-subtle";

function lire(cle: string): string {
  try {
    return localStorage.getItem(cle) ?? "";
  } catch {
    return "";
  }
}

function garder(cle: string, v: string) {
  try {
    localStorage.setItem(cle, v);
  } catch {
    // Stockage indisponible : la saisie vaut pour cette session seulement.
  }
}

export function BudgetChance({
  cle,
  devise,
  versBase,
  chance,
  note,
}: {
  /** Clé du stockage local où la saisie est gardée. */
  cle: string;
  /** Nom court de la monnaie de saisie (celle de l'affichage des coûts). */
  devise: string;
  /** Montant saisi ramené dans la monnaie des calculs. */
  versBase: (v: number) => number;
  /** Chance de finir avec ce budget (monnaie des calculs). */
  chance: (budget: number) => number;
  /** Ce que le budget couvre (base comprise ou non). */
  note: string;
}) {
  const [saisie, setSaisie] = useState(() => lire(cle));
  const montant = lirePrix(saisie);
  const p = montant !== null && montant > 0 ? chance(versBase(montant)) : null;
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-panel border border-ink-700 bg-ink-950 px-3 py-2 text-xs">
      <label className="flex items-center gap-2 text-text-subtle">
        <span className="text-sm font-semibold text-text-primary">Avec mon budget</span>
        <input
          value={saisie}
          inputMode="decimal"
          placeholder="ex. 200"
          aria-label="Budget"
          onChange={(e) => {
            setSaisie(e.target.value);
            garder(cle, e.target.value);
          }}
          className={`${input} w-24 text-right font-mono`}
        />
        <span>{devise}</span>
      </label>
      {p !== null ? (
        <span className="text-text-muted">
          <span className="font-semibold text-accent">{formatChanceBudget(p)}</span> de chance de finir le craft en suivant le
          plan
        </span>
      ) : saisie.trim() !== "" ? (
        <span className="text-signal-warn">Saisis un montant positif.</span>
      ) : (
        <span className="text-text-subtle">Saisis ce que tu peux dépenser pour voir ta chance de finir.</span>
      )}
      <span className="w-full text-[11px] text-text-subtle">{note}</span>
    </div>
  );
}

/** Avertissement : les prix du calcul datent de plus de 24 h. */
export function PrixAnciens({
  age,
  busy,
  onActualiser,
  apres,
}: {
  /** Âge lisible du relevé (« 30 h », « 3 jours »). */
  age: string;
  busy: boolean;
  onActualiser: () => void;
  /** Ce qu'il faut faire après l'actualisation. */
  apres: string;
}) {
  return (
    <div
      role="status"
      className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-panel border border-signal-warn/50 bg-ink-950 px-3 py-2 text-xs"
    >
      <span className="text-signal-warn">
        Prix relevés il y a {age} : le coût moyen peut avoir changé. {apres}
      </span>
      <button
        type="button"
        disabled={busy}
        onClick={onActualiser}
        className="rounded-chip border border-ink-600 px-3 py-1 text-xs font-semibold text-text-primary hover:bg-ink-800 disabled:opacity-50"
      >
        {busy ? "Actualisation…" : "Actualiser"}
      </button>
    </div>
  );
}
