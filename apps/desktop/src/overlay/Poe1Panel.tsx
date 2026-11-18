import { cx, DeltaPanel, Kbd, StatePanel, formatNumber, slotLabel } from "@augur/ui";
import { useEffect, useState } from "react";
import type { Poe1Evaluation, Poe1ItemInfo, Poe1Line, Poe1Price } from "../bridge";

export type Poe1View =
  | { status: "loading" }
  | { status: "ready"; info: Poe1ItemInfo }
  | { status: "error"; message: string };

/** Comparaison avec l'équipement du build importé (Path of Building embarqué). */
export type Poe1EvalView =
  | { status: "loading" }
  | { status: "ready"; evaluation: Poe1Evaluation }
  | { status: "noCharacter" }
  | { status: "error"; message: string }
  | { status: "none" };

/** Lignes de PoB affichées sous le delta, pour garder l'overlay compact. */
const MAX_LIGNES = 8;

/** Une ligne de PoB est-elle un gain ? (certaines stats valent mieux en baisse). */
export function ligneGagnante(l: Poe1Line): boolean {
  return l.lowerIsBetter ? l.after < l.before : l.after > l.before;
}

/** Écart d'une ligne de PoB : « +72 » ou « −3,5 ». */
export function ecart(l: Poe1Line): string {
  const d = l.after - l.before;
  const abs = Math.abs(d);
  const txt = abs >= 100 ? formatNumber(Math.round(abs)) : abs.toLocaleString("fr-FR", { maximumFractionDigits: 1 });
  return `${d >= 0 ? "+" : "−"}${txt}`;
}

const RARETE: Record<string, string> = {
  normal: "Normal",
  magic: "Magique",
  rare: "Rare",
  unique: "Unique",
  currency: "Devise",
  gem: "Gemme",
  divinationCard: "Carte de divination",
};

const nombre = (v: number) => v.toLocaleString("fr-FR", { maximumFractionDigits: v < 10 ? 1 : 0 });

/** Cote lisible : « ≈ 157 c · 2 div » ou « ≈ 10 à 25 c ». */
export function formatCote(p: Poe1Price): string {
  const chaos = p.chaosMax ? `${nombre(p.chaos)} à ${nombre(p.chaosMax)} c` : `${nombre(p.chaos)} c`;
  const div = p.divine !== null && p.divine >= 0.5 ? ` · ${p.divine.toLocaleString("fr-FR", { maximumFractionDigits: 2 })} div` : "";
  return `≈ ${chaos}${div}`;
}

/** Overlay de PoE 1 : l'objet copié, sa cote poe.ninja et la recherche de trade. */
export function Poe1Panel({
  view,
  evaluation,
  onClose,
  onTrade,
  fermer,
  trade,
  compact: compactReglage = false,
  pied,
}: {
  view: Poe1View;
  evaluation?: Poe1EvalView;
  onClose?: () => void;
  onTrade?: () => void;
  fermer: string;
  trade: string;
  /** Version compacte (Réglages) : nom, verdict, cote ; « détails » déplie le reste. */
  compact?: boolean;
  /** Contrôle ajouté au pied (épinglage de l'overlay). */
  pied?: React.ReactNode;
}) {
  // « détails » vaut pour l'objet affiché seulement.
  const [deplie, setDeplie] = useState(false);
  const info = view.status === "ready" ? view.info : null;
  useEffect(() => setDeplie(false), [info]);
  const compact = compactReglage && !deplie;
  return (
    <div
      className={cx(
        "augur-panel text-text-primary motion-safe:animate-[augur-enter_120ms_var(--ease-out-quick)]",
        compact ? "w-[280px] p-2.5" : "w-[360px] p-3.5",
      )}
      style={{ zoom: "var(--augur-scale)" }}
    >
      <p className={cx("text-2xs font-semibold uppercase tracking-wider text-accent", compact ? "mb-1" : "mb-2")}>PoE 1</p>
      {view.status === "loading" && <p className="text-sm text-text-subtle">Lecture de l'objet…</p>}
      {view.status === "error" && (
        <StatePanel icon="alert" title="Objet non lu" variant="warn">
          {view.message}
        </StatePanel>
      )}
      {view.status === "ready" && (
        <Contenu info={view.info} evaluation={evaluation ?? { status: "none" }} compact={compact} />
      )}
      <footer
        className={cx(
          "flex items-center justify-between gap-2 border-t border-ink-700 pt-2 text-2xs text-text-subtle",
          compact ? "mt-2" : "mt-3",
        )}
      >
        <button type="button" onClick={onClose} className="flex items-center gap-1 hover:text-text-muted">
          <Kbd>{fermer}</Kbd> fermer
        </button>
        {compactReglage && view.status === "ready" && (
          <button
            type="button"
            onClick={() => setDeplie((v) => !v)}
            aria-expanded={deplie}
            className="hover:text-text-muted"
          >
            {deplie ? "réduire" : "détails"}
          </button>
        )}
        {view.status === "ready" && (
          <button type="button" onClick={onTrade} className="flex items-center gap-1 hover:text-text-muted">
            <Kbd>{trade}</Kbd> {compact ? "trade" : "chercher mieux sur le trade"}
          </button>
        )}
        {pied}
      </footer>
    </div>
  );
}

function Comparaison({ view, compact = false }: { view: Poe1EvalView; compact?: boolean }) {
  if (view.status === "none") return null;
  if (view.status === "loading") {
    return <p className="text-2xs text-text-subtle">Comparaison avec ton build…</p>;
  }
  if (view.status === "noCharacter") {
    return (
      <p className="text-2xs text-text-subtle">
        Importe ton code Path of Building (page Copilot) pour savoir si cet objet est mieux que le tien.
      </p>
    );
  }
  if (view.status === "error") {
    return <p className="text-2xs text-signal-warn">Comparaison impossible : {view.message}</p>;
  }
  const { delta, others, lines } = view.evaluation;
  if (compact) return <DeltaPanel delta={delta} compact />;
  const shown = lines.slice(0, MAX_LIGNES);
  return (
    <div className="space-y-2">
      <DeltaPanel delta={delta} />
      {shown.length > 0 && (
        <ul className="space-y-0.5 text-2xs" aria-label="Détail du calcul">
          {shown.map((l) => (
            <li key={l.label} className="flex justify-between gap-2">
              <span className="text-text-muted">{l.label}</span>
              <span className={`tabular-nums ${ligneGagnante(l) ? "text-signal-gain" : "text-signal-loss"}`}>{ecart(l)}</span>
            </li>
          ))}
          {lines.length > shown.length && <li className="text-text-subtle">et {lines.length - shown.length} autres</li>}
        </ul>
      )}
      {others.map((o) => (
        <p key={o.slot} className="text-2xs text-text-subtle">
          À la place de {o.equippedName ?? slotLabel(o.slot)} ({slotLabel(o.slot)}) : DPS{" "}
          {o.dps.before > 0 ? `${o.dps.after >= o.dps.before ? "+" : "−"}${Math.abs(((o.dps.after - o.dps.before) / o.dps.before) * 100).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} %` : "inchangé"}
        </p>
      ))}
    </div>
  );
}

function Contenu({
  info,
  evaluation,
  compact = false,
}: {
  info: Poe1ItemInfo;
  evaluation: Poe1EvalView;
  compact?: boolean;
}) {
  const separateur = <div className={cx("augur-divider", compact ? "my-2" : "my-3")} />;
  const details = [
    RARETE[info.rarity] ?? info.rarity,
    info.itemLevel ? `niveau d'objet ${info.itemLevel}` : null,
    info.links >= 5 ? `${info.links} liens` : null,
    ...info.influences.map((i) => i.replace(/ Item$/, "")),
    info.corrupted ? "corrompu" : null,
  ].filter(Boolean);
  return (
    <>
      <p className="text-sm font-semibold">{info.name ?? info.base}</p>
      {info.name && <p className="text-2xs text-text-muted">{info.base}</p>}
      {!compact && <p className="mt-0.5 text-2xs text-text-subtle">{details.join(" · ")}</p>}
      {!compact && info.mods.length > 0 && (
        <ul className="mt-2 max-h-40 space-y-0.5 overflow-hidden text-2xs text-text-muted">
          {info.mods.slice(0, 10).map((m, i) => (
            <li key={i}>{m}</li>
          ))}
        </ul>
      )}
      {evaluation.status !== "none" && (
        <>
          {separateur}
          <Comparaison view={evaluation} compact={compact} />
        </>
      )}
      {separateur}
      {info.price ? (
        <div>
          <p className="text-base font-semibold text-signal-gain">{formatCote(info.price)}</p>
          {!compact && (
            <p className="text-2xs text-text-subtle">
            Cote approximative en {info.league}
            {info.price.annonces !== null ? `, ${info.price.annonces} annonce${info.price.annonces > 1 ? "s" : ""}` : ""}
            {info.price.liens ? `, ${info.price.liens} liens` : ""}.
          </p>
          )}
        </div>
      ) : (
        <p className="text-2xs text-text-subtle">{info.priceNote ?? "Pas de cote."}</p>
      )}
      {!compact && info.tradeSummary.length > 0 && (
        <p className="mt-2 text-2xs text-text-subtle">Recherche de trade : {info.tradeSummary.join(", ")}.</p>
      )}
    </>
  );
}
