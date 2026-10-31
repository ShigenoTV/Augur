import { useEffect, useState } from "react";

/** Décompte local à partir d'une durée reçue du backend (rafraîchi toutes les 250 ms). */
export function useCountdown(ms: number | undefined): number | undefined {
  const [start] = useState(() => Date.now());
  const [now, setNow] = useState(start);
  useEffect(() => {
    if (ms === undefined || ms <= 0) return;
    const id = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(id);
  }, [ms]);
  if (ms === undefined) return undefined;
  return Math.max(0, ms - (now - start));
}
