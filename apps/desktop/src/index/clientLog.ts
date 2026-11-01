import type { ClientLogStatus } from "../bridge";

export type ClientLogSummary = { ok: boolean; label: string; hint: string | null };

/**
 * Texte de la ligne « Client.txt » : distingue « introuvable » de « trouvé,
 * en attente d'un changement de zone », que l'ancien libellé confondait.
 */
export function summarizeClientLog(status: ClientLogStatus | null): ClientLogSummary {
  if (!status) return { ok: false, label: "Client.txt\u00a0: recherche…", hint: null };
  if (!status.watching || !status.path) {
    return {
      ok: false,
      label: "Client.txt introuvable",
      hint: "Indique son emplacement ci-dessous, ou passe le jeu au premier plan : Augur le retrouve à partir du jeu lancé.",
    };
  }
  if (status.lastZone) {
    return { ok: true, label: `Zone\u00a0: ${status.lastZone.zone}`, hint: null };
  }
  return {
    ok: true,
    label: "Client.txt trouvé",
    hint: "La zone s'affichera au prochain changement de zone en jeu.",
  };
}
