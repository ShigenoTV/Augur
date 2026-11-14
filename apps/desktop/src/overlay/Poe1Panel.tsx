import { Kbd, StatePanel } from "@augur/ui";
import type { Poe1ItemInfo, Poe1Price } from "../bridge";

export type Poe1View =
  | { status: "loading" }
  | { status: "ready"; info: Poe1ItemInfo }
  | { status: "error"; message: string };

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
  onClose,
  onTrade,
  fermer,
  trade,
}: {
  view: Poe1View;
  onClose?: () => void;
  onTrade?: () => void;
  fermer: string;
  trade: string;
}) {
  return (
    <div
      className="augur-panel w-[340px] p-3.5 text-text-primary motion-safe:animate-[augur-enter_120ms_var(--ease-out-quick)]"
      style={{ zoom: "var(--augur-scale)" }}
    >
      <p className="mb-2 text-2xs font-semibold uppercase tracking-wider text-accent">PoE 1</p>
      {view.status === "loading" && <p className="text-sm text-text-subtle">Lecture de l'objet…</p>}
      {view.status === "error" && (
        <StatePanel icon="alert" title="Objet non lu" variant="warn">
          {view.message}
        </StatePanel>
      )}
      {view.status === "ready" && <Contenu info={view.info} />}
      <footer className="mt-3 flex items-center justify-between gap-2 border-t border-ink-700 pt-2 text-2xs text-text-subtle">
        <button type="button" onClick={onClose} className="flex items-center gap-1 hover:text-text-muted">
          <Kbd>{fermer}</Kbd> fermer
        </button>
        {view.status === "ready" && (
          <button type="button" onClick={onTrade} className="flex items-center gap-1 hover:text-text-muted">
            <Kbd>{trade}</Kbd> chercher sur le trade
          </button>
        )}
      </footer>
    </div>
  );
}

function Contenu({ info }: { info: Poe1ItemInfo }) {
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
      <p className="mt-0.5 text-2xs text-text-subtle">{details.join(" · ")}</p>
      {info.mods.length > 0 && (
        <ul className="mt-2 max-h-40 space-y-0.5 overflow-hidden text-2xs text-text-muted">
          {info.mods.slice(0, 10).map((m, i) => (
            <li key={i}>{m}</li>
          ))}
        </ul>
      )}
      <div className="augur-divider my-3" />
      {info.price ? (
        <div>
          <p className="text-base font-semibold text-signal-gain">{formatCote(info.price)}</p>
          <p className="text-2xs text-text-subtle">
            Cote approximative en {info.league}
            {info.price.annonces !== null ? `, ${info.price.annonces} annonce${info.price.annonces > 1 ? "s" : ""}` : ""}
            {info.price.liens ? `, ${info.price.liens} liens` : ""}.
          </p>
        </div>
      ) : (
        <p className="text-2xs text-text-subtle">{info.priceNote ?? "Pas de cote."}</p>
      )}
      {info.tradeSummary.length > 0 && (
        <p className="mt-2 text-2xs text-text-subtle">Recherche de trade : {info.tradeSummary.join(", ")}.</p>
      )}
    </>
  );
}
