/**
 * Pont typé vers le cœur Rust : commandes et événements du contrat.
 * Hors Tauri (navigateur, tests), `isTauri()` est faux et l'interface
 * bascule sur les données fictives de @augur/ui.
 */
import type {
  AppError,
  CharacterSummary,
  ItemDelta,
  OverlayState,
  ParsedItem,
  PriceEstimate,
  UpdateInfo,
  UpdateProgress,
  ZoneChanged,
} from "@augur/ui";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const EVENTS = {
  itemCopied: "augur://item-copied",
  /** Texte copié qui ressemble à un objet mais que le parseur refuse (charge : AppError). */
  itemUnrecognized: "augur://item-unrecognized",
  zoneChanged: "augur://zone-changed",
  overlayHidden: "augur://overlay-hidden",
  upgradeSearchRequested: "augur://upgrade-search-requested",
  rateLimitChanged: "augur://rate-limit-changed",
  featureFlagsChanged: "augur://feature-flags-changed",
  updateAvailable: "augur://update-available",
  updateProgress: "augur://update-progress",
  updateCheckRequested: "augur://update-check-requested",
  clientLogStatus: "augur://client-log-status",
  measureSession: "augur://measure-session",
  ledgerSession: "augur://ledger-session",
  characterChanged: "augur://character-changed",
} as const;

export type Settings = { betaChannel: boolean; clientLogPath: string | null };

/** État du suivi de Client.txt (apps/desktop/src-tauri/src/client_log.rs). */
export type ClientLogStatus = {
  path: string | null;
  watching: boolean;
  lastZone: ZoneChanged | null;
  searched: string[];
};

/** Session de mesures (apps/desktop/src-tauri/src/measures.rs). */
export type MeasureCounts = { items: number; pobCodes: number; zones: number; notes: number };
export type MeasureExport = { jsonPath: string; markdownPath: string; counts: MeasureCounts };
export type MeasureStatus = {
  active: boolean;
  startedAt: string | null;
  counts: MeasureCounts;
  lastExport: MeasureExport | null;
};

/** Atlas Ledger : bilan calculé par poe2-core (crates/poe2-core/src/ledger/session.rs). */
export type ZoneKind = "map" | "hideout" | "ville" | "carrefour" | "autre";
export type LedgerMapRun = {
  areaId: string;
  nom: string | null;
  niveau: number;
  graine: number;
  debut: number;
  fin: number;
  dureeActive: number;
  entrees: number;
  sousZones: number;
};
export type LedgerTime = { map: number; hideout: number; ville: number; autre: number };
export type LedgerSummary = {
  debut: number | null;
  fin: number | null;
  temps: LedgerTime;
  maps: LedgerMapRun[];
  mapsParHeure: number | null;
  dureeMoyenneMap: number | null;
  zoneCourante: string | null;
  typeZoneCourante: ZoneKind | null;
};
/** Stratégie étiquetée par le joueur (apps/desktop/src-tauri/src/ledger_store.rs). */
export type LedgerStrategy = {
  waystoneTier: number | null;
  tablets: string[];
  mechanics: string[];
  note: string;
};
export type LedgerStatus = {
  active: boolean;
  startedAt: string | null;
  strategy: LedgerStrategy;
  summary: LedgerSummary | null;
  lastSavedId: number | null;
  storageError: string | null;
};
export type LedgerSessionRow = {
  id: number;
  startedAt: string;
  endedAt: string;
  strategy: LedgerStrategy;
  maps: number;
  totalSecs: number;
  mapSecs: number;
  mapsPerHour: number | null;
  avgMapSecs: number | null;
};
export type LedgerSession = {
  id: number;
  startedAt: string;
  endedAt: string;
  strategy: LedgerStrategy;
  summary: LedgerSummary;
};

/** Personnage importé d'un code Path of Building (apps/desktop/src-tauri/src/character.rs). */
export type GemInfo = { name: string; level: number; quality: number };
export type CharacterInfo = {
  summary: CharacterSummary;
  importedAt: string;
  skill: GemInfo | null;
  supports: string[];
  skillSupported: boolean;
  equippedSlots: string[];
  passives: number;
  treeVersion: string | null;
};

/** Forge (crates/poe2-core/src/forge/atelier.rs, prix.rs, ninja.rs, solveur/plan.rs). */
export type ForgeSlot = "prefix" | "suffix";
export type ForgeRarity = "normal" | "magic" | "rare";
export type ForgeAttribution = { source: string; licence: string; dateReleve: string; repoeVersion: string };
export type ForgeBase = { id: string; nom: string; classe: string; niveauDrop: number; page: string };
export type ForgeTier = { tier: number; niveau: number; texte: string; poids: number };
export type ForgeAffix = {
  group: number;
  family: number;
  slot: ForgeSlot;
  libelle: string;
  part: number;
  /** Posé seulement par une Essence ou un Alloy à ce niveau d'objet. */
  essence: boolean;
  paliers: ForgeTier[];
};
export type ForgeAffixes = {
  baseId: string;
  ilvl: number;
  page: string;
  affixes: ForgeAffix[];
  sansPoids: { id: string; slot: ForgeSlot; niveau: number }[];
  attribution: ForgeAttribution;
};
export type ForgeCatalogueEntry = { id: string; nom: string; genre: "base" | "monnaie" | "omen" | "essence" };
export type ForgePriceSource = "joueur" | "poeNinja" | "indicatif";
export type ForgeMarket = {
  ligue: string;
  releveA: number;
  prix: Record<string, number>;
  dates: Record<string, number>;
  manquants: string[];
};
export type ForgePrices = {
  corrections: Record<string, number>;
  marche: ForgeMarket | null;
  prix: Record<string, number>;
  sources: Record<string, ForgePriceSource>;
};
export type ForgeWanted = { group: number; family: number; maxTier: number };
export type ForgePlanRequest = { baseId: string; ilvl: number; voulus: ForgeWanted[]; exclues: string[] };
export type ForgeItemSummary = {
  rarity: ForgeRarity;
  heldWanted: number[];
  blockedWanted: number[];
  fracturedWanted: number | null;
  badPrefixes: number;
  badSuffixes: number;
};
export type ForgeActionInfo = { id: string; label: string; unitCost: number; isAbandon: boolean };
export type ForgeRepeat = { selfLoopProbability: number; expectedAttempts: number; p90Attempts: number };
export type ForgeBranch = {
  id: string;
  kind: "success" | "failure";
  label: string;
  probability: number;
  to: string;
  loopback: boolean;
  extraCost: number;
};
export type ForgeActionNode = {
  kind: "action";
  id: string;
  stateKey: string;
  state: ForgeItemSummary;
  expectedVisits: number;
  costToGo: number;
  action: ForgeActionInfo;
  repeat: ForgeRepeat | null;
  branches: ForgeBranch[];
  mergedMinorProbability: number;
  mergedMinorCount: number;
};
export type ForgeTerminalNode = {
  kind: "terminal";
  id: string;
  stateKey: string;
  state: ForgeItemSummary;
  expectedVisits: number;
  costToGo: number;
  result: string;
  note: string | null;
};
export type ForgeShoppingLine = { id: string; label: string; expectedCount: number; unitCost: number; expectedCost: number };
export type ForgeVerify = {
  trials: number;
  meanCost: number;
  medianCost: number;
  p90Cost: number;
  p99Cost: number;
  meanSteps: number;
  meanAbandons: number;
  censored: number;
};
export type ForgePlan = {
  baseId: string;
  ilvl: number;
  goal: { label: string; slot: ForgeSlot; group: number; familyId: number; maxTier: number }[];
  rootId: string;
  nodes: Record<string, ForgeActionNode | ForgeTerminalNode>;
  expectedCost: number;
  baseCost: number;
  shopping: ForgeShoppingLine[];
  solver: { states: number; sweeps: number; converged: boolean; millis: number; costFromVisits: number };
  mc: ForgeVerify | null;
  pricesSource: string;
};
export type ForgePlanResult = {
  plan: ForgePlan;
  complet: boolean;
  sansPoids: number;
  prix: { id: string; prix: number; source: ForgePriceSource }[];
  ligue: string | null;
  attribution: ForgeAttribution;
};

export type ForgeAdviceOutcome = { label: string; probability: number; kind: "success" | "failure"; costToGo: number | null };
export type ForgeAdvice = {
  stateKey: string;
  state: ForgeItemSummary;
  goalReached: boolean;
  costToGo: number | null;
  action: ForgeActionInfo | null;
  repeat: ForgeRepeat | null;
  outcomes: ForgeAdviceOutcome[];
};
/** Objet copié en jeu suivi sur le plan Forge actif (crates/poe2-core/src/forge/suivi.rs). */
export type ForgeReadMod = {
  texte: string;
  slot: ForgeSlot;
  voulu: number | null;
  tierTropBas: boolean;
  reconnu: boolean;
  fracture: boolean;
};
export type ForgeTip = {
  baseNom: string;
  labels: string[];
  coutPlan: number;
  mods: ForgeReadMod[];
  ilvlObjet: number | null;
  ilvlPlan: number;
  conseil: ForgeAdvice;
};
export type ForgeTracking = { baseNom: string; ilvl: number; labels: string[] };
/** Plans calculés (crates/poe2-core/src/forge/historique.rs). */
export type ForgeHistoryEntry = {
  id: number;
  calculeA: number;
  demande: ForgePlanRequest;
  baseNom: string;
  labels: string[];
  cout: number;
};
export type ForgeHistory = { entrees: ForgeHistoryEntry[]; suivi: number | null };

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function isAppError(value: unknown): value is AppError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

export const commands = {
  parseClipboardItem: (text: string) => invoke<ParsedItem>("parse_clipboard_item", { text }),
  getOverlayState: () => invoke<OverlayState>("get_overlay_state"),
  hideOverlay: () => invoke<void>("hide_overlay"),
  fitOverlay: (width: number, height: number) => invoke<void>("fit_overlay", { width, height }),
  openMainWindow: () => invoke<void>("open_main_window"),
  checkForUpdate: () => invoke<UpdateInfo | null>("check_for_update"),
  installUpdate: () => invoke<void>("install_update"),
  getSettings: () => invoke<Settings>("get_settings"),
  setUpdateChannel: (beta: boolean) => invoke<Settings>("set_update_channel", { beta }),
  getClientLogStatus: () => invoke<ClientLogStatus>("get_client_log_status"),
  setClientLogPath: (path: string | null) => invoke<Settings>("set_client_log_path", { path }),
  getMeasureStatus: () => invoke<MeasureStatus>("get_measure_status"),
  startMeasureSession: () => invoke<MeasureStatus>("start_measure_session"),
  stopMeasureSession: () => invoke<MeasureStatus>("stop_measure_session"),
  addMeasureNote: (text: string) => invoke<MeasureStatus>("add_measure_note", { text }),
  copyMeasureExport: () => invoke<void>("copy_measure_export"),
  getLedgerStatus: () => invoke<LedgerStatus>("get_ledger_status"),
  startLedgerSession: () => invoke<LedgerStatus>("start_ledger_session"),
  stopLedgerSession: () => invoke<LedgerStatus>("stop_ledger_session"),
  setLedgerStrategy: (id: number | null, strategy: LedgerStrategy) =>
    invoke<LedgerStatus>("set_ledger_strategy", { id, strategy }),
  listLedgerSessions: (limit: number) => invoke<LedgerSessionRow[]>("list_ledger_sessions", { limit }),
  getLedgerSession: (id: number) => invoke<LedgerSession>("get_ledger_session", { id }),
  deleteLedgerSession: (id: number) => invoke<void>("delete_ledger_session", { id }),
  getCharacter: () => invoke<CharacterInfo | null>("get_character"),
  importPobCode: (code: string) => invoke<CharacterInfo>("import_pob_code", { code }),
  clearCharacter: () => invoke<void>("clear_character"),
  getForgeBases: () => invoke<ForgeBase[]>("get_forge_bases"),
  getForgeAffixes: (baseId: string, ilvl: number) => invoke<ForgeAffixes>("get_forge_affixes", { baseId, ilvl }),
  getForgeCatalogue: () => invoke<ForgeCatalogueEntry[]>("get_forge_catalogue"),
  planForgeCraft: (demande: ForgePlanRequest) => invoke<ForgePlanResult>("plan_forge_craft", { demande }),
  getForgeAdvice: () => invoke<ForgeTip | null>("get_forge_advice"),
  getForgeTracking: () => invoke<ForgeTracking | null>("get_forge_tracking"),
  stopForgeTracking: () => invoke<void>("stop_forge_tracking"),
  getForgeHistory: () => invoke<ForgeHistory>("get_forge_history"),
  deleteForgeHistoryEntry: (id: number) => invoke<ForgeHistory>("delete_forge_history_entry", { id }),
  getForgePrices: () => invoke<ForgePrices>("get_forge_prices"),
  setForgePrices: (prix: Record<string, number>) => invoke<ForgePrices>("set_forge_prices", { prix }),
  resetForgePrices: () => invoke<ForgePrices>("reset_forge_prices"),
  refreshForgePrices: (ligue: string | null) => invoke<ForgePrices>("refresh_forge_prices", { ligue }),
  // Commandes du chantier Architecture / moteur, branchées dès leur fusion.
  evaluateItem: (item: ParsedItem) => invoke<ItemDelta>("evaluate_item", { item }),
  estimatePrice: (item: ParsedItem) => invoke<PriceEstimate>("estimate_price", { item }),
};

export const on = {
  /** Contrat : la charge est l'objet déjà parsé par le cœur Rust. */
  itemCopied: (cb: (p: ParsedItem) => void): Promise<UnlistenFn> =>
    listen<ParsedItem>(EVENTS.itemCopied, (e) => cb(e.payload)),
  zoneChanged: (cb: (p: ZoneChanged) => void): Promise<UnlistenFn> => listen<ZoneChanged>(EVENTS.zoneChanged, (e) => cb(e.payload)),
  itemUnrecognized: (cb: (p: AppError) => void): Promise<UnlistenFn> =>
    listen<AppError>(EVENTS.itemUnrecognized, (e) => cb(e.payload)),
  overlayHidden: (cb: () => void): Promise<UnlistenFn> => listen(EVENTS.overlayHidden, () => cb()),
  upgradeSearchRequested: (cb: () => void): Promise<UnlistenFn> => listen(EVENTS.upgradeSearchRequested, () => cb()),
  updateAvailable: (cb: (p: UpdateInfo) => void): Promise<UnlistenFn> =>
    listen<UpdateInfo>(EVENTS.updateAvailable, (e) => cb(e.payload)),
  updateProgress: (cb: (p: UpdateProgress) => void): Promise<UnlistenFn> =>
    listen<UpdateProgress>(EVENTS.updateProgress, (e) => cb(e.payload)),
  updateCheckRequested: (cb: () => void): Promise<UnlistenFn> => listen(EVENTS.updateCheckRequested, () => cb()),
  clientLogStatus: (cb: (p: ClientLogStatus) => void): Promise<UnlistenFn> =>
    listen<ClientLogStatus>(EVENTS.clientLogStatus, (e) => cb(e.payload)),
  measureSession: (cb: (p: MeasureStatus) => void): Promise<UnlistenFn> =>
    listen<MeasureStatus>(EVENTS.measureSession, (e) => cb(e.payload)),
  ledgerSession: (cb: (p: LedgerStatus) => void): Promise<UnlistenFn> =>
    listen<LedgerStatus>(EVENTS.ledgerSession, (e) => cb(e.payload)),
  characterChanged: (cb: (p: CharacterInfo | null) => void): Promise<UnlistenFn> =>
    listen<CharacterInfo | null>(EVENTS.characterChanged, (e) => cb(e.payload)),
};
