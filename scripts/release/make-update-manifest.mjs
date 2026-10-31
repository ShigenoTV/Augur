#!/usr/bin/env node
// Génère le manifeste de mise à jour Tauri 2 (« latest.json ») d'une version Windows.
//
// Usage :
//   node scripts/release/make-update-manifest.mjs \
//     --bundle-dir target/release/bundle --version 0.1.0 \
//     --base-url https://updates.example.com/releases/0.1.0 \
//     [--notes-file CHANGELOG-extrait.md] [--out latest.json]
//
// Le manifeste suit le format « static JSON » du plugin updater de Tauri 2 :
// https://v2.tauri.app/plugin/updater/#static-json-file
// Chaque installeur doit être accompagné de son fichier .sig (minisign), produit
// par `tauri build` quand bundle.createUpdaterArtifacts vaut true et que
// TAURI_SIGNING_PRIVATE_KEY est défini.

import { readFileSync, readdirSync, writeFileSync, existsSync } from "node:fs";
import { join, basename } from "node:path";
import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

/** Cherche l'unique fichier de `dir` qui satisfait `pred`. */
function findOne(dir, pred, label) {
  if (!existsSync(dir)) return null;
  const hits = readdirSync(dir).filter(pred);
  if (hits.length > 1) {
    throw new Error(`plusieurs ${label} dans ${dir} : ${hits.join(", ")}`);
  }
  return hits[0] ?? null;
}

/** Lit la signature minisign associée à un installeur. */
function readSignature(file) {
  const sigPath = `${file}.sig`;
  if (!existsSync(sigPath)) {
    throw new Error(
      `signature de mise à jour absente : ${sigPath} (TAURI_SIGNING_PRIVATE_KEY défini ? bundle.createUpdaterArtifacts = true ?)`,
    );
  }
  const sig = readFileSync(sigPath, "utf8").trim();
  if (!sig) throw new Error(`signature vide : ${sigPath}`);
  return sig;
}

export function buildManifest({ bundleDir, version, baseUrl, notes = "", pubDate = new Date() }) {
  if (!SEMVER.test(version)) throw new Error(`version invalide : ${version}`);
  if (!/^https:\/\//.test(baseUrl)) throw new Error(`l'URL de base doit être en https : ${baseUrl}`);
  const base = baseUrl.replace(/\/+$/, "");

  const nsisDir = join(bundleDir, "nsis");
  const msiDir = join(bundleDir, "msi");
  const nsis = findOne(nsisDir, (f) => f.endsWith("-setup.exe") && f.includes(`_${version}_`), "installeurs NSIS");
  const msi = findOne(msiDir, (f) => f.endsWith(".msi") && f.includes(`_${version}_`), "installeurs MSI");
  if (!nsis && !msi) {
    throw new Error(`aucun installeur pour la version ${version} dans ${bundleDir}`);
  }

  const entry = (dir, file) => ({
    signature: readSignature(join(dir, file)),
    url: `${base}/${encodeURIComponent(basename(file))}`,
  });

  const platforms = {};
  if (nsis) platforms["windows-x86_64-nsis"] = entry(nsisDir, nsis);
  if (msi) platforms["windows-x86_64-msi"] = entry(msiDir, msi);
  // Clé générique : NSIS de préférence (mise à jour par utilisateur, sans UAC).
  platforms["windows-x86_64"] = platforms["windows-x86_64-nsis"] ?? platforms["windows-x86_64-msi"];

  return {
    version,
    notes,
    pub_date: pubDate.toISOString().replace(/\.\d{3}Z$/, "Z"),
    platforms,
  };
}

function main() {
  const { values } = parseArgs({
    options: {
      "bundle-dir": { type: "string" },
      version: { type: "string" },
      "base-url": { type: "string" },
      "notes-file": { type: "string" },
      out: { type: "string", default: "latest.json" },
    },
  });
  for (const k of ["bundle-dir", "version", "base-url"]) {
    if (!values[k]) throw new Error(`argument manquant : --${k}`);
  }
  const notes = values["notes-file"] ? readFileSync(values["notes-file"], "utf8").trim() : "";
  const manifest = buildManifest({
    bundleDir: values["bundle-dir"],
    version: values.version.replace(/^v/, ""),
    baseUrl: values["base-url"],
    notes,
  });
  writeFileSync(values.out, JSON.stringify(manifest, null, 2) + "\n");
  console.log(`manifeste écrit : ${values.out} (${Object.keys(manifest.platforms).join(", ")})`);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    main();
  } catch (err) {
    console.error(`erreur : ${err.message}`);
    process.exit(1);
  }
}
