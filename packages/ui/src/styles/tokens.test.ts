import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { accent, ink, mod, overlayOpacity, rarity, signal, text, uniqueText } from "./tokens";

// Vitest tourne depuis la racine du paquet.
const css = readFileSync(resolve(process.cwd(), "src/styles/tokens.css"), "utf8");

function cssVar(name: string): string | undefined {
  return css.match(new RegExp(`--${name}:\\s*([^;]+);`))?.[1]?.trim();
}

function luminance(hex: string): number {
  const n = Number.parseInt(hex.slice(1), 16);
  const channels = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * channels[0]! + 0.7152 * channels[1]! + 0.0722 * channels[2]!;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** Fond réel de l'overlay posé sur une zone de jeu très claire (blanc). */
function overlayOnWhite(): string {
  const base = Number.parseInt(ink[950].slice(1), 16);
  const mix = (c: number) => Math.round(c * overlayOpacity + 255 * (1 - overlayOpacity));
  const r = mix((base >> 16) & 255);
  const g = mix((base >> 8) & 255);
  const b = mix(base & 255);
  return `#${[r, g, b].map((c) => c.toString(16).padStart(2, "0")).join("")}`;
}

describe("tokens.css reflète tokens.ts", () => {
  const groups: Array<[string, Record<string, string>]> = [
    ["color-ink", Object.fromEntries(Object.entries(ink).map(([k, v]) => [k, v]))],
    ["color-text", text],
    ["color-rarity", rarity],
    ["color-mod", mod],
    ["color-signal", signal],
  ];
  for (const [prefix, values] of groups) {
    for (const [key, value] of Object.entries(values)) {
      it(`--${prefix}-${key}`, () => {
        expect(cssVar(`${prefix}-${key}`)).toBe(value);
      });
    }
  }
  it("--color-accent", () => {
    expect(cssVar("color-accent")).toBe(accent.DEFAULT);
    expect(cssVar("color-accent-strong")).toBe(accent.strong);
  });
  it("opacité de l'overlay", () => {
    expect(cssVar("augur-overlay-opacity")).toBe(String(overlayOpacity));
  });
  it("--color-rarity-unique-text", () => {
    expect(cssVar("color-rarity-unique-text")).toBe(uniqueText);
  });
});

describe("contrastes (WCAG AA, 4,5:1 pour le texte)", () => {
  const backgrounds = { "ink-950": ink[950], "ink-800": ink[800], "overlay sur zone blanche": overlayOnWhite() };
  const { unique: _gameUnique, ...rarityReadable } = rarity;
  const readable = { ...text, ...signal, accent: accent.DEFAULT, ...rarityReadable, unique: uniqueText, enchant: mod.enchant, fractured: mod.fractured, desecrated: mod.desecrated };

  for (const [bgName, bg] of Object.entries(backgrounds)) {
    for (const [fgName, fg] of Object.entries(readable)) {
      it(`${fgName} sur ${bgName}`, () => {
        expect(contrast(fg, bg)).toBeGreaterThanOrEqual(4.5);
      });
    }
  }

  it("orange unique du jeu : réservé aux bordures (3:1 non textuel)", () => {
    expect(contrast(rarity.unique, ink[950])).toBeGreaterThanOrEqual(3);
  });

  it("rouge « corrompu » du jeu : au moins 3:1 (texte gras uniquement)", () => {
    expect(contrast(mod.corrupted, ink[950])).toBeGreaterThanOrEqual(3);
  });
});
