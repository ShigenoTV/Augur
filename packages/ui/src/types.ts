/**
 * Types du contrat application ↔ interface : générés par ts-rs depuis
 * `crates/augur-contracts` (paquet `@augur/contracts`), réexportés ici pour
 * que les composants n'aient qu'une source d'import. Seuls `CopilotView` et
 * les types de mise à jour, en bas, sont propres à l'interface.
 */

import type {
  AppError,
  CharacterSummary,
  ItemDelta,
  ParsedItem,
  PriceEstimate,
  RateLimitStatus,
} from "@augur/contracts";

export type * from "@augur/contracts";

// ── Propre à l'interface ───────────────────────────────────────────────────

/**
 * `unpriced` : pas de cote automatique pour cet objet (uniques et rares : cote trade sur raccourci).
 * `unavailable` : la cote n'a pas pu être obtenue (réseau, limite du site de trade…).
 */
export type PricingState = "pending" | "done" | "unavailable" | "disabled" | "unpriced";

/**
 * Modèle de vue de l'overlay Upgrade Copilot, union discriminée sur `status`.
 * Construit côté front à partir des commandes et événements du contrat.
 */
export type CopilotView =
  | { status: "hidden" }
  | { status: "evaluating"; item?: ParsedItem }
  | {
      status: "ready";
      item: ParsedItem;
      /** Absent quand l'évaluation est coupée par le kill switch. */
      delta?: ItemDelta;
      price?: PriceEstimate;
      pricing: PricingState;
      rateLimit?: RateLimitStatus;
      character?: CharacterSummary;
      /** Objet sans comparaison possible (monnaie, essence…) : seul le prix est affiché. */
      priceOnly?: boolean;
      /** Précision affichée sous le prix (pas de cote, erreur du site de trade…). */
      priceNote?: string;
    }
  | {
      status: "noCharacter";
      item: ParsedItem;
      /** Le prix reste disponible sans personnage importé. */
      price?: PriceEstimate;
      pricing?: PricingState;
      priceNote?: string;
    }
  | { status: "unrecognized"; error: AppError }
  | { status: "disabled"; reason: string };

// ── Mise à jour de l'application (apps/desktop, hors contrat moteur) ──────

export type UpdateChannel = "stable" | "beta";

export type UpdateInfo = {
  version: string;
  currentVersion: string;
  notes: string | null;
  publishedAt: string | null;
  channel: UpdateChannel;
};

export type UpdateProgress = { downloaded: number; total: number | null };

export type UpdatePhase =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "upToDate"; checkedAt: string }
  | { phase: "available"; info: UpdateInfo }
  | { phase: "installing"; info: UpdateInfo; progress: UpdateProgress | null }
  | { phase: "error"; message: string };
