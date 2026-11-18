import type { CopilotView, ItemDelta, OverlayState, ParsedItem, PriceEstimate, PricingState } from "@augur/ui";
import { isAppError } from "../bridge";

/** Délai avant d'afficher l'état « analyse » : en dessous, pas de clignotement. */
export const EVALUATING_DELAY_MS = 150;

export interface CopilotDeps {
  overlayState: () => Promise<OverlayState>;
  evaluate: (item: ParsedItem) => Promise<ItemDelta>;
  price: (item: ParsedItem) => Promise<PriceEstimate>;
}

/** Objets sans comparaison au build (monnaies, essences, gemmes…) : seul le prix compte. */
export function priceOnly(item: ParsedItem): boolean {
  return item.rarity === "currency" || item.rarity === "gem" || item.rarity === "other";
}

/** Uniques et rares : cote trade à la demande (raccourci du joueur). */
export function tradable(item: ParsedItem): boolean {
  return item.rarity === "unique" || item.rarity === "rare";
}

type Pricing = { price?: PriceEstimate; pricing: PricingState; priceNote?: string };

/** Prix estimé (cote du marché), ou pourquoi il n'y en a pas. */
async function estimate(item: ParsedItem, deps: CopilotDeps): Promise<Pricing> {
  try {
    return { price: await deps.price(item), pricing: "done" };
  } catch (err) {
    if (!isAppError(err)) return { pricing: "unavailable" };
    if (err.code === "featureDisabled") return { pricing: "disabled" };
    // Pas de cote automatique : les uniques et rares se cotent sur le trade (message du raccourci).
    if (err.code === "unsupported") return { pricing: "unpriced", priceNote: tradable(item) ? undefined : err.message };
    return { pricing: "unavailable", priceNote: err.message };
  }
}

/**
 * Enchaînement d'un Ctrl+C sur un objet déjà parsé par le cœur Rust :
 * personnage, évaluation, prix. `emit` est appelé à chaque étape visible ;
 * le résultat final est renvoyé. Le prix ne dépend pas du personnage.
 */
export async function runCopilot(
  item: ParsedItem,
  deps: CopilotDeps,
  emit: (v: CopilotView) => void,
): Promise<CopilotView> {
  if (priceOnly(item)) {
    emit({ status: "ready", item, pricing: "pending", priceOnly: true });
    const view: CopilotView = { status: "ready", item, priceOnly: true, ...(await estimate(item, deps)) };
    emit(view);
    return view;
  }

  const state = await deps.overlayState().catch(() => null);
  const noCharacter = async (): Promise<CopilotView> => {
    emit({ status: "noCharacter", item, pricing: "pending" });
    const view: CopilotView = { status: "noCharacter", item, ...(await estimate(item, deps)) };
    emit(view);
    return view;
  };
  if (!state?.character) return noCharacter();

  let delta: ItemDelta | undefined;
  try {
    delta = await deps.evaluate(item);
  } catch (err) {
    if (isAppError(err) && err.code === "noCharacter") return noCharacter();
    // featureDisabled ou commande absente : on garde l'objet et le prix.
  }

  const ready: CopilotView = { status: "ready", item, delta, pricing: "pending", character: state.character };
  emit(ready);
  const done: CopilotView = { ...ready, ...(await estimate(item, deps)) };
  emit(done);
  return done;
}
