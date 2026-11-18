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

  for (const { id, state } of overlayStates) {
    it(`rend l'état « ${id} » en compact`, () => {
      const { container } = render(<CopilotOverlay state={state} now={NOW} compact />);
      if (state.status !== "hidden") expect(container.firstChild).not.toBeNull();
    });
  }

  it("compact : verdict et alertes seulement, « détails » déplie la vue complète", () => {
    const s = overlayStates.find((o) => o.id === "tradeoff")!.state;
    render(<CopilotOverlay state={s} now={NOW} compact />);
    expect(screen.queryByText("DPS total")).toBeNull();
    expect(screen.queryByText(/les \d+ mods/)).toBeNull();
    expect(screen.getByText(/Résistance au froid sous le plafond/)).toBeTruthy();
    fireEvent.click(screen.getByText("détails"));
    expect(screen.getByText("DPS total")).toBeTruthy();
    fireEvent.click(screen.getByText("réduire"));
    expect(screen.queryByText("DPS total")).toBeNull();
  });

  it("affiche le contrôle ajouté au pied", () => {
    const s = overlayStates.find((o) => o.id === "upgrade")!.state;
    render(<CopilotOverlay state={s} now={NOW} footerExtra={<button type="button">épingler</button>} />);
    expect(screen.getByText("épingler")).toBeTruthy();
    expect(screen.queryByText("détails")).toBeNull();
  });
});
