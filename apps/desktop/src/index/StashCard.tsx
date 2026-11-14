import { StatePanel } from "@augur/ui";
import { useEffect, useRef, useState } from "react";
import { commands, isAppError, isTauri, on, type StashStatus, type StashTab } from "../bridge";

const champ =
  "min-w-0 rounded-chip border border-ink-700 bg-ink-950 px-2 py-1.5 text-sm text-text-primary placeholder:text-text-subtle";
const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold disabled:opacity-50";
const message = (err: unknown) => (isAppError(err) ? err.message : String(err));

/** « 1 234 c (15,8 div) » */
export function formatChaos(chaos: number, coursDivine: number | null): string {
  const c = `${Math.round(chaos).toLocaleString("fr-FR")} c`;
  if (!coursDivine || Math.abs(chaos) < coursDivine / 2) return c;
  return `${c} (${(chaos / coursDivine).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} div)`;
}

const signe = (v: number) => (v > 0 ? "+" : v < 0 ? "−" : "");
const heure = (iso: string) =>
  new Date(iso).toLocaleString("fr-FR", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });

/**
 * Coffres de PoE 1 : connexion (compte + POESESSID), onglets suivis, relevé de leur
 * valeur au prix poe.ninja et gain depuis le relevé précédent. Avec « relevé
 * automatique », un relevé est fait au début et à la fin de chaque session Ledger.
 */
export function StashCard() {
  const [status, setStatus] = useState<StashStatus | null>(null);
  const [tabs, setTabs] = useState<StashTab[] | null>(null);
  const [account, setAccount] = useState("");
  const [session, setSession] = useState("");
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [auto, setAuto] = useState(() => {
    try {
      return localStorage.getItem("augur.poe1.releveAuto") !== "0";
    } catch {
      return true;
    }
  });
  const autoRef = useRef(auto);
  autoRef.current = auto;
  const statusRef = useRef(status);
  statusRef.current = status;

  useEffect(() => {
    if (!isTauri()) return;
    void commands
      .getPoe1Stash()
      .then((s) => {
        setStatus(s);
        setAccount(s.account ?? "");
      })
      .catch(() => undefined);
    const un = on.poe1Stash(setStatus);
    // Relevé automatique au début et à la fin d'une session Ledger.
    let actif: boolean | null = null;
    const unLedger = on.ledgerSession((l) => {
      const avant = actif;
      actif = l.active;
      const s = statusRef.current;
      if (avant === null || avant === l.active || !autoRef.current) return;
      if (!s?.hasSession || !s.account || s.tabs.length === 0) return;
      void relever();
    });
    void commands
      .getLedgerStatus()
      .then((l) => (actif = l.active))
      .catch(() => undefined);
    return () => {
      void un.then((f) => f());
      void unLedger.then((f) => f());
    };
  }, []);

  async function run<T>(f: () => Promise<T>): Promise<T | undefined> {
    setError(null);
    setBusy(true);
    try {
      return await f();
    } catch (err) {
      setError(message(err));
      return undefined;
    } finally {
      setBusy(false);
    }
  }

  async function connecter() {
    const liste = await run(() => commands.connectPoe1Stash(account, session.trim() || null));
    if (liste) {
      setTabs(liste);
      setSession("");
      setEditing(false);
      setStatus(await commands.getPoe1Stash());
    }
  }

  async function chargerOnglets() {
    const liste = await run(commands.listPoe1StashTabs);
    if (liste) setTabs(liste);
  }

  async function basculer(index: number) {
    if (!status) return;
    const suivis = status.tabs.includes(index) ? status.tabs.filter((t) => t !== index) : [...status.tabs, index];
    const s = await run(() => commands.setPoe1StashTabs(suivis));
    if (s) setStatus(s);
  }

  async function relever() {
    const s = await run(commands.takePoe1StashSnapshot);
    if (s) setStatus(s);
  }

  function choisirAuto(v: boolean) {
    setAuto(v);
    try {
      localStorage.setItem("augur.poe1.releveAuto", v ? "1" : "0");
    } catch {
      // Préférence non retenue : sans gravité.
    }
  }

  const connecte = Boolean(status?.hasSession && status.account);
  const last = status?.last ?? null;
  const cours = last?.coursDivine ?? null;

  return (
    <div className="space-y-4 text-sm">
      {(!connecte || editing) && (
        <div className="space-y-2">
          <p className="text-text-muted">
            Augur lit tes onglets sur pathofexile.com avec ta session de connexion (cookie <code>POESESSID</code>). Elle reste
            chiffrée sur ce PC (Gestionnaire d'identification Windows), n'est envoyée qu'à pathofexile.com, et sert
            uniquement à lire tes coffres.
          </p>
          <p className="text-xs text-signal-warn">
            Cette valeur donne accès à ton compte : ne la donne à personne d'autre. Elle change quand tu te déconnectes du
            site ; il faudra alors la recopier.
          </p>
          <p className="text-xs text-text-subtle">
            Pour la trouver : connecté sur pathofexile.com, ouvre les outils du navigateur (F12) &gt; Application (ou
            Stockage) &gt; Cookies &gt; www.pathofexile.com, et copie la valeur de <code>POESESSID</code>.
          </p>
          <div className="flex flex-wrap gap-2">
            <input
              value={account}
              onChange={(e) => setAccount(e.target.value)}
              placeholder="Nom de compte, ex. Nom#1234"
              aria-label="Nom de compte"
              className={`${champ} w-56`}
            />
            <input
              value={session}
              onChange={(e) => setSession(e.target.value)}
              type="password"
              autoComplete="off"
              placeholder={status?.hasSession ? "POESESSID (inchangée si vide)" : "POESESSID"}
              aria-label="POESESSID"
              className={`${champ} w-72 font-mono text-xs`}
            />
            <button
              type="button"
              onClick={() => void connecter()}
              disabled={busy || !account.trim() || (!session.trim() && !status?.hasSession)}
              className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong`}
            >
              {busy ? "Vérification…" : "Connecter"}
            </button>
            {editing && (
              <button type="button" onClick={() => setEditing(false)} className={`${btn} text-text-subtle hover:text-text-muted`}>
                Annuler
              </button>
            )}
          </div>
        </div>
      )}

      {connecte && !editing && status && (
        <>
          <div className="flex flex-wrap items-center gap-3">
            <p className="text-text-muted">
              Compte <span className="text-text-primary">{status.account}</span>, ligue{" "}
              <span className="text-text-primary">{status.league}</span>
            </p>
            <button type="button" onClick={() => setEditing(true)} className="text-xs text-accent hover:underline">
              Changer
            </button>
            <button
              type="button"
              onClick={() => void run(commands.forgetPoe1StashSession).then((s) => s && setStatus(s))}
              className="text-xs text-text-subtle hover:text-text-muted"
            >
              Oublier ma session
            </button>
          </div>

          <div className="space-y-2">
            <div className="flex items-center gap-3">
              <p className="text-xs font-semibold uppercase tracking-wider text-text-subtle">
                Onglets suivis ({status.tabs.length})
              </p>
              <button type="button" onClick={() => void chargerOnglets()} disabled={busy} className="text-xs text-accent hover:underline">
                {tabs ? "Actualiser la liste" : "Choisir les onglets"}
              </button>
            </div>
            {tabs && (
              <div className="grid max-h-56 grid-cols-3 gap-x-4 gap-y-1 overflow-y-auto rounded-chip border border-ink-700 p-2 text-xs">
                {tabs.map((t) => (
                  <label key={t.index} className="flex items-center gap-1.5 text-text-muted">
                    <input
                      type="checkbox"
                      checked={status.tabs.includes(t.index)}
                      disabled={busy}
                      onChange={() => void basculer(t.index)}
                      className="accent-[var(--color-accent)]"
                    />
                    <span className="truncate" title={t.genre}>
                      {t.nom || `Onglet ${t.index}`}
                    </span>
                  </label>
                ))}
              </div>
            )}
            <p className="text-xs text-text-subtle">
              Chaque onglet est une lecture sur le site (une toutes les 2,5 s, 20 onglets au plus). Les onglets de cartes et
              d'uniques spéciaux peuvent revenir vides.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-4">
            <button
              type="button"
              onClick={() => void relever()}
              disabled={busy || status.busy || status.tabs.length === 0}
              className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong`}
            >
              {busy || status.busy ? "Relevé en cours…" : "Relever mes coffres"}
            </button>
            <label className="flex items-center gap-2 text-xs text-text-muted">
              <input
                type="checkbox"
                checked={auto}
                onChange={(e) => choisirAuto(e.target.checked)}
                className="accent-[var(--color-accent)]"
              />
              Relevé automatique au début et à la fin d'une session Ledger
            </label>
          </div>

          {last && (
            <div className="space-y-3">
              <div className="grid grid-cols-3 gap-3">
                <div className="rounded-chip border border-ink-700 p-3">
                  <p className="text-xs text-text-subtle">Valeur relevée</p>
                  <p className="text-lg font-semibold">{formatChaos(last.chaos, cours)}</p>
                  <p className="text-xs text-text-subtle">le {heure(last.at)}</p>
                </div>
                {status.gain !== null && status.previous && (
                  <div className="rounded-chip border border-ink-700 p-3">
                    <p className="text-xs text-text-subtle">Depuis le relevé du {heure(status.previous.at)}</p>
                    <p className={`text-lg font-semibold ${status.gain >= 0 ? "text-signal-gain" : "text-signal-loss"}`}>
                      {signe(status.gain)}
                      {formatChaos(Math.abs(status.gain), cours)}
                    </p>
                  </div>
                )}
                <div className="rounded-chip border border-ink-700 p-3">
                  <p className="text-xs text-text-subtle">Objets sans cote</p>
                  <p className="text-lg font-semibold">{last.nonCotes}</p>
                  <p className="text-xs text-text-subtle">rares, objets que poe.ninja ne suit pas</p>
                </div>
              </div>

              {status.changes.length > 0 && (
                <div>
                  <p className="mb-1 text-xs font-semibold uppercase tracking-wider text-text-subtle">Ce qui a bougé</p>
                  <ul className="space-y-0.5 text-xs">
                    {status.changes.map((c) => (
                      <li key={c.nom} className="flex justify-between gap-4">
                        <span className="text-text-muted">
                          {c.quantite !== 0 && `${signe(c.quantite)}${Math.abs(c.quantite)} × `}
                          {c.nom}
                        </span>
                        <span className={c.chaos >= 0 ? "text-signal-gain" : "text-signal-loss"}>
                          {signe(c.chaos)}
                          {formatChaos(Math.abs(c.chaos), cours)}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              <div>
                <p className="mb-1 text-xs font-semibold uppercase tracking-wider text-text-subtle">Plus grosses valeurs</p>
                <ul className="space-y-0.5 text-xs">
                  {last.lignes.slice(0, 8).map((l) => (
                    <li key={l.nom} className="flex justify-between gap-4">
                      <span className="text-text-muted">
                        {l.quantite > 1 && `${l.quantite.toLocaleString("fr-FR")} × `}
                        {l.nom}
                      </span>
                      <span>{formatChaos(l.chaos, cours)}</span>
                    </li>
                  ))}
                </ul>
              </div>
            </div>
          )}
        </>
      )}

      {error && (
        <StatePanel icon="alert" title="Coffres non lus" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}
