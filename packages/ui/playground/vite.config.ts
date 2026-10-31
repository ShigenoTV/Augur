import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

/** Bac à sable des maquettes : tous les états de l'overlay sur des fonds de jeu simulés. */
export default defineConfig({
  root: import.meta.dirname,
  plugins: [react(), tailwindcss()],
  build: { outDir: "../dist-playground", emptyOutDir: true },
});
