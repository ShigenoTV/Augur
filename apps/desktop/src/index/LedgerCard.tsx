import { StatePanel } from "@augur/ui";
import { Fragment, useCallback, useEffect, useState } from "react";
import {
  commands,
  isAppError,
  isTauri,
  on,
  type LedgerGains,
  type LedgerLoot,
  type LedgerMapRun,
  type LedgerSession,
  type LedgerSessionRow,
  type LedgerStatus,
  type LedgerStrategy,
} from "../bridge";
import { shortTime } from "./measures";
import { useCout } from "./devise";
import {
  describeStrategy,
  formatDuration,
  formatRate,
  lootDraft,
  parseTags,
  shortDateTime,
  timeShares,
} from "./ledger";
import { LootEditor } from "./LootEditor";

const HISTORY_LIMIT = 20;
const EMPTY: LedgerStrategy = { waystoneTier: null, tablets: [], mechanics: [], note: "" };
const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold";
const input =
  "rounded-chip border border-ink-700 bg-ink-950 px-2 py-1 text-xs text-text-primary placeholder:text-text-subtle";

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-chip border border-ink-700 bg-ink-950 px-3 py-2">
      <p className="text-xs text-text-subtle">{label}</p>
      <p className="font-mono text-sm text-text-primary">{value}</p>
    </div>
  );
}

/** Saisie de la stratégie (session en cours ou session de l'historique). */
function StrategyForm({ value, onSave }: { value: LedgerStrategy; onSave: (s: LedgerStrategy) => Promise<boolean> }) {
  const [tier, setTier] = useState(value.waystoneTier ? String(value.waystoneTier) : "");
  const [mechanics, setMechanics] = useState(value.mechanics.join(", "));
  const [tablets, setTablets] = useState(value.tablets.join(", "));
  const [note, setNote] = useState(value.note);
  const [saved, setSaved] = useState(false);

  async function save() {
    setSaved(false);
    const ok = await onSave({
      waystoneTier: tier ? Number(tier) : null,
      mechanics: parseTags(mechanics),
      tablets: parseTags(tablets),
      note: note.trim(),
    });
    if (!ok) return;
    setSaved(true);
    setTimeout(() => setSaved(false), 3_000);
  }

  return (
    <div className="grid grid-cols-[auto_1fr_1fr] items-end gap-2">
      <label className="flex flex-col gap-1 text-xs text-text-subtle">
        Waystone
        <select value={tier} onChange={(e) => setTier(e.target.value)} className={input}>
          <option value="">–</option>
          {Array.from({ length: 16 }, (_, i) => i + 1).map((t) => (
            <option key={t} value={t}>
              T{t}
            </option>
          ))}
        </select>
      </label>
      <label className="flex flex-col gap-1 text-xs text-text-subtle">
        Mécaniques ciblées
        <input value={mechanics} onChange={(e) => setMechanics(e.target.value)} placeholder="Breach, Ritual…" className={input} />
      </label>
      <label className="flex flex-col gap-1 text-xs text-text-subtle">
        Tablettes de précurseur
        <input value={tablets} onChange={(e) => setTablets(e.target.value)} placeholder="Séparées par des virgules" className={input} />
      </label>
      <label className="col-span-2 flex flex-col gap-1 text-xs text-text-subtle">
        Note
        <input value={note} onChange={(e) => setNote(e.target.value)} placeholder="Atlas, build, contexte…" className={input} />
      </label>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => void save()}
          className={`${btn} border border-ink-600 text-text-primary hover:bg-ink-800`}
        >
          Enregistrer l'étiquette
        </button>
        {saved && <span className="text-xs text-signal-gain">Enregistrée.</span>}
      </div>
    </div>
  );
}

/** Gains nets, par map et par heure (session en cours ou enregistrée). */
function GainsStats({ gains }: { gains: LedgerGains | null }) {
  const cout = useCout();
  const saisie = gains != null && gains.mapsSaisies > 0;
  return (
    <div className="grid grid-cols-4 gap-2">
      <Stat label="Gains nets" value={saisie ? cout(gains.net) : "–"} />
      <Stat label="Par map" value={saisie ? cout(gains.netParMap) : "–"} />
      <Stat label="Par heure" value={saisie && gains.netParHeure != null ? `${cout(gains.netParHeure)}/h` : "–"} />
      <Stat label="Dépense" value={saisie ? cout(gains.depense) : "–"} />
    </div>
  );
}

/** Maps d'une session, avec le butin de chacune (dernière map en haut). */
function MapsTable({
  maps,
  loot,
  gains,
  prix,
  onSaveLoot,
}: {
  maps: LedgerMapRun[];
  loot: Record<string, LedgerLoot>;
  gains: LedgerGains | null;
  prix: Record<string, number>;
  onSaveLoot: (map: number, loot: LedgerLoot) => Promise<boolean>;
}) {
  const cout = useCout();
  const [open, setOpen] = useState<number | null>(null);
  const rows = maps.map((m, i) => ({ m, i })).reverse();
  return (
    <table className="w-full text-left text-xs">
      <thead className="text-text-subtle">
        <tr>
          <th className="py-1 font-normal">Map</th>
          <th className="py-1 font-normal">Niveau</th>
          <th className="py-1 font-normal">Durée</th>
          <th className="py-1 font-normal">Entrées</th>
          <th className="py-1 font-normal">Gains nets</th>
          <th className="py-1" />
        </tr>
      </thead>
      <tbody>
        {rows.map(({ m, i }) => {
          const v = gains?.parMap[i] ?? null;
          return (
            <Fragment key={`${m.areaId}-${m.graine}-${m.debut}`}>
              <tr className="border-t border-ink-800">
                <td className="py-1 text-text-primary">{m.nom ?? m.areaId}</td>
                <td className="py-1 font-mono">{m.niveau}</td>
                <td className="py-1 font-mono">{formatDuration(m.dureeActive)}</td>
                <td className="py-1 font-mono">{m.entrees}</td>
                <td className={`py-1 font-mono ${v && v.net < 0 ? "text-signal-loss" : ""}`}>
                  {v ? cout(v.net) : "–"}
                </td>
                <td className="py-1 text-right">
                  <button
                    type="button"
                    onClick={() => setOpen(open === i ? null : i)}
                    className="text-text-subtle hover:text-text-primary"
                  >
                    {v ? "Modifier le butin" : "Saisir le butin"}
                  </button>
                </td>
              </tr>
              {open === i && (
                <tr>
                  <td colSpan={6} className="py-2">
                    <LootEditor
                      value={lootDraft(loot, i)}
                      prix={prix}
                      onSave={(l) => onSaveLoot(i, l)}
                      onClose={() => setOpen(null)}
                    />
                  </td>
                </tr>
              )}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}

/**
 * Atlas Ledger : session de farm suivie dans Client.txt (maps jouées, temps
 * par map, rythme) et historique local des sessions, étiquetées par stratégie.
 */
export function LedgerCard() {
  const [status, setStatus] = useState<LedgerStatus | null>(null);
  const [history, setHistory] = useState<LedgerSessionRow[]>([]);
  const [editing, setEditing] = useState<number | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [prix, setPrix] = useState<Record<string, number>>({});
  const [opened, setOpened] = useState<LedgerSession | null>(null);
  const cout = useCout();

  const refreshHistory = useCallback(() => {
    void commands
      .listLedgerSessions(HISTORY_LIMIT)
      .then(setHistory)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    void commands.getLedgerStatus().then(setStatus).catch(() => undefined);
    void commands
      .getForgePrices()
      .then((t) => setPrix(t.prix))
      .catch(() => undefined);
    refreshHistory();
    const unlisten = on.ledgerSession(setStatus);
    return () => void unlisten.then((f) => f());
  }, [refreshHistory]);

  const active = status?.active ?? false;

  // Les durées avancent même sans changement de zone.
  useEffect(() => {
    if (!active) return;
    const timer = setInterval(() => {
      void commands.getLedgerStatus().then(setStatus).catch(() => undefined);
    }, 5_000);
    return () => clearInterval(timer);
  }, [active]);

  async function run(action: () => Promise<LedgerStatus | void>): Promise<boolean> {
    setError(null);
    if (!isTauri()) {
      setError("Atlas Ledger fonctionne dans l'application.");
      return false;
    }
    try {
      const next = await action();
      if (next) setStatus(next);
      return true;
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
      return false;
    }
  }

  async function stop() {
    await run(commands.stopLedgerSession);
    refreshHistory();
  }

  async function openLoot(id: number) {
    if (opened?.id === id) {
      setOpened(null);
      return;
    }
    await run(async () => setOpened(await commands.getLedgerSession(id)));
  }

  async function saveSessionLoot(id: number, map: number, loot: LedgerLoot): Promise<boolean> {
    const ok = await run(async () => setOpened(await commands.setLedgerSessionLoot(id, map, loot)));
    refreshHistory();
    return ok;
  }

  async function remove(id: number) {
    setConfirmDelete(null);
    if (opened?.id === id) setOpened(null);
    await run(() => commands.deleteLedgerSession(id));
    refreshHistory();
  }

  const summary = status?.summary ?? null;
  const shares = summary ? timeShares(summary.temps) : null;

  return (
    <div className="space-y-4 text-sm">
      {active && status ? (
        <div className="space-y-3">
          <p className="flex items-center gap-2">
            <span className="inline-block size-2 animate-pulse rounded-full bg-signal-gain" aria-hidden />
            Session depuis {shortTime(status.startedAt)}
            {summary?.zoneCourante && <span className="text-text-subtle">· zone actuelle : {summary.zoneCourante}</span>}
          </p>
          <div className="grid grid-cols-4 gap-2">
            <Stat label="Maps" value={String(summary?.maps.length ?? 0)} />
            <Stat label="Rythme" value={formatRate(summary?.mapsParHeure)} />
            <Stat label="Durée moyenne d'une map" value={formatDuration(summary?.dureeMoyenneMap)} />
            <Stat
              label="Temps en map / hideout / ville"
              value={shares ? `${shares.map} / ${shares.hideout} / ${shares.ville} %` : "–"}
            />
          </div>

          <StrategyForm
            key={`courante-${status.startedAt}`}
            value={status.strategy}
            onSave={(s) => run(() => commands.setLedgerStrategy(null, s))}
          />

          <GainsStats gains={status.gains} />

          {summary && summary.maps.length > 0 && (
            <MapsTable
              maps={summary.maps}
              loot={status.loot}
              gains={status.gains}
              prix={prix}
              onSaveLoot={(map, loot) => run(() => commands.setLedgerLoot(map, loot))}
            />
          )}

          <button type="button" onClick={() => void stop()} className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong`}>
            Arrêter et enregistrer
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          <p className="text-text-muted">
            Pendant la session, Augur lit Client.txt pour compter les maps, le temps passé dans chacune et ton rythme en
            maps par heure. Après chaque map, saisis ton butin pour suivre tes gains. Étiquette ta stratégie pour comparer
            tes sessions.
          </p>
          <button
            type="button"
            onClick={() => void run(commands.startLedgerSession)}
            className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong`}
          >
            Démarrer une session de farm
          </button>
        </div>
      )}

      {history.length > 0 && (
        <div className="space-y-2">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-text-subtle">Historique</h3>
          <table className="w-full text-left text-xs">
            <thead className="text-text-subtle">
              <tr>
                <th className="py-1 font-normal">Début</th>
                <th className="py-1 font-normal">Stratégie</th>
                <th className="py-1 font-normal">Maps</th>
                <th className="py-1 font-normal">Durée</th>
                <th className="py-1 font-normal">Rythme</th>
                <th className="py-1 font-normal">Moyenne</th>
                <th className="py-1 font-normal">Gains nets</th>
                <th className="py-1 font-normal">Par heure</th>
                <th className="py-1" />
              </tr>
            </thead>
            <tbody>
              {history.map((row) => (
                <Fragment key={row.id}>
                  <tr className="border-t border-ink-800">
                    <td className="py-1 font-mono">{shortDateTime(row.startedAt)}</td>
                    <td className="py-1 text-text-primary">{describeStrategy(row.strategy)}</td>
                    <td className="py-1 font-mono">{row.maps}</td>
                    <td className="py-1 font-mono">{formatDuration(row.totalSecs)}</td>
                    <td className="py-1 font-mono">{formatRate(row.mapsPerHour)}</td>
                    <td className="py-1 font-mono">{formatDuration(row.avgMapSecs)}</td>
                    <td className={`py-1 font-mono ${row.net != null && row.net < 0 ? "text-signal-loss" : ""}`}>
                      {row.net != null ? cout(row.net) : "–"}
                    </td>
                    <td className="py-1 font-mono">{row.netPerHour != null ? `${cout(row.netPerHour)}/h` : "–"}</td>
                    <td className="py-1 text-right whitespace-nowrap">
                      <button
                        type="button"
                        onClick={() => void openLoot(row.id)}
                        className="text-text-subtle hover:text-text-primary"
                      >
                        Butin
                      </button>{" "}
                      ·{" "}
                      <button
                        type="button"
                        onClick={() => setEditing(editing === row.id ? null : row.id)}
                        className="text-text-subtle hover:text-text-primary"
                      >
                        Étiqueter
                      </button>{" "}
                      ·{" "}
                      {confirmDelete === row.id ? (
                        <button type="button" onClick={() => void remove(row.id)} className="text-signal-loss">
                          Confirmer
                        </button>
                      ) : (
                        <button
                          type="button"
                          onClick={() => setConfirmDelete(row.id)}
                          className="text-text-subtle hover:text-text-primary"
                        >
                          Supprimer
                        </button>
                      )}
                    </td>
                  </tr>
                  {opened?.id === row.id && (
                    <tr>
                      <td colSpan={9} className="space-y-2 py-2">
                        <GainsStats gains={opened.gains} />
                        {opened.summary.maps.length > 0 ? (
                          <MapsTable
                            maps={opened.summary.maps}
                            loot={opened.loot}
                            gains={opened.gains}
                            prix={Object.keys(opened.prices).length ? opened.prices : prix}
                            onSaveLoot={(map, loot) => saveSessionLoot(row.id, map, loot)}
                          />
                        ) : (
                          <p className="text-text-subtle">Aucune map dans cette session.</p>
                        )}
                      </td>
                    </tr>
                  )}
                  {editing === row.id && (
                    <tr>
                      <td colSpan={9} className="py-2">
                        <StrategyForm
                          value={row.strategy ?? EMPTY}
                          onSave={async (s) => {
                            const ok = await run(() => commands.setLedgerStrategy(row.id, s));
                            refreshHistory();
                            return ok;
                          }}
                        />
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <p className="text-xs text-text-subtle">
        Le butin est saisi à la main (l'API des réserves n'est pas ouverte pour PoE 2) et valorisé aux prix de Forge :
        relevé poe.ninja s'il a été actualisé, sinon valeurs indicatives. Une session enregistrée garde les prix de son
        arrêt. Rien n'est envoyé : l'historique reste sur ce PC.
      </p>
      {status?.storageError && (
        <StatePanel icon="alert" title="Historique" variant="warn">
          {status.storageError}
        </StatePanel>
      )}
      {error && (
        <StatePanel icon="alert" title="Atlas Ledger" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}
