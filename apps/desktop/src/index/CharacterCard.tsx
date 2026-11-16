import { StatePanel } from "@augur/ui";
import { useEffect, useState } from "react";
import { commands, isAppError, isTauri, on, type CharacterInfo, type Game } from "../bridge";
import { describeSkill, describeTargetSkill } from "./character";

/**
 * Personnage du joueur du jeu actif : import d'un code d'export Path of Building
 * (PoE 2 ou PoE 1). Le code est collé à la main ; Augur ne télécharge rien.
 * `target` : le personnage cible (build visé), qui ne sert qu'à la carte Écart.
 */
export function CharacterCard({ game = "poe2", target = false }: { game?: Game; target?: boolean }) {
  const poe1 = game === "poe1";
  const api = target
    ? { get: commands.getTarget, import: commands.importTargetCode, clear: commands.clearTarget, changed: on.targetChanged }
    : { get: commands.getCharacter, import: commands.importPobCode, clear: commands.clearCharacter, changed: on.characterChanged };
  const [info, setInfo] = useState<CharacterInfo | null>(null);
  const [editing, setEditing] = useState(false);
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    void api.get().then(setInfo).catch(() => undefined);
    const unlisten = api.changed(setInfo);
    return () => void unlisten.then((f) => f());
    // `target` ne change pas pendant la vie de la carte.
  }, []);

  async function importCode() {
    setError(null);
    if (!isTauri()) {
      setError("L'import fonctionne dans l'application.");
      return;
    }
    setBusy(true);
    try {
      setInfo(await api.import(code));
      setCode("");
      setEditing(false);
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    setError(null);
    try {
      await api.clear();
      setInfo(null);
    } catch (err) {
      setError(isAppError(err) ? err.message : String(err));
    }
  }

  const btn = "rounded-chip px-3 py-1.5 text-sm font-semibold";
  const showForm = editing || !info;

  return (
    <div className="space-y-3 text-sm">
      {info && (
        <div className="space-y-1">
          <p>
            {info.summary.class} niveau {info.summary.level}
          </p>
          <p className="text-text-muted">{target ? describeTargetSkill(info) : describeSkill(info)}</p>
          {info.supports.length > 0 && (
            <p className="text-xs text-text-subtle">Supports : {info.supports.join(", ")}</p>
          )}
          <p className="text-xs text-text-subtle">
            {info.equippedSlots.length} objets équipés, {info.passives} passifs
          </p>
          {poe1 && !target && (
            <p className="text-xs text-text-subtle">
              PoE 1 : Path of Building tourne dans Augur. Ctrl+C sur un objet en jeu dit s'il est mieux que le tien (DPS,
              EHP, vie, résistances).
            </p>
          )}
          {!poe1 && !target && !info.skillSupported && (
            <p className="text-xs text-signal-loss">
              Augur ne calcule que les compétences qui infligent des dégâts (sorts, attaques, minions) : le gain de DPS ne sera pas affiché pour cette
              compétence.
            </p>
          )}
          {!editing && (
            <div className="flex gap-2 pt-1">
              <button
                type="button"
                onClick={() => setEditing(true)}
                className={`${btn} border border-ink-600 text-text-primary hover:bg-ink-800`}
              >
                Changer
              </button>
              <button
                type="button"
                onClick={() => void clear()}
                className={`${btn} text-text-subtle hover:text-text-muted`}
              >
                Oublier
              </button>
            </div>
          )}
        </div>
      )}

      {showForm && (
        <div className="space-y-2">
          {target && (
            <p className="text-xs text-text-subtle">
              Le build que tu vises (le tien plus tard, ou celui d'un guide). Augur te montre l'écart avec ton personnage actuel.
            </p>
          )}
          <p className="text-xs text-text-subtle">
            Dans Path of Building {poe1 ? "(PoE 1)" : "PoE 2"} : Import/Export Build &gt; Generate &gt; Copy, puis colle le code ici. Sur pobb.in,
            utilise le bouton de copie du code (pas le lien).
          </p>
          <textarea
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="Code Path of Building (eNrt…)"
            className="h-20 w-full resize-none rounded-chip border border-ink-700 bg-ink-950 p-2 font-mono text-xs text-text-primary placeholder:text-text-subtle"
          />
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => void importCode()}
              disabled={!code.trim() || busy}
              className={`${btn} bg-accent text-accent-ink hover:bg-accent-strong disabled:opacity-50`}
            >
              Importer
            </button>
            {info && (
              <button
                type="button"
                onClick={() => {
                  setEditing(false);
                  setError(null);
                }}
                className={`${btn} text-text-subtle hover:text-text-muted`}
              >
                Annuler
              </button>
            )}
          </div>
        </div>
      )}

      {error && (
        <StatePanel icon="alert" title="Import refusé" variant="warn">
          {error}
        </StatePanel>
      )}
    </div>
  );
}
