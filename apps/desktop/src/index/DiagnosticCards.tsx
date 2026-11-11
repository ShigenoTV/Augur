import { useEffect, useState } from "react";
import { commands, isAppError, isTauri, type Diagnostic } from "../bridge";
import { champManquant, diagnosticTicket, MODULES_BUG } from "./bugReport";

const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold";
const champ =
  "w-full rounded-chip border border-ink-700 bg-ink-950 p-2 text-sm text-text-primary placeholder:text-text-subtle";

function useDiagnostic(actif: boolean) {
  const [diag, setDiag] = useState<Diagnostic | null>(null);
  const recharger = () => {
    if (isTauri()) void commands.getDiagnostic().then(setDiag).catch(() => undefined);
  };
  // Rechargé quand la page Réglages s'ouvre : le journal bouge pendant l'utilisation.
  useEffect(recharger, [actif]);
  return { diag, recharger };
}

/** Réglages → Diagnostic : résumé technique, journal d'Augur et copie. */
export function DiagnosticCard({ actif }: { actif: boolean }) {
  const { diag, recharger } = useDiagnostic(actif);
  const [message, setMessage] = useState<string | null>(null);

  async function copier() {
    setMessage(null);
    if (!isTauri()) {
      setMessage("La copie du diagnostic fonctionne dans l'application.");
      return;
    }
    try {
      await commands.copyDiagnostic();
      setMessage("Diagnostic copié : colle-le dans un ticket ou un message à Max.");
      recharger();
    } catch (err) {
      setMessage(isAppError(err) ? err.message : String(err));
    }
  }

  return (
    <div className="space-y-3 text-sm">
      <pre className="whitespace-pre-wrap rounded-chip border border-ink-700 bg-ink-950 p-2 font-mono text-xs text-text-muted">
        {diag?.resume ?? "Diagnostic disponible dans l'application."}
      </pre>
      <div>
        <p className="mb-1 text-xs text-text-subtle">Journal d'Augur ({diag?.journal.length ?? 0} lignes, la plus récente en bas)</p>
        <pre
          aria-label="Journal de diagnostic"
          className="max-h-56 overflow-auto whitespace-pre-wrap rounded-chip border border-ink-700 bg-ink-950 p-2 font-mono text-[11px] text-text-muted"
        >
          {diag?.journal.length ? diag.journal.join("\n") : "Rien d'anormal depuis le démarrage."}
        </pre>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <button type="button" onClick={() => void copier()} className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong`}>
          Copier le diagnostic
        </button>
        <button type="button" onClick={recharger} className={`${btn} border border-ink-600 font-normal text-text-primary hover:bg-ink-800`}>
          Actualiser
        </button>
        {message && <span className="text-text-subtle">{message}</span>}
      </div>
      <p className="text-xs text-text-subtle">
        Aucune donnée personnelle : ton dossier Windows est masqué (~) et le nom de ton personnage n'apparaît pas. Rien n'est envoyé
        par Augur.
      </p>
    </div>
  );
}

/** Réglages → Signaler un bug : prépare un ticket GitHub que le joueur relit et envoie lui-même. */
export function BugReportCard({ actif }: { actif: boolean }) {
  const { diag } = useDiagnostic(actif);
  const [titre, setTitre] = useState("");
  const [module, setModule] = useState<string>(MODULES_BUG[1]);
  const [etapes, setEtapes] = useState("");
  const [attendu, setAttendu] = useState("");
  const [objet, setObjet] = useState("");
  const [joindre, setJoindre] = useState(true);
  const [message, setMessage] = useState<{ ok: boolean; texte: string } | null>(null);

  const diagnostic = diag ? diagnosticTicket(diag.resume, diag.journal) : "";

  async function envoyer() {
    const manque = champManquant({ titre, etapes, attendu });
    if (manque) {
      setMessage({ ok: false, texte: manque });
      return;
    }
    if (!isTauri()) {
      setMessage({ ok: false, texte: "Le signalement fonctionne dans l'application." });
      return;
    }
    try {
      const r = await commands.openBugReport({
        titre,
        module,
        etapes,
        attendu,
        objet,
        diagnostic: joindre ? diagnostic : "",
      });
      setMessage({
        ok: true,
        texte: r.truncated
          ? "Ticket ouvert dans ton navigateur. Le texte était trop long et a été raccourci : colle la suite dans le ticket avant de cliquer sur « Create »."
          : "Ticket ouvert dans ton navigateur : relis-le, ajoute une capture si besoin, puis clique sur « Create ».",
      });
    } catch (err) {
      setMessage({ ok: false, texte: isAppError(err) ? err.message : String(err) });
    }
  }

  return (
    <div className="space-y-3 text-sm">
      <div className="grid grid-cols-[2fr_1fr] gap-3">
        <label className="space-y-1">
          <span className="text-xs text-text-subtle">Titre court</span>
          <input value={titre} onChange={(e) => setTitre(e.target.value)} placeholder="L'overlay reste vide sur un anneau" className={champ} />
        </label>
        <label className="space-y-1">
          <span className="text-xs text-text-subtle">Où est le problème ?</span>
          <select value={module} onChange={(e) => setModule(e.target.value)} className={champ}>
            {MODULES_BUG.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </label>
      </div>
      <label className="block space-y-1">
        <span className="text-xs text-text-subtle">Ce que tu as fait</span>
        <textarea value={etapes} onChange={(e) => setEtapes(e.target.value)} placeholder={"1. Ctrl+C sur un anneau rare\n2. …"} className={`${champ} h-20 resize-y`} />
      </label>
      <label className="block space-y-1">
        <span className="text-xs text-text-subtle">Ce que tu attendais, et ce qui s'est passé</span>
        <textarea value={attendu} onChange={(e) => setAttendu(e.target.value)} className={`${champ} h-20 resize-y`} />
      </label>
      <label className="block space-y-1">
        <span className="text-xs text-text-subtle">Texte de l'objet (facultatif, Ctrl+C en jeu puis colle ici)</span>
        <textarea value={objet} onChange={(e) => setObjet(e.target.value)} className={`${champ} h-16 resize-y font-mono text-xs`} />
      </label>
      <label className="flex items-center gap-2 text-text-muted">
        <input type="checkbox" checked={joindre} onChange={(e) => setJoindre(e.target.checked)} className="accent-[var(--color-accent)]" />
        Joindre le diagnostic ci-dessous
      </label>
      {joindre && (
        <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded-chip border border-ink-700 bg-ink-950 p-2 font-mono text-[11px] text-text-muted">
          {diagnostic || "Diagnostic disponible dans l'application."}
        </pre>
      )}
      <div className="flex flex-wrap items-center gap-3">
        <button type="button" onClick={() => void envoyer()} className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong`}>
          Préparer le ticket
        </button>
        {message && <span className={message.ok ? "text-text-muted" : "text-signal-loss"}>{message.texte}</span>}
      </div>
      <p className="text-xs text-text-subtle">
        Augur ouvre un ticket GitHub prérempli dans ton navigateur (compte GitHub gratuit nécessaire). Rien n'est envoyé tant que tu n'as pas
        cliqué sur « Create ». Le ticket est public : n'y mets aucune information personnelle.
      </p>
    </div>
  );
}
