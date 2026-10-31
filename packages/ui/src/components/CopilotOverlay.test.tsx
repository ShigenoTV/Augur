import { cleanup, render, screen } from "@testing-library/react";
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
});
