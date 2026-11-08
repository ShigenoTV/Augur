import { StatePanel } from "@augur/ui";
import { useEffect, useMemo, useState } from "react";
import {
  commands,
  isAppError,
  isTauri,
  type ForgeAffix,
  type ForgeAffixes,
  type ForgeBase,
  type ForgeCatalogueEntry,
  type ForgePlanResult,
  type ForgePrices,
  type ForgeSlot,
  type ForgeTracking,
  type ForgeWanted,
} from "../bridge";
import {
  SLOT_LABEL,
  SOURCE_LABEL,
  basesParClasse,
  cleAffixe,
  dateReleve,
  decrireEtat,
  etapes,
  formatCount,
  formatEx,
  formatPercent,
  libelleCourt,
  sourcesAchat,
  lirePrix,
  peutAjouter,
} from "./forge";

const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold disabled:opacity-50";
const btnPrimary = `${btn} bg-accent text-accent-ink hover:bg-accent-strong`;
const btnSecondary = `${btn} border border-ink-600 text-text-primary hover:bg-ink-800`;
const input =
  "rounded-chip border border-ink-700 bg-ink-950 px-2 py-1 text-xs text-text-primary placeholder:text-text-subtle";
const TOUT_TIER = 255;

const message = (err: unknown) => (isAppError(err) ? err.message : String(err));

function Stat({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="rounded-chip border border-ink-700 bg-ink-950 px-3 py-2">
      <p className="text-xs text-text-subtle">{label}</p>
      <p className="font-mono text-base text-text-primary">{value}</p>
      {hint && <p className="text-[11px] text-text-subtle">{hint}</p>}
    </div>
  );
}

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
                <span className="font-mono text-[11px] text-text-subtle" title="Chance d'être le prochain mod de ce côté">
                  {formatPercent(a.part)}
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
  onPrices,
}: {
  prices: ForgePrices | null;
  catalogue: ForgeCatalogueEntry[];
  onPrices: (p: ForgePrices) => void;
}) {
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [ligue, setLigue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  useEffect(() => {
    if (prices?.marche && !ligue) setLigue(prices.marche.ligue);
  }, [prices, ligue]);

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

  const marche = prices.marche;
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs text-text-subtle">
          Ligue poe.ninja
          <input
            value={ligue}
            onChange={(e) => setLigue(e.target.value)}
            placeholder="Ligue en cours"
            className={`${input} w-48`}
          />
        </label>
        <button
          type="button"
          disabled={busy}
          onClick={() => void run(() => commands.refreshForgePrices(ligue.trim() || null), "Prix actualisés depuis poe.ninja.")}
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
        {catalogue.map((e) => {
          const source = prices.sources[e.id] ?? "indicatif";
          const v = edits[e.id];
          return (
            <label key={e.id} className="flex items-center gap-2 py-0.5">
              <span className="min-w-0 flex-1 truncate text-text-muted" title={e.nom}>
                {e.nom}
              </span>
              <span
                className={`text-[10px] uppercase ${source === "joueur" ? "text-accent" : source === "poeNinja" ? "text-signal-info" : "text-text-subtle"}`}
              >
                {SOURCE_LABEL[source]}
              </span>
              <input
                value={v ?? ""}
                placeholder={formatEx(prices.prix[e.id]).replace(" ex", "")}
                onChange={(ev) => setEdits({ ...edits, [e.id]: ev.target.value })}
                className={`${input} w-20 text-right font-mono ${v !== undefined && v.trim() !== "" && lirePrix(v) === null ? "border-signal-loss" : ""}`}
                aria-label={`Prix de ${e.nom} en Exalted`}
              />
            </label>
          );
        })}
      </div>

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
        revente n'ont pas de cote : saisis-les.
      </p>
    </div>
  );
}

/** Résultat d'un calcul : coût espéré, quantiles, étapes du plan, achats et sources. */
function PlanView({ result }: { result: ForgePlanResult }) {
  const { plan } = result;
  const labels = plan.goal.map((g) => libelleCourt(g.label));
  const mc = plan.mc;
  const sources = result.prix.reduce<Record<string, number>>((acc, p) => {
    acc[p.source] = (acc[p.source] ?? 0) + 1;
    return acc;
  }, {});
  const sourceLigne = (id: string) => sourcesAchat(id, result.prix);
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-4 gap-2">
        <Stat label="Coût espéré" value={formatEx(plan.expectedCost)} hint="hors base de départ" />
        <Stat label="Médiane (P50)" value={formatEx(mc?.medianCost)} hint={mc ? `${formatCount(mc.trials)} crafts simulés` : undefined} />
        <Stat label="Malchanceux (P90)" value={formatEx(mc?.p90Cost)} hint="9 crafts sur 10 coûtent moins" />
        <Stat label="Base de départ" value={formatEx(plan.baseCost)} hint={mc ? `${formatCount(mc.meanAbandons)} abandon(s) en moyenne` : undefined} />
      </div>

      {!result.complet && (
        <StatePanel icon="alert" title="Coût approximatif" variant="warn">
          {result.sansPoids} mod(s) de cette base n'ont pas de poids connu et sont écartés du calcul : le coût réel peut différer.
        </StatePanel>
      )}

      <div className="grid grid-cols-[3fr_2fr] gap-6">
        <div>
          <p className="mb-1 text-xs font-semibold text-text-muted">Que faire selon l'état de l'objet</p>
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
                  <td className="whitespace-nowrap py-1 text-right font-mono text-text-muted">{formatEx(n.costToGo)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div>
          <p className="mb-1 text-xs font-semibold text-text-muted">Achats moyens</p>
          <table className="w-full text-xs">
            <tbody>
              {plan.shopping.map((s) => (
                <tr key={s.id} className="border-t border-ink-800">
                  <td className="py-1 pr-2 text-text-muted">
                    {s.label}
                    <span className="block text-[10px] uppercase text-text-subtle">{sourceLigne(s.id)}</span>
                  </td>
                  <td className="whitespace-nowrap py-1 pr-2 text-right font-mono text-text-subtle">×{formatCount(s.expectedCount)}</td>
                  <td className="whitespace-nowrap py-1 text-right font-mono text-text-primary">{formatEx(s.expectedCost)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="space-y-1 text-[11px] text-text-subtle">
        <p>
          Prix des achats : {Object.entries(sources).map(([s, n]) => `${n} ${SOURCE_LABEL[s as keyof typeof SOURCE_LABEL]}`).join(", ")}
          {result.ligue && sources.poeNinja ? ` (ligue ${result.ligue})` : ""}. Corrige-les dans l'onglet Prix.
        </p>
        <p>
          Probabilités : poids <span className="text-text-muted">estimés</span> par {result.attribution.source}, licence{" "}
          {result.attribution.licence}, relevé du {result.attribution.dateReleve} ; mods et paliers{" "}
          <span className="text-text-muted">extraits</span> des fichiers du jeu (RePoE {result.attribution.repoeVersion}).
          Calcul : {formatCount(plan.solver.states)} états, {plan.solver.millis} ms
          {mc ? `, vérifié sur ${formatCount(mc.trials)} crafts simulés (moyenne ${formatEx(mc.meanCost)})` : ""}.
        </p>
      </div>
    </div>
  );
}

/**
 * Forge : choisir une base et un objectif, obtenir la stratégie au meilleur coût espéré (solveur
 * exact du cœur Rust) avec ses quantiles, ses achats et la source de chaque prix.
 */
export function ForgeCard() {
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
  const [onglet, setOnglet] = useState<"plan" | "prix">("plan");
  const [suivi, setSuivi] = useState<ForgeTracking | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    void commands.getForgeBases().then(setBases).catch((e) => setError(message(e)));
    void commands.getForgeCatalogue().then(setCatalogue).catch(() => undefined);
    void commands.getForgePrices().then(setPrices).catch(() => undefined);
    void commands.getForgeTracking().then(setSuivi).catch(() => undefined);
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

  async function calculer() {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await commands.planForgeCraft({ baseId, ilvl, voulus, exclues }));
      setSuivi(await commands.getForgeTracking());
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }

  if (!isTauri()) {
    return <p className="text-sm text-text-subtle">Forge fonctionne dans l'application.</p>;
  }

  const monnaies = catalogue.filter((c) => c.genre !== "base");
  return (
    <div className="space-y-4">
      <div className="flex gap-1 text-sm">
        {(["plan", "prix"] as const).map((o) => (
          <button
            key={o}
            type="button"
            onClick={() => setOnglet(o)}
            className={`rounded-chip px-3 py-1 ${onglet === o ? "bg-ink-800 text-text-primary" : "text-text-subtle hover:text-text-primary"}`}
          >
            {o === "plan" ? "Plan de craft" : "Prix"}
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
            onClick={() => void commands.stopForgeTracking().then(() => setSuivi(null))}
            className="ml-auto rounded-chip border border-ink-600 px-2 py-0.5 text-text-primary hover:bg-ink-800"
          >
            Arrêter le suivi
          </button>
        </div>
      )}

      {onglet === "prix" ? (
        <PriceTable
          prices={prices}
          catalogue={catalogue}
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
                  {affixes.sansPoids.length} mod(s) éligibles sans poids connu chez poe2db : ils sont écartés, les
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
