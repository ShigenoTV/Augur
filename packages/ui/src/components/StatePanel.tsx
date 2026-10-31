import type { ReactNode } from "react";
import { cx } from "../lib/cx";
import { Icon, type IconName } from "./Icon";

const tone = {
  neutral: "text-text-muted",
  info: "text-signal-info",
  warn: "text-signal-warn",
  danger: "text-signal-loss",
} as const;

/** États non nominaux de l'overlay : un titre, une phrase, et l'action possible. */
export function StatePanel({
  icon,
  title,
  children,
  variant = "neutral",
}: {
  icon: IconName;
  title: string;
  children?: ReactNode;
  variant?: keyof typeof tone;
}) {
  return (
    <div className="flex gap-2.5" role="status">
      <Icon name={icon} size={16} className={cx("mt-0.5 shrink-0", tone[variant])} />
      <div className="space-y-1">
        <p className={cx("text-sm font-semibold", tone[variant])}>{title}</p>
        {children && <div className="text-sm text-text-muted">{children}</div>}
      </div>
    </div>
  );
}

/** Touche de clavier, pour les aides en pied d'overlay. */
export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded-chip border border-ink-600 bg-ink-800 px-1 py-px font-sans text-2xs font-semibold text-text-muted">
      {children}
    </kbd>
  );
}
