import type { Element, PriceSource } from "../types";

/** Libellés français des emplacements (identifiants du contrat). */
const slots: Record<string, string> = {
  weapon1: "Arme",
  weapon2: "Arme secondaire",
  offhand1: "Main gauche",
  offhand2: "Main gauche (set II)",
  helmet: "Casque",
  bodyArmour: "Armure",
  gloves: "Gants",
  boots: "Bottes",
  belt: "Ceinture",
  amulet: "Amulette",
  ring1: "Anneau gauche",
  ring2: "Anneau droit",
  flask: "Flacon",
  charm: "Charme",
};

export function slotLabel(slot: string): string {
  return slots[slot] ?? slot;
}

const elements: Record<Element, string> = {
  fire: "Résistance au feu",
  cold: "Résistance au froid",
  lightning: "Résistance à la foudre",
  chaos: "Résistance au chaos",
};

export function elementLabel(element: Element): string {
  return elements[element];
}

/** Fiches mécaniques connues ; sinon l'identifiant est rendu lisible. */
const mechanics: Record<string, string> = {
  "electrocute-buildup": "Electrocute : seuil de buildup",
  "shock-buildup": "Shock : buildup",
  "ignite-buildup": "Ignite : buildup",
  "freeze-buildup": "Freeze : buildup",
  "heavy-stun": "Heavy Stun",
  "weapon-set-passives": "Set d'armes : bonus passifs du set",
  "spirit-reservation": "Réservation de Spirit",
  "damage-order": "Ordre des opérations des dégâts",
};

export function mechanicLabel(id: string): string {
  const known = mechanics[id];
  if (known) return known.replace(" : ", " : ");
  const words = id.replace(/[-_]/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

const currencies: Record<string, string> = {
  exalted: "ex",
  divine: "div",
  chaos: "chaos",
};

export function currencyShort(currency: string): string {
  return currencies[currency] ?? currency;
}

const sources: Record<PriceSource, string> = {
  trade: "trade",
  currencyExchange: "échange de devises",
  model: "estimation",
};

export function priceSourceLabel(source: PriceSource): string {
  return sources[source];
}
