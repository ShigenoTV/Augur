import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { NOW, overlayStates } from "../fixtures";
import { CopilotOverlay } from "./CopilotOverlay";

afterEach(cleanup);

describe("CopilotOverlay", () => {
  for (const { id, state } of overlayStates) {
    it(`rend l'état « ${id} »`, () => {
      const { container } = render(<CopilotOverlay state={state} now={NOW} />);
      expect(container.firstChild).not.toBeNull();
    });
  }

  it("déplie ce que le calcul approximatif ignore", () => {
    const s = overlayStates.find((o) => o.id === "approximate")!.state;
    render(<CopilotOverlay state={s} now={NOW} />);
    expect(screen.queryByText("Overcharge")).toBeNull();
    fireEvent.click(screen.getByText(/supports ne sont pas comptés/));
    expect(screen.getByText("Overcharge")).toBeTruthy();
  });

  it("ne rend rien quand l'overlay est masqué", () => {
    const { container } = render(<CopilotOverlay state={{ status: "hidden" }} />);
    expect(container.firstChild).toBeNull();
  });

  it("n'affiche aucun chiffre de DPS pour un archétype non supporté", () => {
    const s = overlayStates.find((o) => o.id === "unsupported")!.state;
    render(<CopilotOverlay state={s} now={NOW} />);
    expect(screen.queryByText("DPS total")).toBeNull();
    expect(screen.getByText(/pas encore pris en charge/)).toBeTruthy();
  });

  it("masque l'EHP quand il n'est pas calculé (zéro avant et après)", () => {
    const s = overlayStates.find((o) => o.id === "upgrade")!.state;
    if (s.status !== "ready" || !s.delta) throw new Error("fixture");
    const sansEhp = { ...s, delta: { ...s.delta, ehp: { before: 0, after: 0 } } };
    render(<CopilotOverlay state={sansEhp} now={NOW} />);
    expect(screen.getByText("DPS total")).toBeTruthy();
    expect(screen.queryByText("EHP")).toBeNull();
  });

  it("alerte quand une résistance passe sous le plafond", () => {
    const s = overlayStates.find((o) => o.id === "tradeoff")!.state;
    render(<CopilotOverlay state={s} now={NOW} />);
    expect(screen.getByText(/Résistance au froid sous le plafond/)).toBeTruthy();
  });

  it("masque le prix quand le kill switch coupe le pricing", () => {
    const s = overlayStates.find((o) => o.id === "upgrade")!.state;
    render(<CopilotOverlay state={s} now={NOW} flags={{ clipboardParse: true, itemEvaluation: true, tradePriceCheck: false, oauthCharacterImport: true, pobImport: true }} />);
    expect(screen.queryByText("Prix")).toBeNull();
  });

  it("affiche le prix d'une pile sans comparaison", () => {
    const s = overlayStates.find((o) => o.id === "stack-price")!.state;
    render(<CopilotOverlay state={s} now={NOW} />);
    expect(screen.getByText("Prix de la pile (20)")).toBeTruthy();
    expect(screen.getByText(/l'unité/)).toBeTruthy();
    expect(screen.queryByText("DPS total")).toBeNull();
  });

  it("propose la cote trade d'un unique sans cote automatique", () => {
    const s = overlayStates.find((o) => o.id === "unpriced")!.state;
    let coté = 0;
    render(
      <CopilotOverlay
        state={s}
        now={NOW}
        hotkeys={{ close: "Échap", upgradeSearch: "Alt+U", priceCheck: "Alt+D" }}
        onPriceCheck={() => coté++}
      />,
    );
    expect(screen.getByText(/Alt\+D pour coter sur le trade/)).toBeTruthy();
    fireEvent.click(screen.getByText("coter"));
    expect(coté).toBe(1);
  });

  it("garde le prix sans personnage importé", () => {
    const s = overlayStates.find((o) => o.id === "no-character")!.state;
    if (s.status !== "noCharacter") throw new Error("fixture");
    render(<CopilotOverlay state={{ ...s, pricing: "unpriced" }} now={NOW} />);
    expect(screen.getByText(/Pas de cote automatique/)).toBeTruthy();
  });
});
