/**
 * Raccourcis de l'overlay : saisie au clavier dans Réglages et affichage en français.
 * Le format enregistré (« Alt+U », « Ctrl+Shift+F2 », « Escape ») est celui que lit le cœur Rust
 * (src-tauri/src/shortcuts.rs), qui fait aussi les vérifications.
 */
import type { ShortcutSettings } from "../bridge";

export const SHORTCUTS_PAR_DEFAUT: ShortcutSettings = { closeOverlay: "Escape", upgradeSearch: "Alt+U" };

type Touche = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

const MODIFICATEURS = new Set(["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight"]);

/** Raccourci tapé, ou null tant que seule une touche de modification est enfoncée. */
export function lireTouche(e: Touche): string | null {
  if (!e.code || MODIFICATEURS.has(e.code)) return null;
  const touche = e.code.replace(/^Key([A-Z])$/, "$1").replace(/^Digit(\d)$/, "$1");
  return [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super", touche]
    .filter(Boolean)
    .join("+");
}

const NOMS: Record<string, string> = { ESCAPE: "Échap", ESC: "Échap", SHIFT: "Maj", CONTROL: "Ctrl", CTRL: "Ctrl", ALT: "Alt", SUPER: "Win" };

/** Libellé affiché : « Escape » → « Échap », « ctrl+shift+KeyU » → « Ctrl+Maj+U ». */
export function afficherRaccourci(s: string): string {
  return s
    .split("+")
    .map((t) => t.trim())
    .filter(Boolean)
    .map((t) => NOMS[t.toUpperCase()] ?? t.replace(/^Key([A-Z])$/i, "$1").replace(/^Digit(\d)$/i, "$1").toUpperCase())
    .join("+");
}
