import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { champManquant, diagnosticTicket, LIGNES_JOURNAL_TICKET, messageTicket, MODULES_BUG } from "./bugReport";

describe("signaler un bug", () => {
  it("joint les dernières lignes du journal", () => {
    const journal = Array.from({ length: 40 }, (_, i) => `ligne ${i}`);
    const t = diagnosticTicket("Augur : v0.1.16", journal);
    expect(t.startsWith("Augur : v0.1.16\n\nJournal")).toBe(true);
    expect(t).toContain("ligne 39");
    expect(t).not.toContain(`ligne ${39 - LIGNES_JOURNAL_TICKET}\n`);
    expect(diagnosticTicket("résumé", [])).toBe("résumé");
  });

  it("exige titre, étapes et résultat", () => {
    expect(champManquant({ titre: " ", etapes: "a", attendu: "b" })).toMatch(/titre/);
    expect(champManquant({ titre: "x", etapes: "", attendu: "b" })).toMatch(/fait/);
    expect(champManquant({ titre: "x", etapes: "a", attendu: "" })).toMatch(/passé/);
    expect(champManquant({ titre: "x", etapes: "a", attendu: "b" })).toBeNull();
  });

  it("propose les mêmes options que le modèle de ticket GitHub", () => {
    const yml = readFileSync(resolve(process.cwd(), "../../.github/ISSUE_TEMPLATE/bug-beta.yml"), "utf8");
    for (const m of MODULES_BUG) expect(yml).toContain(`        - ${m}\n`);
  });

  it("garde un modèle de ticket valide (sinon GitHub ouvre un ticket vierge)", () => {
    const yml = readFileSync(resolve(process.cwd(), "../../.github/ISSUE_TEMPLATE/bug-beta.yml"), "utf8");
    // Une valeur YAML sans guillemets ne peut pas contenir « : » suivi d'un espace.
    const fautives = yml
      .split("\n")
      .map((l) => /^\s*(?:- )?[\w-]+: (.*)$/.exec(l)?.[1]?.trim() ?? "")
      .filter((v) => v && !/^["'|>[{]/.test(v) && /:\s/.test(v));
    expect(fautives).toEqual([]);
  });

  it("signale le rapport complet copié dans le presse-papiers", () => {
    expect(messageTicket({ truncated: false, copied: true })).toMatch(/presse-papiers.*Ctrl\+V/);
    expect(messageTicket({ truncated: true, copied: true })).toMatch(/raccourci.*presse-papiers/);
    expect(messageTicket({ truncated: true, copied: false })).not.toMatch(/presse-papiers/);
  });
});
