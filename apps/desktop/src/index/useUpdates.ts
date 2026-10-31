import type { UpdatePhase } from "@augur/ui";
import { useCallback, useEffect, useState } from "react";
import { commands, isAppError, isTauri, on } from "../bridge";

/**
 * État de la mise à jour dans la fenêtre principale : vérification en tâche
 * de fond (Rust), à la demande (bouton ou menu de la zone de notification),
 * installation seulement sur action du joueur.
 */
export function useUpdates() {
  const [update, setUpdate] = useState<UpdatePhase>({ phase: "idle" });
  const [dismissed, setDismissed] = useState<string | null>(null);

  const check = useCallback(async () => {
    if (!isTauri()) return;
    setUpdate({ phase: "checking" });
    try {
      const info = await commands.checkForUpdate();
      setUpdate(info ? { phase: "available", info } : { phase: "upToDate", checkedAt: new Date().toISOString() });
      setDismissed(null);
    } catch (err) {
      setUpdate({ phase: "error", message: isAppError(err) ? err.message : String(err) });
    }
  }, []);

  const install = useCallback(async () => {
    if (update.phase !== "available") return;
    const info = update.info;
    setUpdate({ phase: "installing", info, progress: null });
    try {
      await commands.installUpdate(); // redémarre l'application en cas de succès
    } catch (err) {
      setUpdate({ phase: "error", message: isAppError(err) ? err.message : String(err) });
    }
  }, [update]);

  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = [
      on.updateAvailable((info) => setUpdate((u) => (u.phase === "installing" ? u : { phase: "available", info }))),
      on.updateProgress((progress) => setUpdate((u) => (u.phase === "installing" ? { ...u, progress } : u))),
      on.updateCheckRequested(() => void check()),
    ];
    return () => unlisten.forEach((p) => void p.then((f) => f()));
  }, [check]);

  // « Plus tard » masque le bandeau jusqu'à la prochaine version annoncée.
  const visible: UpdatePhase =
    update.phase === "available" && dismissed === update.info.version ? { phase: "idle" } : update;
  const later = () => update.phase === "available" && setDismissed(update.info.version);

  return { update, visible, check, install, later };
}
