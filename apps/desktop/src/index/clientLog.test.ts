import { describe, expect, it } from "vitest";
import { summarizeClientLog } from "./clientLog";

const base = { path: null, watching: false, lastZone: null, searched: [] };

describe("summarizeClientLog", () => {
  it("distingue introuvable, trouvé et zone lue", () => {
    expect(summarizeClientLog(base).label).toBe("Client.txt introuvable");
    const found = { ...base, path: "D:\\SteamLibrary\\...\\Client.txt", watching: true };
    expect(summarizeClientLog(found)).toMatchObject({ ok: true, label: "Client.txt trouvé" });
    const zone = { ...found, lastZone: { zone: "Clearfell", at: "2026-10-31T20:00:00Z" } };
    expect(summarizeClientLog(zone).label).toBe("Zone\u00a0: Clearfell");
  });
});
