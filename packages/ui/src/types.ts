/**
 * Types du contrat application ↔ interface.
 *
 * MIROIR TEMPORAIRE de `@augur/contracts` (généré par ts-rs depuis
 * `crates/augur-contracts`, chantier Architecture, pas encore fusionné).
 * Les noms et les formes sont recopiés à l'identique : à la fusion, la
 * première section de ce fichier devient `export * from "@augur/contracts"`.
 * Seul `CopilotView`, en bas, est propre à l'interface.
 */

// ── Miroir de @augur/contracts ─────────────────────────────────────────────

export type Rarity = "normal" | "magic" | "rare" | "unique" | "currency" | "gem" | "other";

export type ModKind = "implicit" | "explicit" | "enchant" | "rune" | "crafted" | "fractured" | "desecrated";

export type Affix = "prefix" | "suffix";

export type ItemMod = {
  /** Texte tel qu'affiché, pour l'UI. */
  text: string;
  kind: ModKind;
  /** Préfixe ou suffixe : connu seulement en Ctrl+Alt+C. */
  affix: Affix | null;
  /** Tier (1 = meilleur) : connu seulement en Ctrl+Alt+C. */
  tier: number | null;
  statIds: Array<string>;
  values: Array<number>;
};

export type ParsedItem = {
  rawText: string;
  advanced: boolean;
  itemClass: string;
  rarity: Rarity;
  name: string | null;
  baseType: string;
  itemLevel: number | null;
  quality: number | null;
  mods: Array<ItemMod>;
  corrupted: boolean;
  unidentified: boolean;
  dataPatch: string;
};

export type SupportLevel = "verified" | "approximate" | "unsupported";

/** Niveau de confiance d'une mécanique (fiches du Theorycrafter). */
export type Confidence = "confirmed" | "measured" | "assumed";

export type Element = "fire" | "cold" | "lightning" | "chaos";

export type StatDelta = { before: number; after: number };

export type ResistanceDelta = { element: Element; before: number; after: number; cap: number };

export type MechanicNote = { mechanic: string; confidence: Confidence };

export type ItemDelta = {
  /** Identifiant d'emplacement (« weapon1 », « helmet ») ; libellé traduit dans l'UI. */
  slot: string;
  /** Nom de l'objet remplacé. */
  equippedName: string | null;
  /** Set d'armes concerné (PoE 2). */
  weaponSet: 1 | 2 | null;
  support: SupportLevel;
  dps: StatDelta;
  ehp: StatDelta;
  resistances: Array<ResistanceDelta>;
  spirit: StatDelta | null;
  mechanics: Array<MechanicNote>;
  computeMs: number;
};

export type PriceSource = "trade" | "currencyExchange" | "model";

export type PriceEstimate = {
  /** Devise de référence, identifiant du jeu (ex. « exalted »). */
  currency: string;
  low: number;
  median: number;
  high: number;
  sampleSize: number;
  source: PriceSource;
  league: string;
  observedAt: string;
  /** Donnée plus vieille que la fraîcheur attendue : on affiche l'horodatage. */
  stale: boolean;
};

export type RuleWindowState = { rule: string; hits: number; maxHits: number; usableHits: number; periodS: number };

export type PolicyStatus = {
  policy: string;
  windows: Array<RuleWindowState>;
  blockedForMs: number | null;
  queued: number;
};

export type RateLimitStatus = { policies: Array<PolicyStatus> };

export type FeatureFlags = {
  clipboardParse: boolean;
  itemEvaluation: boolean;
  tradePriceCheck: boolean;
  oauthCharacterImport: boolean;
  pobImport: boolean;
};

export type ErrorCode =
  | "parseFailed"
  | "unsupported"
  | "noCharacter"
  | "rateLimited"
  | "featureDisabled"
  | "network"
  | "oauth"
  | "internal";

export type AppError = { code: ErrorCode; message: string; retryAfterMs: number | null };

export type CharacterSource = "oauth" | "pobCode";

export type CharacterSummary = { name: string; class: string; level: number; league: string; source: CharacterSource };

/** Résultat de `get_overlay_state`. */
export type OverlayState = {
  contractVersion: number;
  appVersion: string;
  gameFocused: boolean;
  character: CharacterSummary | null;
  lastItem: ParsedItem | null;
  dataPatch: string | null;
};

/** Charge de `augur://zone-changed`. */
export type ZoneChanged = { zone: string; at: string };

// ── Propre à l'interface ───────────────────────────────────────────────────

export type PricingState = "pending" | "done" | "unavailable" | "disabled";

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
    }
  | { status: "noCharacter"; item: ParsedItem }
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
