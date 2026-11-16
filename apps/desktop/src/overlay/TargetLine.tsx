import type { TargetProgress } from "../bridge";
import { targetLine, targetMissing } from "./target";

/** Sous l'objet copié : où il en est par rapport à l'objet de la cible, et la recherche de cet objet. */
export function TargetLine({ target, onSearch }: { target: TargetProgress; onSearch: () => void }) {
  const line = targetLine(target);
  const missing = target.status === "reached" ? [] : targetMissing(target);
  const color = line.better === true ? "text-signal-gain" : line.better === false ? "text-signal-loss" : "text-text-primary";
  return (
    <div className="mt-1 max-w-[360px] rounded-panel border border-ink-700 bg-ink-900 px-3 py-2 text-[11px]">
      <p className={`font-semibold ${color}`}>{line.text}</p>
      {missing.map((m) => (
        <p key={m} className="text-text-subtle">
          {m}
        </p>
      ))}
      {target.status !== "reached" && (
        <button type="button" onClick={onSearch} className="mt-1 text-accent hover:underline">
          Chercher l'objet de la cible
        </button>
      )}
    </div>
  );
}
