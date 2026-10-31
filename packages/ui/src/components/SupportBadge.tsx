import type { SupportLevel } from "../types";
import { cx } from "../lib/cx";
import { Icon, type IconName } from "./Icon";

const config: Record<SupportLevel, { label: string; icon: IconName; className: string; title: string }> = {
  verified: {
    label: "Vérifié",
    icon: "check",
    className: "text-signal-gain border-signal-gain/40 bg-signal-gain/10",
    title: "Archétype couvert par des cas de test validés en jeu.",
  },
  approximate: {
    label: "Approximatif",
    icon: "approx",
    className: "text-signal-warn border-signal-warn/40 bg-signal-warn/10",
    title: "Certaines mécaniques de ce build ne sont pas encore modélisées.",
  },
  unsupported: {
    label: "Non supporté",
    icon: "ban",
    className: "text-signal-loss border-signal-loss/40 bg-signal-loss/10",
    title: "Augur ne sait pas encore calculer ce build : aucun chiffre n'est affiché.",
  },
};

export function SupportBadge({ level }: { level: SupportLevel }) {
  const c = config[level];
  return (
    <span
      title={c.title}
      className={cx(
        "inline-flex items-center gap-1 rounded-chip border px-1.5 py-px text-2xs font-semibold uppercase tracking-wide",
        c.className,
      )}
    >
      <Icon name={c.icon} size={12} />
      {c.label}
    </span>
  );
}
