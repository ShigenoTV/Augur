import { StatePanel } from "@augur/ui";
import { useContext, useEffect, useMemo, useState } from "react";
import {
  commands,
  DEFAULT_LEAGUE,
  isAppError,
  isTauri,
  type ForgeAffix,
  type ForgeAffixes,
  type ForgeBase,
  type ForgeCatalogueEntry,
  type ForgeHistory,
  type ForgeHistoryEntry,
  type ForgePlanRequest,
  type ForgePlanResult,
  type ForgePrices,
  type ForgeSlot,
  type ForgeTracking,
  type ForgeWanted,
} from "../bridge";
import {
  SLOT_LABEL,
  basesParClasse,
  cleAffixe,
  dateReleve,
  decrireEtat,
  etapes,
  formatCount,
  formatEx,
  formatPercent,
  libelleCourt,
  lirePrix,
  peutAjouter,
} from "./forge";
import { DecisionTree } from "./DecisionTree";
import { AffichageContext, useCout } from "./devise";

const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold disabled:opacity-50";
const btnPrimary = `${btn} bg-accent text-accent-ink hover:bg-accent-strong`;
const btnSecondary = `${btn} border border-ink-600 text-text-primary hover:bg-ink-800`;
const input =
  "rounded-chip border border-ink-700 bg-ink-950 px-2 py-1 text-xs text-text-primary placeholder:text-text-subtle";
const TOUT_TIER = 255;
/** Exclut toutes les Essences d'un calcul (même identifiant côté Rust). */
const ESSENCES = "essences";

const message = (err: unknown) => (isAppError(err) ? err.message : String(err));

/** Chiffre clé de la bande en haut d'un résultat (comme l'ancienne appli de craft). */
function Stat({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="border-ink-700 px-4 py-3 [&:not(:last-child)]:border-r">
      <p className="text-xs text-text-subtle">{label}</p>
      <p className="font-display text-xl font-semibold text-accent">{value}</p>
      {hint && <p className="text-[11px] text-text-subtle">{hint}</p>}
    </div>
  );
}

/** Divine Orb : pas une monnaie de craft, seulement le taux d'affichage des coûts en Divine. */
const DIVINE: ForgeCatalogueEntry = { id: "divine", nom: "Divine Orb (affichage en Divine)", genre: "monnaie" };

/** Colonne d'affixes visables (préfixes ou suffixes) avec le tier minimum voulu. */
function AffixColumn({
  slot,
  affixes,
  voulus,
  onToggle,
  onTier,
}: {
  slot: ForgeSlot;
  affixes: ForgeAffix[];
  voulus: ForgeWanted[];
  onToggle: (a: ForgeAffix) => void;
  onTier: (a: ForgeAffix, tier: number) => void;
}) {
  const liste = affixes.filter((a) => a.slot === slot);
  return (
    <div className="min-w-0">
      <p className="mb-1 text-xs font-semibold text-text-muted">
        {SLOT_LABEL[slot]} <span className="font-normal text-text-subtle">({liste.length})</span>
      </p>
      <ul className="max-h-72 space-y-0.5 overflow-y-auto pr-1">
        {liste.map((a) => {
          const w = voulus.find((v) => cleAffixe(v) === cleAffixe(a));
          const libre = Boolean(w) || peutAjouter(affixes, voulus, a);
          return (
            <li
              key={cleAffixe(a)}
              className={`flex items-center gap-2 rounded-chip px-1.5 py-0.5 text-xs ${w ? "bg-ink-800" : ""}`}
            >
              <input
                type="checkbox"
                checked={Boolean(w)}
                disabled={!libre}
                onChange={() => onToggle(a)}
                className="accent-[var(--color-accent)]"
                aria-label={a.libelle}
              />
              <span className={`min-w-0 flex-1 truncate ${w ? "text-text-primary" : "text-text-muted"}`} title={a.libelle}>
                {libelleCourt(a.libelle)}
              </span>
              {w ? (
                <select
                  value={w.maxTier}
                  onChange={(e) => onTier(a, Number(e.target.value))}
                  className={`${input} py-0`}
                  aria-label="Tier minimum"
                >
                  <option value={TOUT_TIER}>tout tier</option>
                  {a.paliers.map((p) => (
                    <option key={p.tier} value={p.tier}>
                      T{p.tier} ou mieux
                    </option>
                  ))}
                </select>
              ) : (
                <span
                  className="font-mono text-[11px] text-text-subtle"
                  title={a.essence ? "Posé seulement par une Essence" : "Chance d'être le prochain mod de ce côté"}
                >
                  {a.essence ? "Essence" : formatPercent(a.part)}
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** Table des prix : corrections du joueur, relevé poe.ninja, valeurs indicatives. */
function PriceTable({
  prices,
  catalogue,
  ligue,
  onReglages,
  onPrices,
}: {
  prices: ForgePrices | null;
  catalogue: ForgeCatalogueEntry[];
  /** Ligue poe.ninja choisie dans Réglages. */
  ligue: string;
  onReglages?: () => void;
  onPrices: (p: ForgePrices) => void;
}) {
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  if (!prices) return null;
  const invalides = Object.entries(edits).filter(([, v]) => v.trim() !== "" && lirePrix(v) === null);
  const saisies = Object.fromEntries(
    Object.entries(edits)
      .map(([k, v]) => [k, lirePrix(v)] as const)
      .filter((e): e is readonly [string, number] => e[1] !== null),
  );

  async function run(f: () => Promise<ForgePrices>, ok: string) {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      onPrices(await f());
      setEdits({});
      setInfo(ok);
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }

  const essences = catalogue.filter((e) => e.genre === "essence");
  const ligne = (e: ForgeCatalogueEntry) => {
    const source = prices.sources[e.id] ?? "indicatif";
    const v = edits[e.id];
    return (
      <label key={e.id} className="flex items-center gap-2 py-0.5">
        <span className="min-w-0 flex-1 truncate text-text-muted" title={e.nom}>
          {e.nom}
        </span>
        {source === "joueur" && <span className="text-[10px] uppercase text-accent">corrigé</span>}
        <input
          value={v ?? ""}
          placeholder={prices.prix[e.id] === undefined ? "?" : formatEx(prices.prix[e.id]).replace(" ex", "")}
          onChange={(ev) => setEdits({ ...edits, [e.id]: ev.target.value })}
          className={`${input} w-20 text-right font-mono ${v !== undefined && v.trim() !== "" && lirePrix(v) === null ? "border-signal-loss" : ""}`}
          aria-label={`Prix de ${e.nom} en Exalted`}
        />
      </label>
    );
  };
  const marche = prices.marche;
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-end gap-2">
        <p className="flex flex-col gap-1 text-xs text-text-subtle">
          Ligue poe.ninja
          <span className="py-1.5 text-sm text-text-primary">
            {ligue}
            {onReglages && (
              <button type="button" onClick={onReglages} className="ml-2 text-xs text-accent hover:underline">
                changer dans Réglages
              </button>
            )}
          </span>
        </p>
        <button
          type="button"
          disabled={busy}
          onClick={() => void run(() => commands.refreshForgePrices(ligue), "Prix actualisés depuis poe.ninja.")}
          className={btnSecondary}
        >
          Actualiser depuis poe.ninja
        </button>
        <span className="text-xs text-text-subtle">
          {marche
            ? `Relevé ${marche.ligue} du ${dateReleve(marche.releveA)}${marche.manquants.length ? ` · sans cote : ${marche.manquants.length}` : ""}`
            : "Aucun relevé : valeurs indicatives de septembre 2026."}
        </span>
      </div>

      <div className="grid grid-cols-2 gap-x-6 gap-y-0.5 text-xs xl:grid-cols-3">
        {[...catalogue.filter((e) => e.genre !== "essence"), DIVINE].map(ligne)}
      </div>
      <details className="text-xs">
        <summary className="cursor-pointer text-text-subtle">
          Essences ({essences.filter((e) => prices.prix[e.id] !== undefined).length}/{essences.length} avec un prix)
        </summary>
        <div className="mt-2 grid grid-cols-2 gap-x-6 gap-y-0.5 xl:grid-cols-3">{essences.map(ligne)}</div>
      </details>

      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={busy || invalides.length > 0 || Object.keys(saisies).length === 0}
          onClick={() => void run(() => commands.setForgePrices(saisies), "Prix enregistrés.")}
          className={btnPrimary}
        >
          Enregistrer mes prix
        </button>
        <button
          type="button"
          disabled={busy || Object.keys(prices.corrections).length === 0}
          onClick={() => void run(() => commands.resetForgePrices(), "Corrections oubliées.")}
          className={btnSecondary}
        >
          Oublier mes corrections ({Object.keys(prices.corrections).length})
        </button>
        {info && <span className="text-xs text-signal-gain">{info}</span>}
        {error && <span className="text-xs text-signal-loss">{error}</span>}
      </div>
      <p className="text-xs text-text-subtle">
        Prix en Exalted Orbs. Ta saisie l'emporte sur poe.ninja, qui l'emporte sur les valeurs indicatives. La base neuve et sa
        revente n'ont pas de cote : saisis-les. Une Essence sans prix n'est pas proposée dans les plans.
      </p>
    </div>
  );
}

/** Plans calculés : rouvrir (nouveau calcul aux prix du moment) ou supprimer. */
function HistoryView({
  historique,
  busy,
  onReopen,
  onDelete,
}: {
  historique: ForgeHistory | null;
  busy: boolean;
  onReopen: (e: ForgeHistoryEntry) => void;
  onDelete: (e: ForgeHistoryEntry) => void;
}) {
  const cout = useCout();
  if (!historique || historique.entrees.length === 0) {
    return <p className="text-xs text-text-subtle">Aucun plan calculé pour l'instant : chaque calcul s'ajoute ici.</p>;
  }
  return (
    <div className="space-y-2">
      <table className="w-full text-xs">
        <thead className="text-left text-text-subtle">
          <tr>
            <th className="py-1 font-normal">Base</th>
            <th className="font-normal">Affixes voulus</th>
            <th className="text-right font-normal">Coût espéré</th>
            <th className="text-right font-normal">Calculé le</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {historique.entrees.map((e) => (
            <tr key={e.id} className="border-t border-ink-800 align-top">
              <td className="py-1 pr-2 text-text-primary">
                {e.baseNom} <span className="text-text-subtle">(niv. {e.demande.ilvl})</span>
                {historique.suivi === e.id && <span className="ml-1 text-[10px] uppercase text-accent">suivi</span>}
              </td>
              <td className="py-1 pr-2 text-text-muted">{e.labels.map(libelleCourt).join(", ")}</td>
              <td className="whitespace-nowrap py-1 pr-2 text-right font-mono text-text-primary">{cout(e.cout)}</td>
              <td className="whitespace-nowrap py-1 pr-2 text-right text-text-subtle">{dateReleve(e.calculeA)}</td>
              <td className="whitespace-nowrap py-1 text-right">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onReopen(e)}
                  className="rounded-chip border border-ink-600 px-2 py-0.5 text-text-primary hover:bg-ink-800 disabled:opacity-50"
                >
                  Rouvrir
                </button>{" "}
                <button
                  type="button"
                  onClick={() => onDelete(e)}
                  className="rounded-chip px-2 py-0.5 text-text-subtle hover:text-signal-loss"
                  aria-label={`Supprimer le plan ${e.baseNom}`}
                >
                  Supprimer
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="text-[11px] text-text-subtle">
        Rouvrir recalcule le plan aux prix du moment. Le plan suivi par l'overlay est gardé quand tu fermes Augur.
      </p>
    </div>
  );
}

/** Résultat d'un calcul : chiffres clés, puis arbre de décision, étapes ou liste de courses. */
function PlanView({ result }: { result: ForgePlanResult }) {
  const cout = useCout();
  const [vue, setVue] = useState<"arbre" | "etapes" | "courses">("arbre");
  const { plan } = result;
  const labels = plan.goal.map((g) => libelleCourt(g.label));
  const mc = plan.mc;
  const vues = [
    ["arbre", "Arbre de décision"],
    ["etapes", "Étapes"],
    ["courses", "Liste de courses"],
  ] as const;
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-4 rounded-panel border border-ink-700 bg-ink-950">
        <Stat label="Coût moyen" value={cout(plan.expectedCost + plan.baseCost)} hint={`base neuve comprise (${cout(plan.baseCost)})`} />
        <Stat label="Une fois sur deux" value={cout(mc?.medianCost)} hint={mc ? `médiane de ${formatCount(mc.trials)} crafts simulés` : undefined} />
        <Stat label="Budget sûr (9 sur 10)" value={cout(mc?.p90Cost)} hint={mc ? `99 sur 100 : ${cout(mc.p99Cost)}` : undefined} />
        <Stat
          label="Abandons"
          value={mc ? formatCount(mc.meanAbandons) : "–"}
          hint={mc ? `${formatCount(mc.meanSteps)} étapes en moyenne` : undefined}
        />
      </div>

      {!result.complet && (
        <StatePanel icon="alert" title="Coût approximatif" variant="warn">
          {result.sansPoids} mod(s) de cette base n'ont pas de poids connu et sont écartés du calcul : le coût réel peut différer.
        </StatePanel>
      )}

      <div className="flex gap-5 border-b border-ink-700 text-sm">
        {vues.map(([id, nom]) => (
          <button
            key={id}
            type="button"
            onClick={() => setVue(id)}
            className={`-mb-px border-b-2 pb-1.5 ${vue === id ? "border-accent font-semibold text-accent" : "border-transparent text-text-muted hover:text-text-primary"}`}
          >
            {nom}
          </button>
        ))}
      </div>

      {vue === "arbre" && <DecisionTree plan={plan} />}

      {vue === "etapes" && (
        <table className="w-full text-xs">
          <thead className="text-left text-text-subtle">
            <tr>
              <th className="py-1 font-normal">Objet</th>
              <th className="font-normal">Action</th>
              <th className="text-right font-normal" title="Coût espéré pour finir depuis cet état">Reste</th>
            </tr>
          </thead>
          <tbody>
            {etapes(plan).map((n) => (
              <tr key={n.id} className="border-t border-ink-800 align-top">
                <td className="py-1 pr-2 text-text-muted">{decrireEtat(n.state, labels)}</td>
                <td className="py-1 pr-2 text-text-primary">
                  {n.action.isAbandon ? "Repartir d'une base neuve" : n.action.label}
                  {n.repeat && n.repeat.expectedAttempts > 1.05 && (
                    <span className="text-text-subtle"> · ~{formatCount(n.repeat.expectedAttempts)} essais</span>
                  )}
                </td>
                <td className="whitespace-nowrap py-1 text-right font-mono text-text-muted">{cout(n.costToGo)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {vue === "courses" && (
        <table className="w-full max-w-xl text-xs">
          <thead className="text-left text-text-subtle">
            <tr>
              <th className="py-1 font-normal">À acheter (en moyenne)</th>
              <th className="text-right font-normal">Quantité</th>
              <th className="text-right font-normal">Coût</th>
            </tr>
          </thead>
          <tbody>
            {plan.shopping.map((l) => (
              <tr key={l.id} className="border-t border-ink-800">
                <td className="py-1 pr-2 text-text-muted">{l.label}</td>
                <td className="whitespace-nowrap py-1 pr-2 text-right font-mono text-text-subtle">×{formatCount(l.expectedCount)}</td>
                <td className="whitespace-nowrap py-1 text-right font-mono text-text-primary">{cout(l.expectedCost)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <p className="text-[11px] text-text-subtle">
        Calcul : {formatCount(plan.solver.states)} états, {plan.solver.millis} ms
        {mc ? `, vérifié sur ${formatCount(mc.trials)} crafts simulés (moyenne ${cout(mc.meanCost)})` : ""}. Corrige les
        prix dans l'onglet Prix.
      </p>
    </div>
  );
}

/**
 * Forge : choisir une base et un objectif, obtenir la stratégie au meilleur coût espéré (solveur
 * exact du cœur Rust) avec ses quantiles et ses achats.
 */
export function ForgeCard({ ligue, onReglages }: { ligue: string | null; onReglages?: () => void }) {
  const [bases, setBases] = useState<ForgeBase[]>([]);
  const [catalogue, setCatalogue] = useState<ForgeCatalogueEntry[]>([]);
  const [prices, setPrices] = useState<ForgePrices | null>(null);
  const [classe, setClasse] = useState("");
  const [baseId, setBaseId] = useState("");
  const [ilvl, setIlvl] = useState(82);
  const [affixes, setAffixes] = useState<ForgeAffixes | null>(null);
  const [voulus, setVoulus] = useState<ForgeWanted[]>([]);
  const [exclues, setExclues] = useState<string[]>([]);
  const [result, setResult] = useState<ForgePlanResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [onglet, setOnglet] = useState<"plan" | "prix" | "historique">("plan");
  const [suivi, setSuivi] = useState<ForgeTracking | null>(null);
  const [historique, setHistorique] = useState<ForgeHistory | null>(null);
  const { majTaux } = useContext(AffichageContext);

  // Taux d'affichage Chaos / Divine : ceux de la table de prix (partagés avec l'overlay).
  useEffect(() => {
    if (prices) majTaux(prices.prix);
    // majTaux change à chaque rendu : seuls les prix déclenchent la mise à jour.
  }, [prices]);

  useEffect(() => {
    if (!isTauri()) return;
    void commands.getForgeBases().then(setBases).catch((e) => setError(message(e)));
    void commands.getForgeCatalogue().then(setCatalogue).catch(() => undefined);
    void commands.getForgePrices().then(setPrices).catch(() => undefined);
    void commands.getForgeTracking().then(setSuivi).catch(() => undefined);
    void commands.getForgeHistory().then(setHistorique).catch(() => undefined);
    // Le plan suivi avant la fermeture est recalculé au démarrage : on relit le suivi un peu après.
    const timer = setTimeout(() => {
      void commands.getForgeTracking().then(setSuivi).catch(() => undefined);
    }, 4000);
    return () => clearTimeout(timer);
  }, []);

  const classes = useMemo(() => basesParClasse(bases), [bases]);
  const basesDeClasse = classes.find(([c]) => c === classe)?.[1] ?? [];

  useEffect(() => {
    setAffixes(null);
    setResult(null);
    if (!baseId || !isTauri()) return;
    const timer = setTimeout(() => {
      commands
        .getForgeAffixes(baseId, ilvl)
        .then((a) => {
          setAffixes(a);
          // On garde les voulus encore présents à ce niveau d'objet.
          setVoulus((v) => v.filter((w) => a.affixes.some((x) => cleAffixe(x) === cleAffixe(w))));
        })
        .catch((e) => setError(message(e)));
    }, 250);
    return () => clearTimeout(timer);
  }, [baseId, ilvl]);

  function toggle(a: ForgeAffix) {
    setResult(null);
    setVoulus((v) =>
      v.some((w) => cleAffixe(w) === cleAffixe(a))
        ? v.filter((w) => cleAffixe(w) !== cleAffixe(a))
        : [...v, { group: a.group, family: a.family, maxTier: TOUT_TIER }],
    );
  }

  function setTier(a: ForgeAffix, tier: number) {
    setResult(null);
    setVoulus((v) => v.map((w) => (cleAffixe(w) === cleAffixe(a) ? { ...w, maxTier: tier } : w)));
  }

  async function calculer(demande: ForgePlanRequest = { baseId, ilvl, voulus, exclues }) {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await commands.planForgeCraft(demande));
      setSuivi(await commands.getForgeTracking());
      setHistorique(await commands.getForgeHistory());
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }

  function rouvrir(e: ForgeHistoryEntry) {
    const b = bases.find((x) => x.id === e.demande.baseId);
    if (b) setClasse(b.classe);
    setBaseId(e.demande.baseId);
    setIlvl(e.demande.ilvl);
    setVoulus(e.demande.voulus);
    setExclues(e.demande.exclues);
    setOnglet("plan");
    void calculer(e.demande);
  }

  async function supprimer(e: ForgeHistoryEntry) {
    try {
      setHistorique(await commands.deleteForgeHistoryEntry(e.id));
      setSuivi(await commands.getForgeTracking());
    } catch (err) {
      setError(message(err));
    }
  }

  if (!isTauri()) {
    return <p className="text-sm text-text-subtle">Forge fonctionne dans l'application.</p>;
  }

  const monnaies = [
    ...catalogue.filter((c) => c.genre === "monnaie" || c.genre === "omen"),
    { id: ESSENCES, nom: "Essences (avec un prix)", genre: "essence" as const },
  ];
  return (
    <div className="space-y-4">
      <div className="flex gap-1 text-sm">
        {(["plan", "prix", "historique"] as const).map((o) => (
          <button
            key={o}
            type="button"
            onClick={() => setOnglet(o)}
            className={`rounded-chip px-3 py-1 ${onglet === o ? "bg-ink-800 text-text-primary" : "text-text-subtle hover:text-text-primary"}`}
          >
            {o === "plan" ? "Plan de craft" : o === "prix" ? "Prix" : `Historique (${historique?.entrees.length ?? 0})`}
          </button>
        ))}
      </div>

      {suivi && (
        <div className="flex flex-wrap items-center gap-2 rounded-chip border border-ink-700 bg-ink-950 px-3 py-2 text-xs">
          <span className="text-text-muted">
            L'overlay suit le plan <span className="text-text-primary">{suivi.baseNom}</span> (niv. {suivi.ilvl} :{" "}
            {suivi.labels.map(libelleCourt).join(", ")}). En jeu, <span className="font-mono">Ctrl+Alt+C</span> sur l'objet
            en cours de craft affiche la prochaine étape.
          </span>
          <button
            type="button"
            onClick={() =>
              void commands
                .stopForgeTracking()
                .then(() => setSuivi(null))
                .then(() => commands.getForgeHistory().then(setHistorique))
            }
            className="ml-auto rounded-chip border border-ink-600 px-2 py-0.5 text-text-primary hover:bg-ink-800"
          >
            Arrêter le suivi
          </button>
        </div>
      )}

      {onglet === "historique" ? (
        <HistoryView historique={historique} busy={busy} onReopen={rouvrir} onDelete={(e) => void supprimer(e)} />
      ) : onglet === "prix" ? (
        <PriceTable
          prices={prices}
          catalogue={catalogue}
          ligue={ligue ?? DEFAULT_LEAGUE}
          onReglages={onReglages}
          onPrices={(p) => {
            setPrices(p);
            // Le plan affiché a été calculé avec les anciens prix.
            setResult(null);
          }}
        />
      ) : (
        <>
          <div className="flex flex-wrap items-end gap-2">
            <label className="flex flex-col gap-1 text-xs text-text-subtle">
              Type d'objet
              <select
                value={classe}
                onChange={(e) => {
                  setClasse(e.target.value);
                  setBaseId("");
                  setVoulus([]);
                }}
                className={input}
              >
                <option value="">Choisir…</option>
                {classes.map(([c]) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs text-text-subtle">
              Base
              <select
                value={baseId}
                disabled={!classe}
                onChange={(e) => {
                  setBaseId(e.target.value);
                  setVoulus([]);
                }}
                className={`${input} w-56`}
              >
                <option value="">Choisir…</option>
                {basesDeClasse.map((b) => (
                  <option key={b.id} value={b.id}>
                    {b.nom} (niv. {b.niveauDrop})
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs text-text-subtle">
              Niveau d'objet
              <input
                type="number"
                min={1}
                max={100}
                value={ilvl}
                onChange={(e) => setIlvl(Math.min(100, Math.max(1, Number(e.target.value) || 1)))}
                className={`${input} w-20`}
              />
            </label>
            <span className="pb-1 text-xs text-text-subtle">{bases.length} bases avec des poids connus</span>
          </div>

          {affixes && (
            <>
              <div className="grid grid-cols-2 gap-4">
                <AffixColumn slot="prefix" affixes={affixes.affixes} voulus={voulus} onToggle={toggle} onTier={setTier} />
                <AffixColumn slot="suffix" affixes={affixes.affixes} voulus={voulus} onToggle={toggle} onTier={setTier} />
              </div>
              {affixes.sansPoids.length > 0 && (
                <p className="text-xs text-signal-warn">
                  {affixes.sansPoids.length} mod(s) éligibles sans poids connu : ils sont écartés, les
                  probabilités sont approximatives.
                </p>
              )}
              <details className="text-xs text-text-muted">
                <summary className="cursor-pointer text-text-subtle">
                  Monnaies autorisées ({monnaies.length - exclues.length}/{monnaies.length})
                </summary>
                <div className="mt-2 grid grid-cols-3 gap-x-4 gap-y-0.5">
                  {monnaies.map((m) => (
                    <label key={m.id} className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={!exclues.includes(m.id)}
                        onChange={() => {
                          setResult(null);
                          setExclues((x) => (x.includes(m.id) ? x.filter((i) => i !== m.id) : [...x, m.id]));
                        }}
                        className="accent-[var(--color-accent)]"
                      />
                      {m.nom}
                    </label>
                  ))}
                </div>
              </details>
              <div className="flex items-center gap-3">
                <button type="button" disabled={busy || voulus.length === 0} onClick={() => void calculer()} className={btnPrimary}>
                  {busy ? "Calcul en cours…" : "Calculer le plan"}
                </button>
                <span className="text-xs text-text-subtle">
                  {voulus.length === 0 ? "Coche les affixes voulus (3 préfixes et 3 suffixes au plus)." : `${voulus.length} affixe(s) voulu(s), depuis une base neuve.`}
                </span>
              </div>
            </>
          )}

          {error && (
            <StatePanel icon="alert" title="Calcul impossible" variant="warn">
              {error}
            </StatePanel>
          )}
          {result && <PlanView result={result} />}
        </>
      )}
    </div>
  );
}
