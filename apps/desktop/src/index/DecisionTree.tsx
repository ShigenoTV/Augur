import { useMemo, useState } from "react";
import type { ForgePlan } from "../bridge";
import { useCout } from "./devise";
import { arbre, decrireEtat, formatCount, formatPercent, libelleCourt, type NoeudArbre } from "./forge";

const L = 210;
const H = 76;
const PAS_X = 290;
const PAS_Y = 96;
const MARGE = 16;

const x = (n: NoeudArbre) => MARGE + n.col * PAS_X;
const y = (n: NoeudArbre) => MARGE + n.row * PAS_Y;

/**
 * Arbre de décision du plan (repris de l'ancienne appli de craft de Max) : une colonne par étape
 * depuis la base neuve, flèches vertes quand l'objet progresse, rouges pointillées quand il recule.
 */
export function DecisionTree({ plan }: { plan: ForgePlan }) {
  const cout = useCout();
  const [retours, setRetours] = useState(false);
  const [zoom, setZoom] = useState(1);
  const t = useMemo(() => arbre(plan), [plan]);
  const labels = plan.goal.map((g) => libelleCourt(g.label));
  const pos = new Map(t.noeuds.map((n) => [n.id, n]));
  const largeur = MARGE * 2 + (t.colonnes - 1) * PAS_X + L;
  const hauteur = MARGE * 2 + (t.lignes - 1) * PAS_Y + H + (retours ? 40 : 0);

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-4 text-xs text-text-subtle">
        <span className="flex items-center gap-1.5">
          <svg width="22" height="6" aria-hidden>
            <line x1="0" y1="3" x2="22" y2="3" stroke="var(--color-signal-gain)" strokeWidth="2" />
          </svg>
          progrès
        </span>
        <span className="flex items-center gap-1.5">
          <svg width="22" height="6" aria-hidden>
            <line x1="0" y1="3" x2="22" y2="3" stroke="var(--color-signal-loss)" strokeWidth="2" strokeDasharray="4 3" />
          </svg>
          recul
        </span>
        <label className="flex items-center gap-1.5">
          <input
            type="checkbox"
            checked={retours}
            onChange={(e) => setRetours(e.target.checked)}
            className="accent-[var(--color-accent)]"
          />
          retours en arrière
        </label>
        <span className="ml-auto flex items-center gap-1">
          {[
            ["−", () => setZoom((z) => Math.max(0.5, z - 0.1)), "Dézoomer"],
            ["100 %", () => setZoom(1), "Taille réelle"],
            ["+", () => setZoom((z) => Math.min(1.5, z + 0.1)), "Zoomer"],
          ].map(([txt, f, aide]) => (
            <button
              key={txt as string}
              type="button"
              onClick={f as () => void}
              aria-label={aide as string}
              className="rounded-chip border border-ink-600 px-2 py-0.5 text-text-primary hover:bg-ink-800"
            >
              {txt as string}
            </button>
          ))}
        </span>
      </div>

      <div className="max-h-[560px] overflow-auto rounded-panel border border-ink-700 bg-ink-950">
        <div style={{ width: largeur * zoom, height: hauteur * zoom }}>
          <div className="relative origin-top-left" style={{ width: largeur, height: hauteur, transform: `scale(${zoom})` }}>
            <svg width={largeur} height={hauteur} className="absolute inset-0" aria-hidden>
              <defs>
                {(["gain", "loss"] as const).map((c) => (
                  <marker key={c} id={`fleche-${c}`} viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
                    <path d="M0,0 L8,4 L0,8 z" fill={`var(--color-signal-${c})`} />
                  </marker>
                ))}
              </defs>
              {t.liens
                .filter((l) => retours || !l.retour)
                .map((l) => {
                  const a = pos.get(l.de)!;
                  const b = pos.get(l.vers)!;
                  const c = l.kind === "success" ? "gain" : "loss";
                  let d: string;
                  let lx: number;
                  let ly: number;
                  if (l.retour) {
                    // Arc sous l'arbre, de la sortie du nœud vers l'entrée de l'état déjà placé.
                    const x1 = x(a) + L / 2;
                    const x2 = x(b) + L / 2;
                    const bas = MARGE + (t.lignes - 1) * PAS_Y + H + 30;
                    d = `M${x1},${y(a) + H} C${x1},${bas} ${x2},${bas} ${x2},${y(b) + H}`;
                    lx = (x1 + x2) / 2;
                    ly = bas - 8;
                  } else {
                    const x1 = x(a) + L;
                    const y1 = y(a) + H / 2;
                    const x2 = x(b);
                    const y2 = y(b) + H / 2;
                    const m = (x1 + x2) / 2;
                    d = `M${x1},${y1} C${m},${y1} ${m},${y2} ${x2 - 2},${y2}`;
                    // Probabilité près de l'arrivée : les cases cibles sont espacées, les libellés ne se chevauchent pas.
                    lx = x2 - 26;
                    ly = y2 - 6;
                  }
                  return (
                    <g key={`${l.de}>${l.vers}`}>
                      <path
                        d={d}
                        fill="none"
                        stroke={`var(--color-signal-${c})`}
                        strokeWidth={1.5}
                        strokeDasharray={c === "loss" ? "5 4" : undefined}
                        opacity={l.retour ? 0.6 : 0.9}
                        markerEnd={`url(#fleche-${c})`}
                      />
                      <text
                        x={lx}
                        y={ly}
                        textAnchor="middle"
                        fontSize="10"
                        fill="var(--color-text-muted)"
                        stroke="var(--color-ink-950)"
                        strokeWidth={3}
                        paintOrder="stroke"
                      >
                        {formatPercent(l.p)}
                      </text>
                    </g>
                  );
                })}
            </svg>

            {t.noeuds.map((n) => {
              const but = n.node.kind === "terminal";
              const racine = n.id === plan.rootId;
              const autres = t.autres[n.id];
              return (
                <div
                  key={n.id}
                  className={`absolute flex flex-col overflow-hidden rounded-chip border px-2 py-1.5 text-[11px] leading-tight ${
                    but
                      ? "border-signal-gain bg-ink-900 text-signal-gain"
                      : racine
                        ? "border-accent bg-ink-850"
                        : "border-ink-600 bg-ink-900"
                  }`}
                  style={{ left: x(n), top: y(n), width: L, height: H }}
                  title={decrireEtat(n.node.state, labels)}
                >
                  {n.node.kind === "terminal" ? (
                    <>
                      <span className="font-semibold">Objectif atteint</span>
                      <span className="text-text-subtle">{formatCount(n.node.expectedVisits)} fois par craft en moyenne</span>
                    </>
                  ) : (
                    <>
                      <span className="flex items-baseline gap-1">
                        <span className="min-w-0 flex-1 truncate font-semibold text-text-primary">
                          {n.node.action.isAbandon ? "Abandonner l'objet" : n.node.action.label}
                        </span>
                        {n.node.repeat && n.node.repeat.expectedAttempts > 1.05 && (
                          <span className="font-mono text-text-subtle">~{formatCount(n.node.repeat.expectedAttempts)}×</span>
                        )}
                      </span>
                      <span className="truncate text-text-muted">{decrireEtat(n.node.state, labels)}</span>
                      <span className="mt-auto flex justify-between text-text-subtle">
                        <span>
                          reste <span className="font-mono text-text-primary">{cout(n.node.costToGo)}</span>
                        </span>
                        {autres && (
                          <span title={`${autres.n} résultat(s) plus rare(s) non dessiné(s)`}>
                            +{autres.n} ({formatPercent(autres.p)})
                          </span>
                        )}
                      </span>
                    </>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
      <p className="text-[11px] text-text-subtle">
        Chaque case est un état de l'objet et la monnaie à utiliser ; « reste » est le coût espéré pour finir depuis cet état.
        Les résultats les plus rares sont regroupés (+N) ; survole une case pour l'état complet.
      </p>
    </div>
  );
}
