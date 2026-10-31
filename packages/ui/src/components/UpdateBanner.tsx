import { useState } from "react";
import type { UpdatePhase } from "../types";
import { cx } from "../lib/cx";
import { Icon } from "./Icon";

export interface UpdateBannerProps {
  update: UpdatePhase;
  onInstall?: () => void;
  onLater?: () => void;
  className?: string;
}

function percent(downloaded: number, total: number | null): number | undefined {
  if (!total) return undefined;
  return Math.min(100, Math.round((downloaded / total) * 100));
}

function mb(bytes: number): string {
  return `${(bytes / 1_048_576).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} Mo`;
}

/**
 * Bandeau de mise à jour de la fenêtre principale. Discret, jamais modal :
 * le joueur installe quand il veut, jamais pendant une partie à son insu.
 */
export function UpdateBanner({ update, onInstall, onLater, className }: UpdateBannerProps) {
  const [showNotes, setShowNotes] = useState(false);

  if (update.phase === "available") {
    const { info } = update;
    return (
      <section
        role="status"
        className={cx("rounded-panel border border-accent/40 bg-accent/5 px-4 py-3", className)}
        aria-label="Mise à jour disponible"
      >
        <div className="flex flex-wrap items-center gap-3">
          <Icon name="info" size={16} className="shrink-0 text-accent" />
          <p className="text-sm">
            <span className="font-semibold text-text-primary">Augur {info.version} est disponible</span>
            <span className="text-text-subtle">
              {" "}
              · tu as la {info.currentVersion}
              {info.channel === "beta" && " · canal bêta"}
            </span>
          </p>
          <div className="ml-auto flex items-center gap-2">
            {info.notes && (
              <button
                type="button"
                onClick={() => setShowNotes((v) => !v)}
                aria-expanded={showNotes}
                className="text-xs text-text-muted hover:text-text-primary"
              >
                {showNotes ? "Masquer les nouveautés" : "Nouveautés"}
              </button>
            )}
            <button type="button" onClick={onLater} className="rounded-chip px-2.5 py-1 text-sm text-text-muted hover:bg-ink-800">
              Plus tard
            </button>
            <button
              type="button"
              onClick={onInstall}
              className="rounded-chip bg-accent px-3 py-1 text-sm font-semibold text-accent-ink hover:bg-accent-strong"
            >
              Installer et redémarrer
            </button>
          </div>
        </div>
        {showNotes && info.notes && (
          <p className="mt-2 whitespace-pre-line border-t border-ink-700 pt-2 text-sm text-text-muted">{info.notes}</p>
        )}
      </section>
    );
  }

  if (update.phase === "installing") {
    const p = update.progress;
    const pct = p ? percent(p.downloaded, p.total) : undefined;
    return (
      <section role="status" className={cx("rounded-panel border border-ink-700 bg-ink-900 px-4 py-3", className)}>
        <p className="mb-2 text-sm text-text-primary">
          Installation d'Augur {update.info.version}… Augur redémarre tout seul à la fin.
        </p>
        <div
          className="h-1.5 overflow-hidden rounded-full bg-ink-700"
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
        {p && (
          <p className="mt-1 text-2xs text-text-subtle tabular-nums">
            {mb(p.downloaded)}
            {p.total ? ` sur ${mb(p.total)}` : ""}
          </p>
        )}
      </section>
    );
  }

  if (update.phase === "error") {
    return (
      <p role="status" className={cx("flex items-center gap-2 text-sm text-signal-warn", className)}>
        <Icon name="alert" size={14} /> {update.message}
      </p>
    );
  }

  return null;
}
