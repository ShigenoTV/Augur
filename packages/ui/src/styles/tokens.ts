/**
 * Tokens du design system Augur « dark ».
 *
 * Source de vérité en TypeScript : `tokens.css` doit déclarer exactement les
 * mêmes valeurs (un test vérifie la synchronisation et les contrastes).
 * Les couleurs de rareté et de mods reprennent les conventions du jeu ;
 * les valeurs marquées « à vérifier » doivent être confirmées sur des
 * captures réelles de PoE 2 (tâche UX S2).
 */

/** Surfaces : du plus profond (fond de l'overlay) au plus clair (bordures). */
export const ink = {
  950: "#0b0a09",
  900: "#14120f",
  850: "#1b1814",
  800: "#24201b",
  700: "#332d26",
  600: "#4a4238",
} as const;

/** Textes : hiérarchie en trois niveaux, tous lisibles sur ink-950 à ink-800. */
export const text = {
  primary: "#ece6da",
  muted: "#b9b0a1",
  subtle: "#958b7c",
} as const;

/** Accent de marque (or d'augure), réservé aux actions et au focus. */
export const accent = {
  DEFAULT: "#e0b061",
  strong: "#f0c77e",
  ink: "#1b1406",
} as const;

/** Couleurs de rareté, conventions du jeu. */
export const rarity = {
  normal: "#c8c8c8",
  magic: "#8888ff",
  rare: "#ffff77",
  unique: "#af6025",
  gem: "#1ba29b",
  currency: "#aa9e82",
} as const;

/**
 * Variante lisible de l'orange unique pour le TEXTE : la valeur du jeu
 * (#af6025) n'atteint que 3,5:1 sur ink-800, sous le seuil WCAG AA.
 * La couleur du jeu reste utilisée pour les bordures et pictogrammes.
 */
export const uniqueText = "#d07a3a";

/** Couleurs de mods, conventions du jeu. */
export const mod = {
  implicit: "#8888ff",
  explicit: "#8888ff",
  rune: "#8888ff", // à vérifier sur capture PoE 2
  enchant: "#b8daf2",
  crafted: "#b8daf2",
  fractured: "#a29162",
  desecrated: "#c7a0e8", // à vérifier sur capture PoE 2
  corrupted: "#d20000",
} as const;

/**
 * Sémantique du verdict. Le vert et le rouge ne portent jamais seuls
 * l'information : chaque delta a aussi un signe et une flèche (▲ ▼ ●).
 */
export const signal = {
  gain: "#55d68a",
  loss: "#ff7a6b",
  neutral: "#b9b0a1",
  warn: "#f2b84b",
  info: "#7cc4ff",
} as const;

/** Opacité du fond de l'overlay : assez opaque pour rester lisible sur une zone claire. */
export const overlayOpacity = 0.94;

export const tokens = { ink, text, accent, rarity, uniqueText, mod, signal } as const;
export type RarityToken = keyof typeof rarity;
export type ModToken = keyof typeof mod;
