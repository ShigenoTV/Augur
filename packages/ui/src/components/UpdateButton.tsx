import type { UpdatePhase } from "../types";
import { cx } from "../lib/cx";
import { Icon } from "./Icon";

export interface UpdateButtonProps {
  update: UpdatePhase;
  /** Lance une recherche de mise à jour. */
  onCheck?: () => void;
  /** Installe la version disponible puis redémarre Augur. */
  onInstall?: () => void;
  /** Libellé au repos (aucune mise à jour connue). */
  idleLabel?: string;
  className?: string;
}

/**
 * Bouton unique de mise à jour : il recherche une mise à jour, puis devient
 * « Installer et redémarrer » dès qu'une version est disponible. Le joueur
 * installe au même endroit que là où il a cherché.
 */
export function UpdateButton({
  update,
  onCheck,
  onInstall,
  idleLabel = "Rechercher une mise à jour",
  className,
}: UpdateButtonProps) {
  const available = update.phase === "available";
  const busy = update.phase === "checking" || update.phase === "installing";

  const label =
    update.phase === "available"
      ? "Installer et redémarrer"
      : update.phase === "installing"
        ? "Installation…"
        : update.phase === "checking"
          ? "Vérification…"
          : update.phase === "upToDate"
            ? "Augur est à jour"
            : idleLabel;

  return (
    <button
      type="button"
      onClick={available ? onInstall : onCheck}
      disabled={busy}
      title={available ? `Installer Augur ${update.info.version} (tu as la ${update.info.currentVersion})` : undefined}
      className={cx(
        "rounded-chip border disabled:opacity-50",
        available
          ? "border-accent bg-accent font-semibold text-accent-ink hover:bg-accent-strong"
          : "border-ink-600 text-text-primary hover:bg-ink-800",
        className,
      )}
    >
      {label}
    </button>
  );
}

/**
 * Ligne d'état au-dessus du bouton : version disponible, progression de
 * l'installation ou erreur. Rien quand il n'y a rien à dire.
 */
export function UpdateStatus({ update, className }: { update: UpdatePhase; className?: string }) {
  if (update.phase === "available") {
    return (
      <p role="status" className={cx("text-[11px] font-semibold text-accent", className)}>
        Augur {update.info.version} est disponible
      </p>
    );
  }
  if (update.phase === "installing") {
    const p = update.progress;
    const pct = p?.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : undefined;
    return (
      <div role="status" className={cx("space-y-1", className)}>
        <p className="text-[11px] text-text-primary">Installation d'Augur {update.info.version}…</p>
        <div
          className="h-1 overflow-hidden rounded-full bg-ink-700"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={pct}
        >
          <div
            className={cx("h-full bg-accent transition-[width]", pct === undefined && "w-1/3 motion-safe:animate-pulse")}
            style={pct !== undefined ? { width: `${pct}%` } : undefined}
          />
        </div>
      </div>
    );
  }
  if (update.phase === "error") {
    return (
      <p role="status" className={cx("flex items-start gap-1 text-[11px] text-signal-warn", className)}>
        <Icon name="alert" size={12} className="mt-0.5 shrink-0" /> {update.message}
      </p>
    );
  }
  return null;
}
