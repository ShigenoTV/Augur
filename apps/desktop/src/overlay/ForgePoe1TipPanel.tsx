import { Kbd, StatePanel } from "@augur/ui";
import type { ForgePoe1Tip } from "../bridge";
import { formatPoe1, useAffichagePoe1 } from "../index/devisePoe1";
import { libelleCourt } from "../index/forge";

export type ForgePoe1View = { status: "tip"; tip: ForgePoe1Tip } | { status: "error"; message: string };

/** Prochaine action du plan Forge PoE 1 suivi, pour l'objet copié en jeu (Ctrl+Alt+C). */
export function ForgePoe1TipPanel({ view, onClose, fermer }: { view: ForgePoe1View; onClose?: () => void; fermer: string }) {
  // Même monnaie d'affichage que la fenêtre principale (stockage local partagé).
  const { affichage } = useAffichagePoe1();
  const cout = (v: number | null) => formatPoe1(v, affichage);
  return (
    <div
      className="augur-panel w-[360px] p-3.5 text-text-primary motion-safe:animate-[augur-enter_120ms_var(--ease-out-quick)]"
      style={{ zoom: "var(--augur-scale)" }}
    >
      <p className="mb-2 text-2xs font-semibold uppercase tracking-wider text-accent">Forge PoE 1</p>
      {view.status === "error" ? (
        <StatePanel icon="alert" title="Objet non suivi" variant="warn">
          {view.message}
        </StatePanel>
      ) : (
        <Conseil tip={view.tip} cout={cout} />
      )}
      <footer className="mt-3 flex items-center justify-between gap-2 border-t border-ink-700 pt-2 text-2xs text-text-subtle">
        <button type="button" onClick={onClose} className="flex items-center gap-1 hover:text-text-muted">
          <Kbd>{fermer}</Kbd> fermer
        </button>
        <span>Plan suivi depuis la fenêtre Augur</span>
      </footer>
    </div>
  );
}

function Conseil({ tip, cout }: { tip: ForgePoe1Tip; cout: (v: number | null) => string }) {
  return (
    <>
      <p className="text-sm font-semibold">{tip.baseNom}</p>
      <p className="text-2xs text-text-subtle">
        {tip.rarete} · {tip.presents.length} mod(s) voulu(s) sur {tip.presents.length + tip.manquants.length}
      </p>
      <ul className="mt-2 space-y-0.5 text-2xs">
        {tip.presents.map((m) => (
          <li key={`p-${m}`} className="text-signal-gain">
            ✓ {libelleCourt(m)}
          </li>
        ))}
        {tip.manquants.map((m) => (
          <li key={`m-${m}`} className="text-text-subtle">
            ○ {libelleCourt(m)}
          </li>
        ))}
      </ul>
      <div className="augur-divider my-3" />
      {tip.fini ? (
        <p className="text-base font-semibold text-signal-gain">Objet fini : tous les mods voulus sont là.</p>
      ) : tip.action ? (
        <div>
          <p className="text-2xs uppercase tracking-wider text-text-subtle">Action conseillée</p>
          <p className="text-base font-semibold text-accent">{tip.action}</p>
          <p className="text-2xs text-text-subtle">
            {tip.cout != null ? `${cout(tip.cout)} par utilisation · ` : ""}environ {cout(tip.reste)} restant en moyenne
          </p>
        </div>
      ) : (
        <p className="text-2xs text-text-subtle">Pas de plan conseillé : il manque des prix (onglet Forge).</p>
      )}
    </>
  );
}
