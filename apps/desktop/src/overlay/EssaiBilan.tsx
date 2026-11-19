import type { ForgeChain } from "../bridge";

/** Formulation du bilan de l'essai précédent (exportée pour les tests). */
export function texteBilan(
  e: ForgeChain,
  cout: (v: number | null) => string,
): { titre: string; detail: string } {
  const serie =
    e.essais > 0
      ? `${e.essais} essai${e.essais > 1 ? "s" : ""} · ~${cout(e.depense)} dépensé${e.depense > 1 ? "s" : ""}`
      : "";
  switch (e.verdict) {
    case "atteint":
      return { titre: "Ça a marché : objet fini", detail: serie };
    case "reussi":
      return {
        titre: `Ça a marché, reste ${cout(e.reste)}`,
        detail: [
          e.resteAvant != null ? `avant : ${cout(e.resteAvant)}` : "",
          serie,
        ]
          .filter(Boolean)
          .join(" · "),
      };
    case "rate":
      return { titre: `Raté, reste ${cout(e.reste)}`, detail: serie };
    case "memeObjet":
      return { titre: "Même objet qu'à la copie précédente", detail: serie };
  }
}

/** Clé du bandeau : nouvelle à chaque essai, pour qu'il réapparaisse alors que le panneau reste. */
export const cleBilan = (e: ForgeChain) => `${e.essais}-${e.verdict}`;

/**
 * Bilan de l'essai depuis la copie précédente, en tête du panneau Forge : le panneau reste le même
 * d'un Ctrl+Alt+C à l'autre et ce bandeau dit si l'essai a fait avancer le plan.
 */
export function EssaiBilan({
  enchainement: e,
  cout,
}: {
  enchainement: ForgeChain;
  cout: (v: number | null) => string;
}) {
  const { titre, detail } = texteBilan(e, cout);
  const ton =
    e.verdict === "rate"
      ? "text-signal-loss"
      : e.verdict === "memeObjet"
        ? "text-text-muted"
        : "text-signal-gain";
  const icone =
    e.verdict === "rate" ? "✗" : e.verdict === "memeObjet" ? "=" : "✓";
  return (
    <div
      role="status"
      className="mb-2 rounded-md border border-ink-700 bg-ink-900/60 px-2.5 py-1.5 motion-safe:animate-[augur-enter_120ms_var(--ease-out-quick)]"
    >
      <p className={`text-sm font-semibold ${ton}`}>
        {icone} {titre}
      </p>
      {detail && <p className="text-2xs text-text-subtle">{detail}</p>}
    </div>
  );
}
