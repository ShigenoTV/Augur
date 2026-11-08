import { describe, expect, it } from "vitest";
import { afficherRaccourci, lireTouche } from "./shortcuts";

const touche = (code: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("lireTouche", () => {
  it("compose le raccourci tapé", () => {
    expect(lireTouche(touche("KeyU", { altKey: true }))).toBe("Alt+U");
    expect(lireTouche(touche("Escape"))).toBe("Escape");
    expect(lireTouche(touche("F5", { ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+F5");
    expect(lireTouche(touche("Digit2", { altKey: true }))).toBe("Alt+2");
  });

  it("attend la touche principale", () => {
    expect(lireTouche(touche("AltLeft", { altKey: true }))).toBeNull();
    expect(lireTouche(touche("ShiftRight", { shiftKey: true }))).toBeNull();
  });
});

describe("afficherRaccourci", () => {
  it("affiche les touches en français", () => {
    expect(afficherRaccourci("Escape")).toBe("Échap");
    expect(afficherRaccourci("Alt+U")).toBe("Alt+U");
    expect(afficherRaccourci("ctrl+shift+KeyU")).toBe("Ctrl+Maj+U");
    expect(afficherRaccourci("Alt+Digit1")).toBe("Alt+1");
    expect(afficherRaccourci("F5")).toBe("F5");
  });
});
