import { DEFAULT_LEAGUE, DEFAULT_LEAGUE_POE1, type Game, type Settings } from "../bridge";

/** Les deux jeux du sélecteur, dans l'ordre d'affichage. */
export const JEUX: { id: Game; nom: string }[] = [
  { id: "poe1", nom: "POE 1" },
  { id: "poe2", nom: "POE 2" },
];

/** Jeu actif (PoE 2 tant que les réglages ne sont pas chargés, comme le cœur Rust). */
export function jeuActif(settings: Settings | null): Game {
  return settings?.game ?? "poe2";
}

/** Ligue par défaut d'un jeu (doit suivre settings.rs). */
export function ligueParDefaut(game: Game): string {
  return game === "poe1" ? DEFAULT_LEAGUE_POE1 : DEFAULT_LEAGUE;
}

/** Ligue enregistrée pour le jeu actif, sinon sa ligue par défaut. */
export function ligueDuJeu(settings: Settings | null): string {
  const game = jeuActif(settings);
  const l = game === "poe1" ? settings?.poe1?.league : settings?.league;
  return l?.trim() ? l.trim() : ligueParDefaut(game);
}

/** Chemin de Client.txt choisi pour le jeu actif. */
export function cheminJournalDuJeu(settings: Settings | null): string | null {
  return jeuActif(settings) === "poe1" ? (settings?.poe1?.clientLogPath ?? null) : (settings?.clientLogPath ?? null);
}

/** Exemple de dossier du jeu, pour le champ Client.txt. */
export function exempleDossier(game: Game): string {
  return game === "poe1"
    ? "D:\\SteamLibrary\\steamapps\\common\\Path of Exile"
    : "D:\\SteamLibrary\\steamapps\\common\\Path of Exile 2";
}
