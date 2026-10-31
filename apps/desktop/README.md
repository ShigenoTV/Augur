# apps/desktop : application Windows Augur (Tauri 2)

Coquille de l'overlay Upgrade Copilot et de la fenêtre principale.

```bash
pnpm install
pnpm --filter @augur/desktop tauri dev     # application (Windows)
pnpm --filter @augur/desktop dev           # interface seule dans le navigateur (données fictives)
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

| Module Rust | Rôle |
|---|---|
| `clipboard.rs` | Détecte un Ctrl+C sur un objet (compteur de séquence du presse-papier Windows), jeu au premier plan uniquement |
| `game_window.rs` | Titre et classe de la fenêtre au premier plan (Win32), rien d'autre |
| `overlay.rs`, `placement.rs` | Affichage près du curseur sans voler le focus, fermeture à l'éloignement, ajustement de taille |
| `shortcuts.rs` | Échap et Alt+U, enregistrés seulement pendant l'affichage de l'overlay |
| `client_log.rs` | Suivi de `Client.txt` (zones), émet `augur://zone-changed` ; `AUGUR_CLIENT_LOG` force le chemin |
| `updates.rs`, `settings.rs` | Mise à jour signée depuis l'application, canal stable ou bêta |
| `item_text.rs` | Lecture minimale du texte d'objet, **temporaire** jusqu'à l'intégration de `poe2-core` |
| `contracts.rs` | Miroir de `crates/augur-contracts`, **temporaire** jusqu'à sa fusion |

Règles GGG appliquées : aucune touche envoyée au jeu, une action par pression, aucune lecture mémoire, aucune modification des fichiers du jeu, jamais de `POESESSID`. Capabilities minimales par fenêtre (`src-tauri/capabilities/`), CSP stricte, aucune permission fichier, shell ou réseau côté WebView.
