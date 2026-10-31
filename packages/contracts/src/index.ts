// Point d’entrée de @augur/contracts.
// Les types viennent de crates/augur-contracts (Rust, source de vérité) :
// régénérer avec `pnpm --filter @augur/contracts generate`.

export type { Affix } from "./generated/Affix";
export type { AppError } from "./generated/AppError";
export type { CharacterSource } from "./generated/CharacterSource";
export type { CharacterSummary } from "./generated/CharacterSummary";
export type { Confidence } from "./generated/Confidence";
export type { Element } from "./generated/Element";
export type { ErrorCode } from "./generated/ErrorCode";
export type { FeatureFlags } from "./generated/FeatureFlags";
export type { ItemDelta } from "./generated/ItemDelta";
export type { ItemMod } from "./generated/ItemMod";
export type { MechanicNote } from "./generated/MechanicNote";
export type { ModKind } from "./generated/ModKind";
export type { OverlayState } from "./generated/OverlayState";
export type { ParsedItem } from "./generated/ParsedItem";
export type { PolicyStatus } from "./generated/PolicyStatus";
export type { PriceEstimate } from "./generated/PriceEstimate";
export type { PriceSource } from "./generated/PriceSource";
export type { Rarity } from "./generated/Rarity";
export type { RateLimitStatus } from "./generated/RateLimitStatus";
export type { ResistanceDelta } from "./generated/ResistanceDelta";
export type { RuleWindowState } from "./generated/RuleWindowState";
export type { StatDelta } from "./generated/StatDelta";
export type { SupportLevel } from "./generated/SupportLevel";
export type { ZoneChanged } from "./generated/ZoneChanged";

/** Version du contrat ; doit égaler `OverlayState.contractVersion`. */
export const CONTRACT_VERSION = 1;

/** Commandes Tauri (`invoke`). Miroir de `augur_contracts::commands`. */
export const Commands = {
  parseClipboardItem: "parse_clipboard_item",
  getOverlayState: "get_overlay_state",
  evaluateItem: "evaluate_item",
  estimatePrice: "estimate_price",
  getRateLimitStatus: "get_rate_limit_status",
  getFeatureFlags: "get_feature_flags",
} as const;

/** Événements émis par le cœur. Miroir de `augur_contracts::events`. */
export const Events = {
  itemCopied: "augur://item-copied",
  zoneChanged: "augur://zone-changed",
  rateLimitChanged: "augur://rate-limit-changed",
  featureFlagsChanged: "augur://feature-flags-changed",
} as const;
