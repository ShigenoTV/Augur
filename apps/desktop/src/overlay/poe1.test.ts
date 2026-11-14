import { describe, expect, it } from "vitest";
import { formatCote } from "./Poe1Panel";

describe("formatCote", () => {
  it("affiche les Chaos, et les Divines au-delà d'une demie", () => {
    expect(formatCote({ chaos: 157, chaosMax: null, divine: 2, annonces: 46, liens: null })).toBe("≈ 157 c · 2 div");
    expect(formatCote({ chaos: 7.2, chaosMax: null, divine: 0.09, annonces: null, liens: null })).toBe("≈ 7,2 c");
    expect(formatCote({ chaos: 10, chaosMax: 25, divine: null, annonces: 3, liens: null })).toBe("≈ 10 à 25 c");
  });
});
