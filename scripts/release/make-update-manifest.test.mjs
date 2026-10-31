// Tests du générateur de manifeste de mise à jour : node --test scripts/release/
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildManifest } from "./make-update-manifest.mjs";

function fakeBundle({ nsis = true, msi = true, sig = true, version = "0.1.0" } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "augur-bundle-"));
  if (nsis) {
    mkdirSync(join(dir, "nsis"));
    const f = join(dir, "nsis", `Augur_${version}_x64-setup.exe`);
    writeFileSync(f, "exe");
    if (sig) writeFileSync(`${f}.sig`, "SIG-NSIS\n");
  }
  if (msi) {
    mkdirSync(join(dir, "msi"));
    const f = join(dir, "msi", `Augur_${version}_x64_en-US.msi`);
    writeFileSync(f, "msi");
    if (sig) writeFileSync(`${f}.sig`, "SIG-MSI\n");
  }
  return dir;
}

const base = "https://updates.example.test/releases/0.1.0/";
const date = new Date("2026-11-01T10:00:00.123Z");

test("manifeste complet NSIS + MSI", () => {
  const m = buildManifest({ bundleDir: fakeBundle(), version: "0.1.0", baseUrl: base, pubDate: date });
  assert.equal(m.version, "0.1.0");
  assert.equal(m.pub_date, "2026-11-01T10:00:00Z");
  assert.equal(m.platforms["windows-x86_64-nsis"].signature, "SIG-NSIS");
  assert.equal(
    m.platforms["windows-x86_64-nsis"].url,
    "https://updates.example.test/releases/0.1.0/Augur_0.1.0_x64-setup.exe",
  );
  assert.equal(m.platforms["windows-x86_64-msi"].signature, "SIG-MSI");
  assert.deepEqual(m.platforms["windows-x86_64"], m.platforms["windows-x86_64-nsis"]);
});

test("MSI seul : la clé générique pointe vers le MSI", () => {
  const m = buildManifest({ bundleDir: fakeBundle({ nsis: false }), version: "0.1.0", baseUrl: base });
  assert.deepEqual(m.platforms["windows-x86_64"], m.platforms["windows-x86_64-msi"]);
});

test("version bêta acceptée", () => {
  const dir = fakeBundle({ version: "0.2.0-beta.3" });
  const m = buildManifest({ bundleDir: dir, version: "0.2.0-beta.3", baseUrl: base });
  assert.equal(m.version, "0.2.0-beta.3");
});

test("signature absente : échec explicite", () => {
  assert.throws(
    () => buildManifest({ bundleDir: fakeBundle({ sig: false }), version: "0.1.0", baseUrl: base }),
    /signature de mise à jour absente/,
  );
});

test("aucun installeur de la bonne version : échec", () => {
  assert.throws(
    () => buildManifest({ bundleDir: fakeBundle(), version: "0.1.1", baseUrl: base }),
    /aucun installeur/,
  );
});

test("URL non https refusée", () => {
  assert.throws(
    () => buildManifest({ bundleDir: fakeBundle(), version: "0.1.0", baseUrl: "http://x" }),
    /https/,
  );
});

test("version invalide refusée", () => {
  assert.throws(
    () => buildManifest({ bundleDir: fakeBundle(), version: "latest", baseUrl: base }),
    /version invalide/,
  );
});
