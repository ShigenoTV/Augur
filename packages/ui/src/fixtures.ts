/**
 * Données fictives pour les maquettes, le bac à sable et les tests.
 * Les objets et chiffres sont inventés : ils illustrent les états de
 * l'interface, pas un calcul réel du moteur.
 */
import type {
  CharacterSummary,
  CopilotView,
  ItemDelta,
  ItemMod,
  ModKind,
  ParsedItem,
  PriceEstimate,
  RateLimitStatus,
  UpdateInfo,
} from "./types";

export const NOW = new Date("2026-10-31T15:00:00Z");
const minutesAgo = (m: number) => new Date(NOW.getTime() - m * 60_000).toISOString();

const m = (text: string, kind: ModKind = "explicit", tier: number | null = null): ItemMod => ({
  text,
  kind,
  affix: null,
  tier,
  statIds: [],
  values: [],
});

const base = { rawText: "", advanced: false, unidentified: false, dataPatch: "0.4.0", quality: null } as const;

export const rareHelmet: ParsedItem = {
  ...base,
  advanced: true,
  itemClass: "Helmets",
  rarity: "rare",
  name: "Dread Crown",
  baseType: "Expert Visored Helm",
  itemLevel: 79,
  quality: 20,
  corrupted: false,
  mods: [
    m("+92 to maximum Life", "explicit", 2),
    m("+38% to Fire Resistance", "explicit", 3),
    m("+24% to Lightning Resistance", "explicit", 5),
    m("+31 to Strength", "explicit", 3),
    m("61% increased Armour and Evasion", "explicit", 4),
    m("+2 to Level of all Minion Skills", "desecrated"),
  ].map((mod, i) => ({ ...mod, affix: i === 0 || i === 4 ? "prefix" : "suffix" })),
};

export const rareBow: ParsedItem = {
  ...base,
  itemClass: "Bows",
  rarity: "rare",
  name: "Storm Thirst",
  baseType: "Advanced Dualstring Bow",
  itemLevel: 81,
  corrupted: false,
  mods: [
    m("Bow Attacks fire an additional Arrow", "implicit"),
    m("168% increased Physical Damage"),
    m("Adds 4 to 131 Lightning Damage"),
    m("+3 to Level of all Projectile Skills"),
    m("+21% to Critical Damage Bonus"),
  ],
};

export const uniqueGloves: ParsedItem = {
  ...base,
  itemClass: "Gloves",
  rarity: "unique",
  name: "Northpaw",
  baseType: "Ringmail Gauntlets",
  itemLevel: 72,
  corrupted: true,
  mods: [m("+15% to Cold Resistance", "enchant"), m("Adds 3 to 7 Physical Damage to Attacks"), m("+40 to Dexterity")],
};

export const character: CharacterSummary = {
  name: "VeyraStorm",
  class: "Deadeye",
  level: 87,
  league: "Standard",
  source: "oauth",
};

export const upgradeDelta: ItemDelta = {
  slot: "weapon1",
  equippedName: "Gale Song, Dualstring Bow",
  weaponSet: 1,
  support: "verified",
  dps: { before: 48_210, after: 61_870 },
  ehp: { before: 9_840, after: 9_840 },
  resistances: [],
  spirit: null,
  mechanics: [{ mechanic: "shock-buildup", confidence: "confirmed" }],
  computeMs: 38,
};

export const tradeoffDelta: ItemDelta = {
  slot: "helmet",
  equippedName: "Hate Visage, Expert Visored Helm",
  weaponSet: null,
  support: "verified",
  dps: { before: 48_210, after: 51_900 },
  ehp: { before: 9_840, after: 9_120 },
  resistances: [
    { element: "fire", before: 75, after: 75, cap: 75 },
    { element: "cold", before: 75, after: 61, cap: 75 },
  ],
  spirit: { before: 30, after: 30 },
  mechanics: [],
  computeMs: 41,
};

export const approximateDelta: ItemDelta = {
  slot: "weapon1",
  equippedName: "Gale Song, Dualstring Bow",
  weaponSet: 2,
  support: "approximate",
  dps: { before: 48_210, after: 55_400 },
  ehp: { before: 9_840, after: 9_840 },
  resistances: [],
  spirit: null,
  mechanics: [
    { mechanic: "electrocute-buildup", confidence: "assumed" },
    { mechanic: "weapon-set-passives", confidence: "measured" },
  ],
  computeMs: 52,
};

export const downgradeDelta: ItemDelta = {
  slot: "gloves",
  equippedName: "Grim Grip, Ringmail Gauntlets",
  weaponSet: null,
  support: "verified",
  dps: { before: 48_210, after: 44_050 },
  ehp: { before: 9_840, after: 9_610 },
  resistances: [{ element: "cold", before: 75, after: 75, cap: 75 }],
  spirit: null,
  mechanics: [],
  computeMs: 35,
};

export const unsupportedDelta: ItemDelta = { ...upgradeDelta, support: "unsupported" };

export const tradePrice: PriceEstimate = {
  currency: "exalted",
  low: 11,
  median: 14,
  high: 18,
  sampleSize: 23,
  source: "trade",
  league: "Standard",
  observedAt: minutesAgo(2),
  stale: false,
};

export const modelPrice: PriceEstimate = {
  currency: "divine",
  low: 1,
  median: 1.5,
  high: 2.5,
  sampleSize: 0,
  source: "model",
  league: "Standard",
  observedAt: minutesAgo(11),
  stale: false,
};

export const stalePrice: PriceEstimate = {
  currency: "exalted",
  low: 7,
  median: 9,
  high: 12,
  sampleSize: 8,
  source: "trade",
  league: "Standard",
  observedAt: minutesAgo(47),
  stale: true,
};

export const queued: RateLimitStatus = {
  policies: [
    {
      policy: "trade-search-request-limit",
      windows: [{ rule: "ip", hits: 7, maxHits: 8, usableHits: 7, periodS: 10 }],
      blockedForMs: null,
      queued: 2,
    },
  ],
};

export const blocked: RateLimitStatus = {
  policies: [
    {
      policy: "trade-search-request-limit",
      windows: [{ rule: "ip", hits: 8, maxHits: 8, usableHits: 7, periodS: 10 }],
      blockedForMs: 58_000,
      queued: 3,
    },
  ],
};

/** Tous les états de l'overlay, dans l'ordre de la spec. */
export const overlayStates: Array<{ id: string; title: string; state: CopilotView }> = [
  {
    id: "upgrade",
    title: "Amélioration vérifiée, prix trade",
    state: { status: "ready", item: rareBow, delta: upgradeDelta, price: tradePrice, pricing: "done", character },
  },
  {
    id: "tradeoff",
    title: "Compromis, résistance sous le plafond",
    state: { status: "ready", item: rareHelmet, delta: tradeoffDelta, price: modelPrice, pricing: "done", character },
  },
  {
    id: "downgrade",
    title: "Régression, prix ancien",
    state: { status: "ready", item: uniqueGloves, delta: downgradeDelta, price: stalePrice, pricing: "done", character },
  },
  {
    id: "approximate",
    title: "Calcul approximatif",
    state: { status: "ready", item: rareBow, delta: approximateDelta, price: tradePrice, pricing: "done", character },
  },
  {
    id: "pricing-queued",
    title: "Prix en file d'attente (rate limit local)",
    state: { status: "ready", item: rareBow, delta: upgradeDelta, pricing: "pending", rateLimit: queued, character },
  },
  {
    id: "rate-limited",
    title: "Limite GGG atteinte (Retry-After)",
    state: { status: "ready", item: rareHelmet, delta: tradeoffDelta, pricing: "pending", rateLimit: blocked, character },
  },
  {
    id: "unsupported",
    title: "Archétype non supporté",
    state: { status: "ready", item: rareBow, delta: unsupportedDelta, price: tradePrice, pricing: "done", character },
  },
  {
    id: "evaluating",
    title: "Analyse en cours (au-delà de 150 ms)",
    state: { status: "evaluating", item: rareHelmet },
  },
  {
    id: "no-character",
    title: "Aucun personnage importé",
    state: { status: "noCharacter", item: rareHelmet },
  },
  {
    id: "unrecognized",
    title: "Objet non reconnu",
    state: {
      status: "unrecognized",
      error: {
        code: "parseFailed",
        message:
          "Le texte copié ne ressemble pas à un objet de Path of Exile 2. Vérifie que le jeu est en anglais ou en français.",
        retryAfterMs: null,
      },
    },
  },
  {
    id: "disabled",
    title: "Fonction coupée à distance (kill switch)",
    state: {
      status: "disabled",
      reason:
        "Le dernier patch a changé le format des objets copiés. Une mise à jour des données arrive : rien à faire de ton côté.",
    },
  },
];

export const updateInfo: UpdateInfo = {
  version: "0.2.0",
  currentVersion: "0.1.0",
  notes: "Overlay : alerte quand une résistance passe sous le plafond.\nParseur : prise en charge du format avancé (Ctrl+Alt+C).",
  publishedAt: NOW.toISOString(),
  channel: "stable",
};
