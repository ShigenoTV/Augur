import { useState } from "react";
import type { LedgerLoot } from "../bridge";
import { useCout } from "./devise";
import { LOOT_ITEMS, lootNet, parseAmount } from "./ledger";

const input =
  "rounded-chip border border-ink-700 bg-ink-950 px-2 py-1 text-xs text-text-primary placeholder:text-text-subtle";
const step = "rounded-chip border border-ink-700 px-1.5 text-xs text-text-subtle hover:bg-ink-800 hover:text-text-primary";

const montant = (v: number) => (v > 0 ? String(v).replace(".", ",") : "");

/**
 * Compteurs rapides du butin d'une map : monnaies ramassées, valeur du reste et dépense (Waystone,
 * tablettes), en Exalted. La valeur nette s'affiche aux prix donnés pendant la saisie.
 */
export function LootEditor({
  value,
  prix,
  onSave,
  onClose,
}: {
  value: LedgerLoot;
  prix: Record<string, number>;
  onSave: (loot: LedgerLoot) => Promise<boolean>;
  onClose: () => void;
}) {
  const cout = useCout();
  const [quantites, setQuantites] = useState<Record<string, number>>(value.quantites);
  const [autre, setAutre] = useState(montant(value.autre));
  const [depense, setDepense] = useState(montant(value.depense));

  const loot: LedgerLoot = {
    quantites: Object.fromEntries(Object.entries(quantites).filter(([, q]) => q > 0)),
    autre: parseAmount(autre),
    depense: parseAmount(depense),
  };
  const set = (id: string, q: number) => setQuantites((s) => ({ ...s, [id]: Math.max(0, Math.min(100_000, q)) }));

  async function save(l: LedgerLoot) {
    if (await onSave(l)) onClose();
  }

  return (
    <div className="space-y-2 rounded-chip border border-ink-700 bg-ink-900 p-3">
      <div className="grid grid-cols-4 gap-x-3 gap-y-1.5">
        {LOOT_ITEMS.map((item) => {
          const q = quantites[item.id] ?? 0;
          return (
            <div key={item.id} className="flex items-center justify-between gap-1">
              <span className={`truncate text-xs ${q > 0 ? "text-text-primary" : "text-text-subtle"}`}>{item.nom}</span>
              <span className="flex items-center gap-1">
                <button type="button" aria-label={`Retirer un ${item.nom}`} onClick={() => set(item.id, q - 1)} className={step}>
                  −
                </button>
                <input
                  value={q || ""}
                  onChange={(e) => set(item.id, Math.floor(parseAmount(e.target.value)))}
                  inputMode="numeric"
                  placeholder="0"
                  aria-label={item.nom}
                  className={`${input} w-10 text-center font-mono`}
                />
                <button type="button" aria-label={`Ajouter un ${item.nom}`} onClick={() => set(item.id, q + 1)} className={step}>
                  +
                </button>
              </span>
            </div>
          );
        })}
      </div>
      <div className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col gap-1 text-xs text-text-subtle">
          Autre butin (ex)
          <input value={autre} onChange={(e) => setAutre(e.target.value)} inputMode="decimal" placeholder="0" className={`${input} w-28`} />
        </label>
        <label className="flex flex-col gap-1 text-xs text-text-subtle">
          Dépense : Waystone, tablettes (ex)
          <input
            value={depense}
            onChange={(e) => setDepense(e.target.value)}
            inputMode="decimal"
            placeholder="0"
            className={`${input} w-28`}
          />
        </label>
        <p className="pb-1 text-xs text-text-subtle">
          Net : <span className="font-mono text-text-primary">{cout(lootNet(loot, prix))}</span>
        </p>
        <span className="ml-auto flex items-center gap-2">
          <button
            type="button"
            onClick={() => void save(loot)}
            className="rounded-chip bg-accent px-3 py-1 text-xs font-semibold text-accent-ink hover:bg-accent-strong"
          >
            Enregistrer le butin
          </button>
          <button
            type="button"
            onClick={() => void save({ quantites: {}, autre: 0, depense: 0 })}
            className="text-xs text-text-subtle hover:text-text-primary"
          >
            Effacer
          </button>
          <button type="button" onClick={onClose} className="text-xs text-text-subtle hover:text-text-primary">
            Fermer
          </button>
        </span>
      </div>
    </div>
  );
}
