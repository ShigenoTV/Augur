import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Trois fenêtres, trois points d'entrée : la fenêtre principale, l'overlay et le bandeau Ledger.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "chrome120",
    rollupOptions: {
      input: { main: "index.html", overlay: "overlay.html", ledgerBanner: "ledger-banner.html" },
    },
  },
  test: { environment: "jsdom" },
});
