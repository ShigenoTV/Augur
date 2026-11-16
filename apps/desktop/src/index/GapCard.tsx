import { elementLabel } from "@augur/ui";
import { useCallback, useEffect, useState } from "react";
import { commands, isTauri, on, type BuildNumbers, type Game, type GapReport } from "../bridge";
import { formatStat, itemSummary, percentGap, pobSlotLabel, progress, sortItems } from "./gap";

/**
 * Écart entre le personnage actuel et le personnage cible : « où j'en suis,
 * où je vais ». Recalculé à chaque import ou suppression de l'un des deux.
 */
export function GapCard({ game }: { game: Game }) {
  const [report, setReport] = useState<GapReport | null>(null);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(() => {
    if (!isTauri()) {
      setLoaded(true);
      return;
    }
    void commands
      .getGap()
      .then(setReport)
      .catch(() => setReport(null))
      .finally(() => setLoaded(true));
  }, []);

  useEffect(() => {
    refresh();
    if (!isTauri()) return;
    const a = on.characterChanged(() => refresh());
    const b = on.targetChanged(() => refresh());
    return () => {
      void a.then((f) => f());
      void b.then((f) => f());
    };
  }, [refresh, game]);

  if (!loaded) return null;
  if (!report) {
    return (
      <p className="text-sm text-text-subtle">
        Importe ton personnage actuel et un personnage cible (le build que tu vises) : Augur te montrera ce qui les sépare,
        emplacement par emplacement.
      </p>
    );
  }

  const n = report.numbers;
  const p = progress(report);
  const items = sortItems(report.items);

  return (
    <div className="space-y-4 text-sm">
      {report.otherClass && (
        <p className="text-xs text-signal-warn">
          La cible est un autre personnage ({report.targetClass}, toi : {report.currentClass}) : l'arbre de passifs ne se compare pas
          vraiment.
        </p>
      )}

      <NumbersTable current={n.current} target={n.target} />
      <p className="text-xs text-text-subtle">
        {n.source === "pob"
          ? "Chiffres de Path of Building, tels qu'exportés avec chaque code. Un « — » : réexporte le code depuis Path of Building."
          : "Chiffres calculés par Augur, comme l'overlay (cible sans résistance, mana plein)."}
        {n.approximate && " Approximatif : une partie des builds n'est pas encore traduite."}
        {n.current.skill && n.target.skill && n.current.skill !== n.target.skill && (
          <> Compétences différentes ({n.current.skill} / {n.target.skill}) : les DPS ne se comparent pas vraiment.</>
        )}
      </p>

      <div className="space-y-2">
        <h3 className="font-semibold text-text-primary">
          Équipement : {p.reached} emplacement{p.reached > 1 ? "s" : ""} atteint{p.reached > 1 ? "s" : ""} sur {p.total}
        </h3>
        <ul className="space-y-1">
          {items.map((item) => {
            const reached = item.status === "reached";
            const details = item.missingMods.length + item.weakerMods.length > 0;
            const head = (
              <span className="flex flex-wrap items-baseline gap-x-2">
                <span className="w-32 shrink-0 font-semibold text-text-primary">{pobSlotLabel(item.slot)}</span>
                <span className={reached ? "text-signal-gain" : "text-text-muted"}>{itemSummary(item)}</span>
                {details && <span className="text-xs text-accent">détail</span>}
              </span>
            );
            return (
              <li key={item.slot} className="rounded-panel border border-ink-800 bg-ink-950 px-3 py-1.5">
                {details ? (
                  <details>
                    <summary className="cursor-pointer list-none [&::-webkit-details-marker]:hidden">{head}</summary>
                    <div className="mt-1 space-y-0.5 pl-34 text-xs">
                      <p className="text-text-subtle">
                        Actuel : {item.current ?? "rien"} · Cible : {item.target ?? "?"}
                      </p>
                      {item.missingMods.map((m) => (
                        <p key={`m-${m}`} className="text-signal-loss">
                          Manque : {m}
                        </p>
                      ))}
                      {item.weakerMods.map((m) => (
                        <p key={`w-${m.target}`} className="text-signal-warn">
                          Plus faible : {m.current} (cible {m.target})
                        </p>
                      ))}
                    </div>
                  </details>
                ) : (
                  head
                )}
              </li>
            );
          })}
        </ul>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-1">
          <h3 className="font-semibold text-text-primary">Gemmes</h3>
          {report.missingSkills.length === 0 && report.missingSupports.length === 0 ? (
            <p className="text-signal-gain">Toutes les gemmes de la cible sont là.</p>
          ) : (
            <ul className="space-y-0.5 text-text-muted">
              {report.missingSkills.map((s) => (
                <li key={`s-${s}`}>Compétence manquante : {s}</li>
              ))}
              {report.missingSupports.map((s) => (
                <li key={`p-${s.skill}-${s.support}`}>
                  {s.skill} : support {s.support} manquant
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="space-y-1">
          <h3 className="font-semibold text-text-primary">Passifs</h3>
          <p className="text-text-muted">
            {report.passives.toTake === 0 && report.passives.toRemove === 0
              ? "Arbre identique à la cible."
              : `${report.passives.toTake} à prendre${report.passives.toRemove > 0 ? `, ${report.passives.toRemove} à retirer` : ""}`}
          </p>
          <p className="text-xs text-text-subtle">
            {report.passives.current} passifs alloués, {report.passives.target} dans la cible
          </p>
        </div>
      </div>
    </div>
  );
}

function NumbersTable({ current, target }: { current: BuildNumbers; target: BuildNumbers }) {
  const rows: { label: string; a: number | null; b: number | null; pct?: boolean }[] = [
    { label: "DPS", a: current.dps, b: target.dps },
    { label: "EHP", a: current.ehp, b: target.ehp },
    { label: "Vie", a: current.life, b: target.life },
    { label: "Bouclier d'énergie", a: current.energyShield, b: target.energyShield },
  ];
  const elements = ["fire", "cold", "lightning", "chaos"] as const;
  for (const e of elements) {
    const a = current.resistances.find((r) => r.element === e);
    const b = target.resistances.find((r) => r.element === e);
    if (!a && !b) continue;
    rows.push({ label: elementLabel(e), a: a?.value ?? null, b: b?.value ?? null, pct: true });
  }
  const shown = rows.filter((r) => r.a !== null || r.b !== null);
  const fmt = (v: number | null, pct?: boolean) => (pct && v !== null ? `${Math.round(v)} %` : formatStat(v));
  return (
    <table className="w-full text-left">
      <thead className="text-xs text-text-subtle">
        <tr>
          <th className="font-normal" />
          <th className="font-normal">Actuel</th>
          <th className="font-normal">Cible</th>
          <th className="font-normal">Écart</th>
        </tr>
      </thead>
      <tbody>
        {shown.map((r) => {
          const gap = r.pct
            ? r.a !== null && r.b !== null
              ? r.b - r.a === 0
                ? "="
                : `${r.b > r.a ? "+" : "−"}${Math.abs(Math.round(r.b - r.a))}`
              : null
            : percentGap(r.a, r.b);
          const behind = r.a !== null && r.b !== null && r.b > r.a;
          return (
            <tr key={r.label} className="border-t border-ink-800">
              <td className="py-1 text-text-muted">{r.label}</td>
              <td className="py-1 font-mono">{fmt(r.a, r.pct)}</td>
              <td className="py-1 font-mono">{fmt(r.b, r.pct)}</td>
              <td className={`py-1 font-mono ${behind ? "text-signal-warn" : "text-text-subtle"}`}>{gap ?? ""}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
