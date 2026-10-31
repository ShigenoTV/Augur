import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Deux fenêtres, deux points d'entrée : la fenêtre principale et l'overlay.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "chrome120",
    rollupOptions: {
      input: { main: "index.html", overlay: "overlay.html" },
    },
  },
  test: { environment: "jsdom" },
});
