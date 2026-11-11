import "@augur/ui/styles.css";
import "./main.css";
import {
  Icon,
  ItemTooltip,
  Kbd,
  StatePanel,
  UpdateBanner,
  type OverlayState,
  type ParsedItem,
} from "@augur/ui";
import { StrictMode, useEffect, useState, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { commands, isAppError, isTauri, on, type ClientLogStatus, type Settings } from "../bridge";
import { CharacterCard } from "./CharacterCard";
import { ClientLogCard } from "./ClientLogCard";
import { LeagueCard, ShortcutsCard, UpgradeBudgetCard } from "./SettingsCards";
import { afficherRaccourci, SHORTCUTS_PAR_DEFAUT } from "./shortcuts";
import { summarizeClientLog } from "./clientLog";
import { ForgeCard } from "./ForgeCard";
import { LedgerCard } from "./LedgerCard";
import { MeasureCard } from "./MeasureCard";
import { AffichageContext, DEVISES, useAffichage } from "./devise";
import { useUpdates } from "./useUpdates";

/** Pas de canal bêta tant que les mises à jour passent par GitHub Releases (voir src-tauri/src/updates.rs). */
const BETA_AVAILABLE = false;

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-panel border border-ink-700 bg-ink-900 p-4">
      <h2 className="mb-3 text-xs font-semibold uppercase tracking-wider text-text-subtle">{title}</h2>
      {children}
    </section>
  );
}

function Status({ ok, label }: { ok: boolean; label: string }) {
  return (
    <p className="flex items-center gap-2 text-sm">
      <span className={ok ? "text-signal-gain" : "text-text-subtle"}>
        <Icon name={ok ? "check" : "flat"} size={14} />
      </span>
      {label}
    </p>
  );
}

type PageId = "copilot" | "forge" | "ledger" | "outils" | "reglages";

/** Barre latérale (reprise de l'ancienne appli de craft de Max) : modules, puis outils et réglages. */
const PAGES: { id: PageId; nom: string; desc: string; groupe: 0 | 1 }[] = [
  { id: "copilot", nom: "Copilot", desc: "Ton personnage, les raccourcis en jeu et le dernier objet analysé.", groupe: 0 },
  {
    id: "forge",
    nom: "Forge",
    desc: "Décris l'objet voulu : le solveur cherche la suite de monnaies au coût moyen minimal, avec ses embranchements.",
    groupe: 0,
  },
  { id: "ledger", nom: "Atlas Ledger", desc: "Suivi de tes sessions de farm de cartes et historique.", groupe: 0 },
  { id: "outils", nom: "Outils", desc: "Session de mesures et banc d'essai du parseur.", groupe: 1 },
  { id: "reglages", nom: "Réglages", desc: "Journal du jeu, ligue souhaitée, raccourcis et mises à jour d'Augur.", groupe: 1 },
];

const CLE_PAGE = "augur.page";

function pageInitiale(): PageId {
  try {
    const p = localStorage.getItem(CLE_PAGE);
    if (PAGES.some((x) => x.id === p)) return p as PageId;
  } catch {
    // Stockage indisponible : page par défaut.
  }
  return "copilot";
}

/** Fenêtre principale : barre latérale à gauche, une page par module. */
function App() {
  const [state, setState] = useState<OverlayState | null>(null);
  const [clientLog, setClientLog] = useState<ClientLogStatus | null>(null);
  const [text, setText] = useState("");
  const [parsed, setParsed] = useState<ParsedItem | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [dernier, setDernier] = useState<{ item: ParsedItem; a: Date } | null>(null);
  const [page, setPage] = useState<PageId>(pageInitiale);
  const updates = useUpdates();
  const affichage = useAffichage();

  function ouvrir(p: PageId) {
    setPage(p);
    try {
      localStorage.setItem(CLE_PAGE, p);
    } catch {
      // Stockage indisponible : la page n'est pas retenue.
    }
  }

  // Ctrl+1 à Ctrl+5 : changer de page.
  useEffect(() => {
    const touche = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.altKey || e.shiftKey) return;
      const p = PAGES[Number(e.key) - 1];
      if (p) {
        e.preventDefault();
        ouvrir(p.id);
      }
    };
    window.addEventListener("keydown", touche);
    return () => window.removeEventListener("keydown", touche);
  }, []);

  useEffect(() => {
    if (isTauri()) void commands.getSettings().then(setSettings).catch(() => undefined);
  }, []);

  async function toggleBeta(beta: boolean) {
    try {
      setSettings(await commands.setUpdateChannel(beta));
      void updates.check();
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
    }
  }

  useEffect(() => {
    if (!isTauri()) return;
    const refresh = () => void commands.getOverlayState().then(setState).catch(() => undefined);
    refresh();
    const timer = setInterval(refresh, 2_000);
    void commands.getClientLogStatus().then(setClientLog).catch(() => undefined);
    const unlisten = on.clientLogStatus(setClientLog);
    const unlistenItem = on.itemCopied((item) => setDernier({ item, a: new Date() }));
    return () => {
      clearInterval(timer);
      void unlisten.then((f) => f());
      void unlistenItem.then((f) => f());
    };
  }, []);

  const logSummary = summarizeClientLog(clientLog);
  const raccourcis = settings?.shortcuts ?? SHORTCUTS_PAR_DEFAUT;

  async function tryParse() {
    setError(null);
    setParsed(null);
    if (!isTauri()) {
      setError("Le banc d'essai du parseur fonctionne dans l'application.");
      return;
    }
    try {
      setParsed(await commands.parseClipboardItem(text));
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
    }
  }

  const courante = PAGES.find((p) => p.id === page)!;
  const lien = (p: (typeof PAGES)[number], i: number) => (
    <button
      key={p.id}
      type="button"
      onClick={() => ouvrir(p.id)}
      aria-current={page === p.id ? "page" : undefined}
      title={`Ctrl+${i + 1}`}
      className={`block w-full border-l-[3px] px-4 py-2 text-left text-sm ${
        page === p.id
          ? "border-accent bg-ink-850 font-semibold text-accent"
          : "border-transparent text-text-muted hover:bg-ink-850 hover:text-text-primary"
      }`}
    >
      {p.nom}
    </button>
  );

  return (
    <AffichageContext.Provider value={affichage}>
      <div className="flex min-h-screen">
        <aside className="sticky top-0 flex h-screen w-52 shrink-0 flex-col border-r border-ink-700 bg-ink-900 py-5">
          <div className="mb-4 px-4">
            <p className="font-display text-xl font-semibold text-accent">Augur</p>
            <p className="text-xs text-text-subtle">{state ? `v${state.appVersion}` : "Upgrade Copilot"}</p>
          </div>
          <nav aria-label="Modules">
            {PAGES.filter((p) => p.groupe === 0).map((p) => lien(p, PAGES.indexOf(p)))}
            <div className="mx-4 my-2 border-t border-ink-700" />
            {PAGES.filter((p) => p.groupe === 1).map((p) => lien(p, PAGES.indexOf(p)))}
          </nav>

          <div className="mt-5 px-4">
            <p className="mb-1.5 text-xs text-text-subtle">Coûts affichés en</p>
            <div className="flex overflow-hidden rounded-chip border border-ink-600 text-xs" role="radiogroup" aria-label="Monnaie des coûts">
              {DEVISES.map((d) => {
                const actif = affichage.affichage.devise === d.id;
                const sansTaux = affichage.affichage.taux[d.id] <= 0;
                return (
                  <button
                    key={d.id}
                    type="button"
                    role="radio"
                    aria-checked={actif}
                    disabled={sansTaux}
                    title={sansTaux ? "Ouvre la Forge pour charger les prix" : undefined}
                    onClick={() => affichage.choisir(d.id)}
                    className={`flex-1 px-1 py-1 disabled:opacity-40 ${actif ? "bg-accent font-semibold text-accent-ink" : "text-text-muted hover:bg-ink-800"}`}
                  >
                    {d.nom}
                  </button>
                );
              })}
            </div>
          </div>

          <div className="mt-auto space-y-1.5 px-4">
            <Status ok={Boolean(state?.gameFocused)} label={state?.gameFocused ? "Jeu au premier plan" : "Jeu non détecté"} />
            <Status ok={logSummary.ok} label={logSummary.label} />
            <Status ok={Boolean(state?.dataPatch)} label={state?.dataPatch ? `Données du jeu : ${state.dataPatch}` : "Données du jeu indisponibles"} />
            <button
              type="button"
              onClick={() => void updates.check()}
              disabled={updates.update.phase === "checking" || updates.update.phase === "installing"}
              className="mt-2 w-full whitespace-nowrap rounded-chip border border-ink-600 px-1 py-1 text-[11px] text-text-primary hover:bg-ink-800 disabled:opacity-50"
            >
              {updates.update.phase === "checking"
                ? "Vérification…"
                : updates.update.phase === "upToDate"
                  ? "Augur est à jour"
                  : "Rechercher une mise à jour"}
            </button>
          </div>
        </aside>

        <main className="min-w-0 flex-1 space-y-5 px-8 py-6">
          <header>
            <h1 className="font-display text-2xl font-semibold text-text-primary">{courante.nom}</h1>
            <p className="mt-1 text-sm text-text-subtle">{courante.desc}</p>
          </header>

          <UpdateBanner update={updates.visible} onInstall={() => void updates.install()} onLater={updates.later} />

          {/* Les pages restent montées : changer de page ne perd ni un plan Forge ni une session Ledger. */}
          <div hidden={page !== "copilot"} className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <Card title="Personnage">
                <CharacterCard />
              </Card>
              <Card title="Raccourcis">
                <ul className="space-y-2 text-sm text-text-muted">
                  <li>
                    <Kbd>Ctrl+C</Kbd> sur un objet en jeu : ouvre l'overlay
                  </li>
                  <li>
                    <Kbd>Ctrl+Alt+C</Kbd> sur l'objet en cours de craft : prochaine étape du plan Forge
                  </li>
                  <li>
                    <Kbd>{afficherRaccourci(raccourcis.closeOverlay)}</Kbd> ferme l'overlay (actif seulement quand il est affiché)
                  </li>
                  <li>
                    <Kbd>{afficherRaccourci(raccourcis.upgradeSearch)}</Kbd> cherche un meilleur objet pour cet emplacement (ouvre le site de trade)
                  </li>
                </ul>
                <button type="button" onClick={() => ouvrir("reglages")} className="mt-2 text-xs text-accent hover:underline">
                  Modifier les raccourcis
                </button>
                <p className="mt-3 text-xs text-text-subtle">
                  Le jeu doit être en mode « fenêtré sans bordure » pour que l'overlay s'affiche par-dessus.
                </p>
              </Card>
            </div>
            <Card title="Dernier objet analysé">
              {dernier ? (
                <div className="space-y-2">
                  <p className="text-xs text-text-subtle">
                    Copié en jeu à {dernier.a.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" })}
                  </p>
                  <ItemTooltip item={dernier.item} />
                </div>
              ) : (
                <p className="text-sm text-text-subtle">
                  Aucun objet depuis l'ouverture d'Augur : fais <Kbd>Ctrl+C</Kbd> sur un objet en jeu, il s'affichera ici.
                </p>
              )}
            </Card>
          </div>

          <div hidden={page !== "forge"}>
            <ForgeCard ligue={settings?.league ?? null} onReglages={() => ouvrir("reglages")} />
          </div>

          <div hidden={page !== "ledger"}>
            <LedgerCard />
          </div>

          <div hidden={page !== "outils"} className="space-y-4">
            <Card title="Session de mesures">
              <MeasureCard />
            </Card>
            <Card title="Banc d'essai du parseur">
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <textarea
                    value={text}
                    onChange={(e) => setText(e.target.value)}
                    placeholder="Colle ici le texte d'un objet copié en jeu (Ctrl+C)."
                    className="h-64 w-full resize-none rounded-chip border border-ink-700 bg-ink-950 p-2 font-mono text-xs text-text-primary placeholder:text-text-subtle"
                  />
                  <button
                    type="button"
                    onClick={() => void tryParse()}
                    className="rounded-chip bg-accent px-3 py-1.5 text-sm font-semibold text-accent-ink hover:bg-accent-strong"
                  >
                    Analyser
                  </button>
                </div>
                <div>
                  {parsed && <ItemTooltip item={parsed} />}
                  {error && (
                    <StatePanel icon="alert" title="Objet non reconnu" variant="warn">
                      {error}
                    </StatePanel>
                  )}
                </div>
              </div>
            </Card>
          </div>

          <div hidden={page !== "reglages"} className="space-y-4">
            <Card title="Journal du jeu (Client.txt)">
              <ClientLogCard status={clientLog} settings={settings} onSettings={setSettings} />
            </Card>
            <Card title="Ligue souhaitée">
              <LeagueCard settings={settings} onSettings={setSettings} />
            </Card>
            <Card title="Budget de la recherche d'upgrades">
              <UpgradeBudgetCard settings={settings} onSettings={setSettings} />
            </Card>
            <Card title="Raccourcis de l'overlay">
              <ShortcutsCard settings={settings} onSettings={setSettings} />
            </Card>
            <Card title="Mises à jour">
              <div className="flex flex-wrap items-center gap-4 text-sm">
                <button
                  type="button"
                  onClick={() => void updates.check()}
                  disabled={updates.update.phase === "checking" || updates.update.phase === "installing"}
                  className="rounded-chip border border-ink-600 px-3 py-1.5 text-text-primary hover:bg-ink-800 disabled:opacity-50"
                >
                  Rechercher des mises à jour
                </button>
                <span className="text-text-subtle">
                  {updates.update.phase === "checking" && "Vérification…"}
                  {updates.update.phase === "upToDate" && "Augur est à jour."}
                  {updates.update.phase === "idle" && "Vérification automatique au démarrage puis toutes les 6 heures."}
                </span>
                {BETA_AVAILABLE && (
                  <label className="ml-auto flex items-center gap-2 text-text-muted">
                    <input
                      type="checkbox"
                      checked={settings?.betaChannel ?? false}
                      disabled={!settings}
                      onChange={(e) => void toggleBeta(e.target.checked)}
                      className="accent-[var(--color-accent)]"
                    />
                    Rejoindre la bêta
                  </label>
                )}
              </div>
              <p className="mt-2 text-xs text-text-subtle">
                Les mises à jour sont signées et vérifiées avant installation. Rien ne s'installe sans ton accord.
              </p>
            </Card>
          </div>
        </main>
      </div>
    </AffichageContext.Provider>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
