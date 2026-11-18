import { fixtures, type CopilotView, type OverlayState } from "@augur/ui";
import { describe, expect, it } from "vitest";
import { runCopilot, type CopilotDeps } from "./copilot";

const withCharacter: OverlayState = {
  contractVersion: 1,
  appVersion: "0.1.0",
  gameFocused: true,
  character: fixtures.character,
  lastItem: null,
  dataPatch: null,
  dataPatchPoe1: null,
};

function deps(overrides: Partial<CopilotDeps> = {}): CopilotDeps {
  return {
    overlayState: async () => withCharacter,
    evaluate: async () => fixtures.upgradeDelta,
    price: async () => fixtures.tradePrice,
    ...overrides,
  };
}

async function run(d: CopilotDeps) {
  const seen: CopilotView[] = [];
  const final = await runCopilot(fixtures.rareBow, d, (v) => seen.push(v));
  return { final, seen };
}

describe("runCopilot", () => {
  it("parcours nominal : prix en attente puis prix connu", async () => {
    const { final, seen } = await run(deps());
    expect(seen.map((v) => (v.status === "ready" ? v.pricing : v.status))).toEqual(["pending", "done"]);
    expect(final.status === "ready" && final.price?.median).toBe(14);
  });

  it("sans personnage importé : le prix reste cherché", async () => {
    const { final } = await run(deps({ overlayState: async () => ({ ...withCharacter, character: null }) }));
    expect(final.status).toBe("noCharacter");
    expect(final.status === "noCharacter" && final.pricing).toBe("done");
  });

  it("monnaie : prix seul, sans personnage ni comparaison", async () => {
    let evaluated = false;
    const seen: CopilotView[] = [];
    const final = await runCopilot(
      fixtures.chaosStack,
      deps({
        overlayState: async () => ({ ...withCharacter, character: null }),
        evaluate: async () => {
          evaluated = true;
          return fixtures.upgradeDelta;
        },
        price: async () => fixtures.stackPrice,
      }),
      (v) => seen.push(v),
    );
    expect(evaluated).toBe(false);
    expect(final.status === "ready" && final.priceOnly && final.price?.quantity).toBe(20);
    expect(seen[0]?.status === "ready" && seen[0].pricing).toBe("pending");
  });

  it("unique sans cote automatique : cote trade proposée", async () => {
    const final = await runCopilot(
      fixtures.uniqueGloves,
      deps({ price: async () => Promise.reject({ code: "unsupported", message: "Pas de cote", retryAfterMs: null }) }),
      () => undefined,
    );
    expect(final.status === "ready" && final.pricing).toBe("unpriced");
    expect(final.status === "ready" && final.priceNote).toBeUndefined();
  });

  it("évaluation coupée : on garde le prix", async () => {
    const { final } = await run(
      deps({ evaluate: async () => Promise.reject({ code: "featureDisabled", message: "", retryAfterMs: null }) }),
    );
    expect(final.status === "ready" && final.delta).toBeUndefined();
    expect(final.status === "ready" && final.pricing).toBe("done");
  });

  it("prix indisponible ou désactivé", async () => {
    const unavailable = await run(deps({ price: async () => Promise.reject(new Error("commande absente")) }));
    expect(unavailable.final.status === "ready" && unavailable.final.pricing).toBe("unavailable");
    const disabled = await run(
      deps({ price: async () => Promise.reject({ code: "featureDisabled", message: "", retryAfterMs: null }) }),
    );
    expect(disabled.final.status === "ready" && disabled.final.pricing).toBe("disabled");
  });
});
