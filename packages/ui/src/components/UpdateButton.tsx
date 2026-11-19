import type { UpdatePhase } from "../types";
import { cx } from "../lib/cx";

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
