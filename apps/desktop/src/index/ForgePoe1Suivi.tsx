import { useContext, useEffect, useState } from "react";
import {
  commands,
  isAppError,
  isTauri,
  type ForgeBuySearch,
  type ForgePoe1History,
  type ForgePoe1Request,
} from "../bridge";
import { AffichagePoe1Context, useCoutPoe1 } from "./devisePoe1";
import { MARGE_EQUIVALENT, libelleCourt, lirePrix, type VerdictAchat } from "./forge";

const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold disabled:opacity-50";
const btnSecondary = `${btn} border border-ink-600 text-text-primary hover:bg-ink-800`;
const input =
  "rounded-chip border border-ink-700 bg-ink-950 px-2 py-1 text-xs text-text-primary placeholder:text-text-subtle";

const message = (e: unknown) => (isAppError(e) ? e.message : String(e));

/** Craft contre achat, en Chaos : même marge « équivalent » que la Forge PoE 2. */
export function comparerAchatPoe1(coutCraft: number, prixAchat: number): { ecart: number; verdict: VerdictAchat } {
  const ecart = prixAchat - coutCraft;
  const reference = Math.max(coutCraft, prixAchat);
  const verdict: VerdictAchat =
    reference <= 0 || Math.abs(ecart) <= MARGE_EQUIVALENT * reference ? "equivalent" : ecart > 0 ? "crafter" : "acheter";
  return { ecart, verdict };
}

/** Prix saisi dans la monnaie d'affichage (Chaos ou Divine), converti en Chaos. */
export function versChaos(v: number, devise: "chaos" | "divine", divine: number): number {
  return devise === "divine" && divine > 0 ? v * divine : v;
}

/**
 * « Crafter ou acheter ? » en PoE 1 : la recherche de l'objet visé s'ouvre sur le site de trade, le
 * joueur reporte le prix vu, Forge le compare au coût moyen du plan conseillé.
 */
export function AchatPoe1({ demande, coutCraft }: { demande: ForgePoe1Request; coutCraft: number }) {
  const cout = useCoutPoe1();
  const { affichage } = useContext(AffichagePoe1Context);
  const devise = affichage.devise === "divine" && affichage.divine > 0 ? "divine" : "chaos";
  const [saisie, setSaisie] = useState("");
  const [recherche, setRecherche] = useState<ForgeBuySearch | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const prix = lirePrix(saisie);
  const comparaison =
    prix !== null && prix > 0 ? comparerAchatPoe1(coutCraft, versChaos(prix, devise, affichage.divine)) : null;

  async function chercher() {
    setErreur(null);
    try {
      setRecherche(await commands.openForgePoe1BuySearch({ ...demande, objetDepart: null }));
    } catch (e) {
      setErreur(message(e));
    }
  }

  return (
    <div className="space-y-2 rounded-panel border border-ink-700 bg-ink-950 p-3 text-xs">
      <div className="flex flex-wrap items-end gap-3">
        <span className="text-sm font-semibold text-text-primary">Crafter ou acheter ?</span>
        <button type="button" onClick={() => void chercher()} className={btnSecondary}>
          Chercher l'objet à l'achat
        </button>
        <label className="flex items-center gap-2 text-text-subtle">
          Prix d'achat estimé
          <input
            value={saisie}
            inputMode="decimal"
            placeholder="ex. 150"
            onChange={(e) => setSaisie(e.target.value)}
            className={`${input} w-24 text-right font-mono`}
          />
          <span>{devise === "divine" ? "div" : "c"}</span>
        </label>
      </div>
      {recherche && (
        <p className="text-text-subtle">
          Recherche ouverte dans ton navigateur ({recherche.filtres} filtre(s), même base, du moins cher au plus cher).
          Reporte le prix d'un objet qui te convient.
          {recherche.ignorees.length > 0 && (
            <span className="text-signal-warn">
              {" "}
              Non filtré : {recherche.ignorees.map(libelleCourt).join(", ")} (vérifie-le sur l'annonce).
            </span>
          )}
        </p>
      )}
      {erreur && <p className="text-signal-warn">{erreur}</p>}
      {saisie.trim() !== "" && (prix === null || prix <= 0) && <p className="text-signal-warn">Saisis un prix positif.</p>}
      {comparaison && (
        <div className="space-y-0.5">
          <p className={`text-sm font-semibold ${comparaison.verdict === "equivalent" ? "text-text-primary" : "text-accent"}`}>
            {comparaison.verdict === "crafter"
              ? `Crafter : environ ${cout(comparaison.ecart)} d'économie en moyenne`
              : comparaison.verdict === "acheter"
                ? `Acheter : environ ${cout(-comparaison.ecart)} d'économie en moyenne`
                : "Équivalent : l'écart est trop faible pour trancher"}
          </p>
          <p className="text-text-muted">
            Craft : {cout(coutCraft)} en moyenne (hors prix de la base) · achat : {cout(coutCraft + comparaison.ecart)}.
          </p>
        </div>
      )}
    </div>
  );
}

const date = (ms: number) =>
  new Date(ms).toLocaleString("fr-FR", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });

/**
 * Plans PoE 1 calculés et plan suivi par l'overlay : Ctrl+Alt+C sur l'objet en cours de craft donne
 * l'action conseillée. `version` change à chaque nouveau calcul pour relire l'historique.
 */
export function HistoriquePoe1({ version, onRouvrir }: { version: number; onRouvrir?: (d: ForgePoe1Request) => void }) {
  const cout = useCoutPoe1();
  const [h, setH] = useState<ForgePoe1History | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    void commands
      .getForgePoe1History()
      .then(setH)
      .catch((e) => setErreur(message(e)));
  }, [version]);

  const agir = (p: Promise<ForgePoe1History>) => void p.then(setH).catch((e) => setErreur(message(e)));

  if (!h || h.entrees.length === 0) {
    return (
      <p className="text-xs text-text-subtle">
        Chaque plan calculé arrive ici. Le dernier est suivi par l'overlay : Ctrl+Alt+C sur ton objet en cours de craft
        donne l'action à faire.
      </p>
    );
  }
  const suivi = h.entrees.find((e) => e.id === h.suivi) ?? null;
  return (
    <div className="space-y-2 text-xs">
      {suivi ? (
        <div className="flex flex-wrap items-center gap-3 rounded-chip border border-accent/40 bg-accent/5 px-2 py-1.5">
          <span className="text-text-muted">
            L'overlay suit <span className="font-semibold text-text-primary">{suivi.baseNom}</span> (
            {suivi.labels.map(libelleCourt).join(", ")}) : <span className="font-mono">Ctrl+Alt+C</span> sur l'objet en cours
            de craft.
          </span>
          <button type="button" onClick={() => agir(commands.followForgePoe1Entry(null))} className="text-accent hover:underline">
            Arrêter le suivi
          </button>
        </div>
      ) : (
        <p className="text-text-subtle">Aucun plan suivi par l'overlay.</p>
      )}
      <ul className="divide-y divide-ink-800">
        {h.entrees.map((e) => (
          <li key={e.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 py-1.5">
            <span className="w-24 text-text-subtle">{date(e.at)}</span>
            <span className="min-w-0 flex-1">
              <span className="text-text-primary">{e.baseNom}</span>
              <span className="text-text-subtle">
                {" "}
                · niv. {e.ilvl}
                {e.influence ? ` · ${e.influence}` : ""} · {e.labels.map(libelleCourt).join(", ")}
              </span>
            </span>
            <span className="font-mono text-text-muted">{cout(e.coutMoyen)}</span>
            {onRouvrir && (
              <button type="button" onClick={() => onRouvrir(e.demande)} className="text-accent hover:underline">
                Rouvrir
              </button>
            )}
            {e.id !== h.suivi && (
              <button type="button" onClick={() => agir(commands.followForgePoe1Entry(e.id))} className="text-accent hover:underline">
                Suivre
              </button>
            )}
            <button
              type="button"
              onClick={() => agir(commands.deleteForgePoe1HistoryEntry(e.id))}
              className="text-text-subtle hover:text-signal-loss"
            >
              Supprimer
            </button>
          </li>
        ))}
      </ul>
      {erreur && <p className="text-signal-warn">{erreur}</p>}
    </div>
  );
}
