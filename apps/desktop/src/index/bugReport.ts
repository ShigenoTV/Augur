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

/** Message affiché après l'ouverture du ticket dans le navigateur. */
export function messageTicket(r: { truncated: boolean; copied: boolean }): string {
  if (!r.copied) {
    return r.truncated
      ? "Ticket ouvert dans ton navigateur. Le texte était trop long et a été raccourci : complète-le avant de cliquer sur « Create »."
      : "Ticket ouvert dans ton navigateur : relis-le, ajoute une capture si besoin, puis clique sur « Create ».";
  }
  const debut = r.truncated
    ? "Ticket ouvert dans ton navigateur. Le texte était trop long et a été raccourci"
    : "Ticket ouvert dans ton navigateur";
  return `${debut}. Le rapport complet est aussi dans ton presse-papiers : si une case est vide ou raccourcie (ou si GitHub n'affiche qu'une description vide), colle-le avec Ctrl+V, puis clique sur « Create ».`;
}
