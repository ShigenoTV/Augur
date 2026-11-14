import { describe, expect, it } from "vitest";
import { formatChaos } from "./StashCard";

describe("formatChaos", () => {
  it("ajoute les Divines au-delà d'une demie", () => {
    expect(formatChaos(30, 78)).toBe("30 c");
    expect(formatChaos(156, 78)).toBe("156 c (2 div)");
    expect(formatChaos(156, null)).toBe("156 c");
  });
});
