import type { PriceEstimate, PricingState, RateLimitStatus } from "../types";
import { cx } from "../lib/cx";
import { formatAge, formatDuration, formatPrice } from "../lib/format";
import { priceSourceLabel } from "../lib/labels";
import { useCountdown } from "../lib/useCountdown";
import { Icon } from "./Icon";

export function Shimmer({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      className={cx(
        "inline-block rounded-chip bg-[linear-gradient(90deg,var(--color-ink-800),var(--color-ink-700),var(--color-ink-800))] bg-[length:200px_100%] motion-safe:animate-[augur-shimmer_1.2s_linear_infinite]",
        className,
      )}
    />
  );
}

/** Agrège les politiques GGG : total en file, restriction la plus longue. */
export function summarizeRateLimit(status: RateLimitStatus): { queued: number; blockedForMs: number | undefined } {
  let queued = 0;
  let blocked: number | undefined;
  for (const p of status.policies) {
    queued += p.queued;
    if (p.blockedForMs !== null && p.blockedForMs > 0) blocked = Math.max(blocked ?? 0, p.blockedForMs);
  }
  return { queued, blockedForMs: blocked };
}

/**
 * File d'attente du rate limiter local. Le budget de requêtes est celui du
 * joueur : on l'affiche au lieu de le cacher (plan fondateur §1.3).
 */
export function RateLimitNotice({ status }: { status: RateLimitStatus }) {
  const { queued, blockedForMs } = summarizeRateLimit(status);
  const blocked = useCountdown(blockedForMs);
  if (blocked !== undefined && blocked > 0) {
    return (
      <p className="flex items-center gap-1.5 text-xs text-signal-warn" role="status">
        <Icon name="pause" size={12} />
        Limite du site de trade atteinte{" "}: reprise dans {formatDuration(blocked)}
      </p>
    );
  }
  if (queued === 0) return null;
  return (
    <p className="flex items-center gap-1.5 text-xs text-text-subtle" role="status">
      <Icon name="clock" size={12} />
      {queued} recherche{queued > 1 ? "s" : ""} en file, lancée{queued > 1 ? "s" : ""} dès que le site de trade le
      permet
    </p>
  );
}

export interface PriceTagProps {
  pricing: PricingState;
  price?: PriceEstimate;
  rateLimit?: RateLimitStatus;
  /** Précision affichée à la place du message par défaut (pas de cote, erreur…). */
  note?: string;
  /** Raccourci de la cote trade, proposé quand l'objet n'a pas de cote automatique. */
  priceCheckKey?: string;
  now?: Date;
}

/** Prix médian, fourchette, âge et source. Un prix vieux est montré comme tel. */
export function PriceTag({ pricing, price, rateLimit, note, priceCheckKey, now }: PriceTagProps) {
  if (pricing === "disabled") {
    return <p className="text-xs text-text-subtle">Prix désactivé temporairement.</p>;
  }
  if ((pricing === "unavailable" || pricing === "unpriced") && !price) {
    const texte =
      note ??
      (pricing === "unpriced"
        ? priceCheckKey
          ? `Pas de cote automatique : ${priceCheckKey} pour coter sur le trade.`
          : "Pas de cote automatique pour cet objet."
        : "Prix indisponible pour l'instant.");
    return (
      <div className="space-y-1">
        <p className={cx("flex items-center gap-1.5 text-xs", pricing === "unavailable" ? "text-signal-warn" : "text-text-subtle")}>
          <Icon name="info" size={12} /> {texte}
        </p>
        {note && pricing === "unpriced" && priceCheckKey && (
          <p className="text-2xs text-text-subtle">{priceCheckKey} pour coter sur le trade.</p>
        )}
        {rateLimit && <RateLimitNotice status={rateLimit} />}
      </div>
    );
  }
  if (pricing === "pending" && !price) {
    return (
      <div className="space-y-1.5" aria-busy="true" aria-label="Recherche du prix en cours">
        <div className="flex items-baseline justify-between">
          <span className="text-xs text-text-subtle">Prix</span>
          <Shimmer className="h-5 w-20" />
        </div>
        {rateLimit && <RateLimitNotice status={rateLimit} />}
      </div>
    );
  }
  if (!price) return null;

  const pile = price.quantity !== undefined && price.quantity > 1;
  const plage = price.low !== price.high;
  return (
    <div className="space-y-1">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-xs text-text-subtle">{pile ? `Prix de la pile (${price.quantity})` : "Prix"}</span>
        <span className={cx("text-lg font-semibold tabular-nums", price.stale ? "text-text-muted" : "text-text-primary")}>
          {formatPrice(price.median, price.currency)}
        </span>
      </div>
      <p className="flex items-center justify-between gap-3 text-2xs text-text-subtle">
        <span className="tabular-nums">
          {pile && price.unitPrice !== undefined && `${formatPrice(price.unitPrice, price.currency)} l'unité`}
          {!pile && plage && `${formatPrice(price.low, price.currency)} – ${formatPrice(price.high, price.currency)}`}
          {price.source === "trade" && ` · ${price.sampleSize} annonce${price.sampleSize > 1 ? "s" : ""}`}
        </span>
        <span className={cx("flex items-center gap-1", price.stale && "text-signal-warn")}>
          {price.stale && <Icon name="clock" size={11} />}
          {priceSourceLabel(price.source)} · {formatAge(price.observedAt, now)}
        </span>
      </p>
      {note && <p className="text-2xs text-text-subtle">{note}</p>}
      {rateLimit && <RateLimitNotice status={rateLimit} />}
    </div>
  );
}
