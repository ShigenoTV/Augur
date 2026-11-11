/**
 * « Signaler un bug » : options du modèle de ticket et texte du diagnostic joint.
 * Les options doivent rester identiques à celles de .github/ISSUE_TEMPLATE/bug-beta.yml.
 */
export const MODULES_BUG = [
  "Installation ou SmartScreen / antivirus",
  "Overlay (Ctrl+C sur un objet)",
  "Objet mal lu ou « Objet non reconnu »",
  "Personnage (import Path of Building)",
  "Forge",
  "Session de mesures",
  "Atlas Ledger",
  "Réglages",
  "Mises à jour",
  "Autre",
] as const;

/** Lignes du journal jointes au ticket (la copie du diagnostic garde tout le journal). */
export const LIGNES_JOURNAL_TICKET = 15;

/** Texte joint au ticket : résumé technique puis les dernières lignes du journal. */
export function diagnosticTicket(resume: string, journal: string[]): string {
  const fin = journal.slice(-LIGNES_JOURNAL_TICKET);
  return fin.length ? `${resume}\n\nJournal (dernières lignes) :\n${fin.join("\n")}` : resume;
}

/** Message d'erreur si le formulaire est incomplet, sinon null. */
export function champManquant(f: { titre: string; etapes: string; attendu: string }): string | null {
  if (!f.titre.trim()) return "Donne un titre court au bug.";
  if (!f.etapes.trim()) return "Décris ce que tu as fait.";
  if (!f.attendu.trim()) return "Décris ce qui s'est passé.";
  return null;
}
