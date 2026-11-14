import { StatePanel } from "@augur/ui";
import { useEffect, useMemo, useState } from "react";
import {
  commands,
  isAppError,
  isTauri,
  type ForgeAffix,
  type ForgePoe1Affixes,
  type ForgePoe1Base,
  type ForgePoe1Plan,
  type ForgePoe1Prices,
  type ForgePoe1Request,
  type ForgePoe1Result,
  type ForgeWanted,
} from "../bridge";
import { AffixColumn } from "./ForgeCard";
import { cleAffixe, libelleCourt, lirePrix } from "./forge";
import {
  VOULUS_MAX_PLAN,
  type SituationPoe1,
  chiffrer,
  decisions,
  formatChance,
  formatChaos,
  formatEssais,
  libelleBase,
  prixEffectif,
  prixPourPlan,
  trouverEtape,
} from "./forgePoe1";

const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold disabled:opacity-50";
const btnPrimary = `${btn} bg-accent text-accent-ink hover:bg-accent-strong`;
const btnSecondary = `${btn} border border-ink-600 text-text-primary hover:bg-ink-800`;
const input =
  "rounded-chip border border-ink-700 bg-ink-950 px-2 py-1 text-xs text-text-primary placeholder:text-text-subtle";
const TOUT_TIER = 255;
const CLE_PRIX = "augur.forgePoe1.prix";

const message = (err: unknown) => (isAppError(err) ? err.message : String(err));

function lirePrixSaisis(): Record<string, number> {
  try {
    const v = JSON.parse(localStorage.getItem(CLE_PRIX) ?? "{}");
    return v && typeof v === "object" ? (v as Record<string, number>) : {};
  } catch {
    return {};
  }
}

function garderPrixSaisis(p: Record<string, number>) {
  try {
    localStorage.setItem(CLE_PRIX, JSON.stringify(p));
  } catch {
    // Stockage indisponible : les saisies durent jusqu'à la fermeture.
  }
}

/** Monnaies utilisées par le résultat, avec leur cote et la saisie du joueur (aucune source affichée). */
function PrixUtilises({
  noms,
  prix,
  saisis,
  onSaisie,
}: {
  noms: string[];
  prix: ForgePoe1Prices | null;
  saisis: Record<string, number>;
  onSaisie: (nom: string, v: number | null) => void;
}) {
  return (
    <details className="text-xs text-text-muted">
      <summary className="cursor-pointer text-text-subtle">
        Prix utilisés ({prix ? `cote du marché, ligue ${prix.league}` : "cote en cours de chargement"})
      </summary>
      {prix?.erreur && <p className="mt-1 text-signal-warn">Cote incomplète : {prix.erreur}</p>}
      <table className="mt-2 w-full max-w-xl">
        <thead className="text-left text-text-subtle">
          <tr>
            <th className="font-normal">Monnaie</th>
            <th className="font-normal">Cote</th>
            <th className="font-normal">Ton prix (Chaos)</th>
          </tr>
        </thead>
        <tbody>
          {noms.map((nom) => (
            <tr key={nom}>
              <td className="py-0.5 pr-3 text-text-primary">{nom}</td>
              <td className="pr-3 font-mono">{formatChaos(prix?.prix[nom] ?? null)}</td>
              <td>
                <input
                  defaultValue={saisis[nom] != null ? String(saisis[nom]) : ""}
                  placeholder="auto"
                  onBlur={(e) => onSaisie(nom, lirePrix(e.target.value))}
                  className={`${input} w-24`}
                  aria-label={`Prix de ${nom}`}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </details>
  );
}

const SITUATIONS_MONTREES = 4;

/** Pas à pas : quoi faire selon l'objet obtenu, en règles puis pour l'objet du joueur. */
function PasAPas({ plan, noms, divine }: { plan: ForgePoe1Plan; noms: string[]; divine: number | null }) {
  const regles = useMemo(() => decisions(plan.etapes, noms), [plan, noms]);
  const metamods = useMemo(
    () => [...new Set(plan.etapes.flatMap((e) => (e.metamod ? [e.metamod] : [])))].sort(),
    [plan],
  );
  const [situation, setSituation] = useState<SituationPoe1>({
    rarete: "Normal",
    voulus: noms.map(() => "absent"),
    autresPrefixes: 0,
    autresSuffixes: 0,
    metamod: null,
  });
  const etape = trouverEtape(plan.etapes, situation);
  const maj = (p: Partial<SituationPoe1>) => setSituation((s) => ({ ...s, ...p }));
  const compte = (valeur: number, onChange: (n: number) => void, label: string) => (
    <label className="flex items-center gap-1">
      {label}
      <select value={valeur} onChange={(e) => onChange(Number(e.target.value))} className={input} aria-label={label}>
        {[0, 1, 2, 3].map((n) => (
          <option key={n} value={n}>
            {n}
          </option>
        ))}
      </select>
    </label>
  );
  return (
    <details className="text-xs text-text-muted">
      <summary className="cursor-pointer text-text-subtle">Pas à pas : quoi faire selon l'objet obtenu</summary>
      <ul className="mt-2 space-y-1.5">
        {regles.map((r) => (
          <li key={`${r.rarete}-${r.action}`}>
            <span className="text-text-subtle">Objet {r.rarete} : </span>
            <span className="font-semibold text-text-primary">{r.action}</span>
            {r.parDefaut ? (
              <span> dans les autres cas.</span>
            ) : r.rarete === "Normal" ? null : (
              <>
                <span> si </span>
                {r.situations.slice(0, SITUATIONS_MONTREES).join(" ; ")}
                {r.situations.length > SITUATIONS_MONTREES &&
                  ` ; et ${r.situations.length - SITUATIONS_MONTREES} autre(s) cas`}
                .
              </>
            )}
          </li>
        ))}
      </ul>
      <div className="mt-3 space-y-2 rounded-card border border-ink-700 p-2">
        <p className="text-text-subtle">Où en est ton objet ?</p>
        <div className="flex flex-wrap items-center gap-3">
          <label className="flex items-center gap-1">
            Rareté
            <select
              value={situation.rarete}
              onChange={(e) => maj({ rarete: e.target.value as SituationPoe1["rarete"] })}
              className={input}
              aria-label="Rareté"
            >
              <option value="Normal">Normal</option>
              <option value="Magique">Magique</option>
              <option value="Rare">Rare</option>
            </select>
          </label>
          {noms.map((nom, i) => (
            <label key={i} className="flex items-center gap-1">
              {nom}
              <select
                value={situation.voulus[i]}
                onChange={(e) =>
                  maj({
                    voulus: situation.voulus.map((v, j) =>
                      j === i ? (e.target.value as SituationPoe1["voulus"][number]) : v,
                    ),
                  })
                }
                className={input}
                aria-label={nom}
              >
                <option value="absent">absent</option>
                <option value="present">présent</option>
                <option value="bas">tier trop bas</option>
              </select>
            </label>
          ))}
          {compte(situation.autresPrefixes, (n) => maj({ autresPrefixes: n }), "Autres préfixes")}
          {compte(situation.autresSuffixes, (n) => maj({ autresSuffixes: n }), "Autres suffixes")}
          {metamods.length > 0 && (
            <label className="flex items-center gap-1">
              Metamod
              <select
                value={situation.metamod ?? ""}
                onChange={(e) => maj({ metamod: e.target.value || null })}
                className={input}
                aria-label="Metamod"
              >
                <option value="">aucun</option>
                {metamods.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
        {etape ? (
          <p className="text-sm">
            À faire : <span className="font-semibold text-accent">{etape.action}</span>
            <span className="text-xs text-text-muted">
              {" "}
              (reste environ {formatChaos(etape.reste, divine)} en moyenne)
            </span>
          </p>
        ) : (
          <p className="text-text-subtle">
            Le plan ne passe presque jamais par cette situation : vérifie l'objet, ou repars d'un objet Normal.
          </p>
        )}
      </div>
    </details>
  );
}

/** Plan au coût moyen minimal, quand le solveur en a trouvé un. */
function PlanConseille({
  plan,
  noms,
  spam,
  divine,
  prixChanges,
}: {
  plan: ForgePoe1Plan | null;
  noms: string[];
  spam: number | null;
  divine: number | null;
  prixChanges: boolean;
}) {
  if (!plan) {
    return (
      <p className="text-[11px] text-text-subtle">
        {noms.length > VOULUS_MAX_PLAN
          ? `Plan conseillé (Exalted, Annulment, metamods) : jusqu'à ${VOULUS_MAX_PLAN} mods voulus pour l'instant.`
          : "Pas de plan conseillé : des prix manquent, ou l'objet est hors d'atteinte."}
      </p>
    );
  }
  const gain = spam != null && plan.coutMoyen < spam * 0.98 ? spam / plan.coutMoyen : null;
  return (
    <section className="space-y-2 rounded-card border border-accent/40 bg-ink-900 p-3">
      <p className="text-sm text-text-muted">
        Plan conseillé : environ{" "}
        <span className="font-semibold text-accent">{formatChaos(plan.coutMoyen, divine)}</span> en moyenne
        {gain != null && gain >= 1.05
          ? `, ${gain.toLocaleString("fr-FR", { maximumFractionDigits: 1 })} fois moins que le meilleur spam`
          : ""}
        .
      </p>
      <p className="text-xs text-text-muted">
        Commence par : <span className="text-text-primary">{plan.depart}</span>. Ensuite, à chaque étape, la meilleure
        action selon l'objet obtenu.
      </p>
      <table className="text-xs">
        <thead className="text-left text-text-subtle">
          <tr>
            <th className="pr-4 font-normal">Action</th>
            <th className="font-normal">Nombre moyen</th>
          </tr>
        </thead>
        <tbody>
          {plan.actions.map((a) => (
            <tr key={a.action}>
              <td className="py-0.5 pr-4 text-text-primary">{a.action}</td>
              <td className="font-mono">{formatEssais(a.nombre)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {plan.etapes.length > 0 && <PasAPas plan={plan} noms={noms} divine={divine} />}
      {plan.sansPrix.length > 0 && (
        <p className="text-[11px] text-text-subtle">Actions écartées faute de prix : {plan.sansPrix.join(", ")}.</p>
      )}
      {prixChanges && (
        <p className="text-[11px] text-signal-warn">
          Un prix a changé depuis le calcul : relance « Comparer les méthodes ».
        </p>
      )}
    </section>
  );
}

function Resultat({
  result,
  prix,
  saisis,
  onSaisie,
  nomsVoulus,
  prixChanges,
}: {
  result: ForgePoe1Result;
  prix: ForgePoe1Prices | null;
  saisis: Record<string, number>;
  onSaisie: (nom: string, v: number | null) => void;
  /** Noms courts des mods voulus, dans l'ordre de la demande. */
  nomsVoulus: string[];
  prixChanges: boolean;
}) {
  const lignes = chiffrer(result.methodes, prix?.prix ?? {}, saisis);
  const divine = prixEffectif("Divine Orb", prix?.prix ?? {}, saisis);
  const noms = [
    ...new Set([
      ...result.methodes.flatMap((m) => [...m.monnaies, ...(m.finition?.monnaies ?? [])].map((c) => c.nom)),
      ...(result.plan?.monnaies.map((c) => c.nom) ?? []),
      ...(result.plan?.sansPrix ?? []),
    ]),
  ].sort();
  const meilleure = lignes[0];
  return (
    <div className="space-y-3">
      <PlanConseille
        plan={result.plan}
        noms={nomsVoulus}
        spam={meilleure?.coutMoyen ?? null}
        divine={divine}
        prixChanges={prixChanges}
      />
      {lignes.length === 0 ? (
        <StatePanel icon="flat" title="Aucune méthode de spam ne mène à cet objet">
          Essaie un tier plus bas, moins d'affixes, ou un niveau d'objet plus haut.
        </StatePanel>
      ) : (
        <>
          {meilleure && meilleure.coutMoyen != null && (
            <p className="text-sm text-text-muted">
              Méthode la moins chère : <span className="font-semibold text-text-primary">{meilleure.nom}</span>, environ{" "}
              <span className="font-semibold text-accent">{formatChaos(meilleure.coutMoyen, divine)}</span> en moyenne (
              {formatEssais(meilleure.essaisMoyens)} essais).
            </p>
          )}
          <table className="w-full text-xs">
            <thead className="text-left text-text-subtle">
              <tr className="border-b border-ink-700">
                <th className="py-1 font-normal">Méthode</th>
                <th className="font-normal">Chance par essai</th>
                <th className="font-normal">Coût d'un essai</th>
                <th className="font-normal">Coût moyen</th>
                <th className="font-normal" title="Essais pour réussir une fois sur deux, puis neuf fois sur dix">
                  Essais (médian / 9 sur 10)
                </th>
              </tr>
            </thead>
            <tbody>
              {lignes.map((m) => (
                <tr key={m.id} className="border-b border-ink-800 align-top">
                  <td className="py-1.5 pr-3">
                    <p className="text-text-primary">{m.nom}</p>
                    <p className="text-[11px] text-text-subtle">{m.note}</p>
                    {m.finition && (
                      <p className="text-[11px] text-text-muted">
                        Craft final : {m.finition.monnaies.map((c) => `${c.quantite} ${c.nom}`).join(" + ")}
                        {m.coutFinition != null ? ` (${formatChaos(m.coutFinition)}, payé une fois)` : ""}
                      </p>
                    )}
                  </td>
                  <td className="pr-3 font-mono">{formatChance(m.chance)}</td>
                  <td className="pr-3 font-mono">
                    {m.coutEssai != null ? (
                      formatChaos(m.coutEssai)
                    ) : (
                      <span className="text-signal-warn">prix inconnu</span>
                    )}
                  </td>
                  <td className="pr-3 font-mono text-text-primary">{formatChaos(m.coutMoyen, divine)}</td>
                  <td className="font-mono">
                    {formatEssais(m.essaisP50)} / {formatEssais(m.essaisP90)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      {result.ecartees.length > 0 && (
        <ul className="text-[11px] text-text-subtle">
          {result.ecartees.map((e) => (
            <li key={e.nom}>
              {e.nom} : {e.raison}.
            </li>
          ))}
        </ul>
      )}
      <PrixUtilises noms={noms} prix={prix} saisis={saisis} onSaisie={onSaisie} />
      <p className="text-[11px] text-text-subtle">
        Coûts hors prix de la base. Le coût moyen suppose qu'on recommence jusqu'à réussir, plus le craft d'établi final
        s'il y en a un ; un essai sur deux finit avant le nombre médian. Les crafts d'établi doivent être débloqués.
      </p>
    </div>
  );
}

/** Forge en mode PoE 1 : méthodes de spam classées par coût moyen. */
export function ForgePoe1Card() {
  const [bases, setBases] = useState<ForgePoe1Base[]>([]);
  const [classe, setClasse] = useState("");
  const [baseId, setBaseId] = useState("");
  const [ilvl, setIlvl] = useState(84);
  const [influence, setInfluence] = useState<string | null>(null);
  const [affixes, setAffixes] = useState<ForgePoe1Affixes | null>(null);
  const [voulus, setVoulus] = useState<ForgeWanted[]>([]);
  const [result, setResult] = useState<ForgePoe1Result | null>(null);
  const [prix, setPrix] = useState<ForgePoe1Prices | null>(null);
  const [saisis, setSaisis] = useState<Record<string, number>>(lirePrixSaisis);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Prix envoyés au dernier calcul, pour signaler un plan devenu périmé. */
  const [prixCalcul, setPrixCalcul] = useState("");

  useEffect(() => {
    if (!isTauri()) return;
    void commands
      .getForgePoe1Bases()
      .then(setBases)
      .catch((e) => setError(message(e)));
  }, []);

  const classes = useMemo(() => [...new Set(bases.map((b) => b.classe))].sort((a, b) => a.localeCompare(b)), [bases]);
  const basesDeClasse = bases.filter((b) => b.classe === classe);
  const base = bases.find((b) => b.id === baseId) ?? null;

  useEffect(() => {
    setAffixes(null);
    setResult(null);
    if (!baseId || !isTauri()) return;
    const timer = setTimeout(() => {
      commands
        .getForgePoe1Affixes(baseId, ilvl, influence)
        .then((a) => {
          setAffixes(a);
          setVoulus((v) => v.filter((w) => a.affixes.some((x) => cleAffixe(x) === cleAffixe(w))));
        })
        .catch((e) => setError(message(e)));
    }, 250);
    return () => clearTimeout(timer);
  }, [baseId, ilvl, influence]);

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
      const cote = prix ?? (await commands.getForgePoe1Prices().catch(() => null));
      if (!prix) setPrix(cote);
      const envoyes = prixPourPlan(cote?.prix ?? {}, saisis);
      const demande: ForgePoe1Request = {
        baseId,
        ilvl,
        influence,
        voulus,
        prix: envoyes,
      };
      setResult(await commands.planForgePoe1(demande));
      setPrixCalcul(JSON.stringify(envoyes));
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }

  function saisir(nom: string, v: number | null) {
    setSaisis((s) => {
      const n = { ...s };
      if (v == null) delete n[nom];
      else n[nom] = v;
      garderPrixSaisis(n);
      return n;
    });
  }

  const nomsVoulus = voulus.map((w, i) => {
    const a = affixes?.affixes.find((x) => cleAffixe(x) === cleAffixe(w));
    return a ? libelleCourt(a.libelle) : `mod ${i + 1}`;
  });

  if (!isTauri()) {
    return <p className="text-sm text-text-subtle">Forge fonctionne dans l'application.</p>;
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs text-text-subtle">
          Type d'objet
          <select
            value={classe}
            onChange={(e) => {
              setClasse(e.target.value);
              setBaseId("");
              setInfluence(null);
              setVoulus([]);
            }}
            className={input}
          >
            <option value="">Choisir…</option>
            {classes.map((c) => (
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
            className={`${input} w-72`}
          >
            <option value="">Choisir…</option>
            {basesDeClasse.map((b) => (
              <option key={b.id} value={b.id}>
                {libelleBase(b, basesDeClasse)}
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
        <label className="flex flex-col gap-1 text-xs text-text-subtle">
          Influence
          <select
            value={influence ?? ""}
            disabled={!base || base.influences.length === 0}
            onChange={(e) => {
              setInfluence(e.target.value || null);
              setResult(null);
            }}
            className={input}
          >
            <option value="">Aucune</option>
            {base?.influences.map((i) => (
              <option key={i} value={i}>
                {i}
              </option>
            ))}
          </select>
        </label>
      </div>

      {base && base.implicites.length > 0 && (
        <p className="text-xs text-text-subtle">Implicite : {base.implicites.join(" · ")}</p>
      )}

      {affixes && (
        <>
          <div className="grid grid-cols-2 gap-4">
            <AffixColumn
              slot="prefix"
              affixes={affixes.affixes}
              voulus={voulus}
              onToggle={toggle}
              onTier={setTier}
              essenceHint="Posé seulement par une Essence"
            />
            <AffixColumn
              slot="suffix"
              affixes={affixes.affixes}
              voulus={voulus}
              onToggle={toggle}
              onTier={setTier}
              essenceHint="Posé seulement par une Essence"
            />
          </div>
          <div className="flex items-center gap-3">
            <button
              type="button"
              disabled={busy || voulus.length === 0}
              onClick={() => void calculer()}
              className={btnPrimary}
            >
              {busy ? "Calcul en cours…" : "Comparer les méthodes"}
            </button>
            {result && (
              <button
                type="button"
                disabled={busy}
                onClick={() =>
                  void commands
                    .getForgePoe1Prices()
                    .then(setPrix)
                    .catch((e) => setError(message(e)))
                }
                className={btnSecondary}
              >
                Actualiser la cote
              </button>
            )}
            <span className="text-xs text-text-subtle">
              {voulus.length === 0
                ? "Coche les affixes voulus (3 préfixes et 3 suffixes au plus)."
                : `${voulus.length} affixe(s) voulu(s) : ${voulus
                    .map((w) => affixes.affixes.find((a) => cleAffixe(a) === cleAffixe(w)))
                    .filter((a): a is ForgeAffix => Boolean(a))
                    .map((a) => libelleCourt(a.libelle))
                    .join(", ")}.`}
            </span>
          </div>
        </>
      )}

      {error && (
        <StatePanel icon="alert" title="Calcul impossible" variant="warn">
          {error}
        </StatePanel>
      )}
      {result && (
        <Resultat
          result={result}
          prix={prix}
          saisis={saisis}
          onSaisie={saisir}
          nomsVoulus={nomsVoulus}
          prixChanges={prixCalcul !== "" && prixCalcul !== JSON.stringify(prixPourPlan(prix?.prix ?? {}, saisis))}
        />
      )}
    </div>
  );
}
