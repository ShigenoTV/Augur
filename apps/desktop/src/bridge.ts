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
  zoneChanged: "augur://zone-changed",
  overlayHidden: "augur://overlay-hidden",
  upgradeSearchRequested: "augur://upgrade-search-requested",
  rateLimitChanged: "augur://rate-limit-changed",
  featureFlagsChanged: "augur://feature-flags-changed",
  updateAvailable: "augur://update-available",
  updateProgress: "augur://update-progress",
  updateCheckRequested: "augur://update-check-requested",
} as const;

export type Settings = { betaChannel: boolean };

export type ItemCopied = { text: string; capturedAtMs: number };

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
  // Commandes du chantier Architecture / moteur, branchées dès leur fusion.
  evaluateItem: (item: ParsedItem) => invoke<ItemDelta>("evaluate_item", { item }),
  estimatePrice: (item: ParsedItem) => invoke<PriceEstimate>("estimate_price", { item }),
};

export const on = {
  itemCopied: (cb: (p: ItemCopied) => void): Promise<UnlistenFn> => listen<ItemCopied>(EVENTS.itemCopied, (e) => cb(e.payload)),
  zoneChanged: (cb: (p: ZoneChanged) => void): Promise<UnlistenFn> => listen<ZoneChanged>(EVENTS.zoneChanged, (e) => cb(e.payload)),
  overlayHidden: (cb: () => void): Promise<UnlistenFn> => listen(EVENTS.overlayHidden, () => cb()),
  upgradeSearchRequested: (cb: () => void): Promise<UnlistenFn> => listen(EVENTS.upgradeSearchRequested, () => cb()),
  updateAvailable: (cb: (p: UpdateInfo) => void): Promise<UnlistenFn> =>
    listen<UpdateInfo>(EVENTS.updateAvailable, (e) => cb(e.payload)),
  updateProgress: (cb: (p: UpdateProgress) => void): Promise<UnlistenFn> =>
    listen<UpdateProgress>(EVENTS.updateProgress, (e) => cb(e.payload)),
  updateCheckRequested: (cb: () => void): Promise<UnlistenFn> => listen(EVENTS.updateCheckRequested, () => cb()),
};
