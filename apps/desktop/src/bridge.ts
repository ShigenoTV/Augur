/**
 * Pont typé vers le cœur Rust : commandes et événements du contrat.
 * Hors Tauri (navigateur, tests), `isTauri()` est faux et l'interface
 * bascule sur les données fictives de @augur/ui.
 */
import type {
  AppError,
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
};
