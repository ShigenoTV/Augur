import type { CopilotView, ItemDelta, OverlayState, ParsedItem, PriceEstimate } from "@augur/ui";
import { isAppError } from "../bridge";

/** Délai avant d'afficher l'état « analyse » : en dessous, pas de clignotement. */
export const EVALUATING_DELAY_MS = 150;

export interface CopilotDeps {
  overlayState: () => Promise<OverlayState>;
  evaluate: (item: ParsedItem) => Promise<ItemDelta>;
  price: (item: ParsedItem) => Promise<PriceEstimate>;
}

/**
 * Enchaînement d'un Ctrl+C sur un objet déjà parsé par le cœur Rust :
 * personnage, évaluation, prix. `emit` est appelé à chaque étape visible ;
 * le résultat final est renvoyé.
 */
export async function runCopilot(
  item: ParsedItem,
  deps: CopilotDeps,
  emit: (v: CopilotView) => void,
): Promise<CopilotView> {
  const state = await deps.overlayState().catch(() => null);
  if (!state?.character) {
    const view: CopilotView = { status: "noCharacter", item };
    emit(view);
    return view;
  }

  let delta: ItemDelta | undefined;
  try {
    delta = await deps.evaluate(item);
  } catch (err) {
    if (isAppError(err) && err.code === "noCharacter") {
      const view: CopilotView = { status: "noCharacter", item };
      emit(view);
      return view;
    }
    // featureDisabled ou commande absente : on garde l'objet et le prix.
  }

  const ready: CopilotView = { status: "ready", item, delta, pricing: "pending", character: state.character };
  emit(ready);

  try {
    const price = await deps.price(item);
    const done: CopilotView = { ...ready, price, pricing: "done" };
    emit(done);
    return done;
  } catch (err) {
    const pricing = isAppError(err) && err.code === "featureDisabled" ? "disabled" : "unavailable";
    const view: CopilotView = { ...ready, pricing };
    emit(view);
    return view;
  }
}
