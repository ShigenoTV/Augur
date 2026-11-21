#!/usr/bin/env node
// Test complet de la Forge PoE 2 contre l'émulateur de Craft of Exile (beta.craftofexile.com), 21/11/2026.
//
// Les cas (une base par classe d'objet, plusieurs crafts depuis un objet fixé) sont exportés par le test
// `exporter_test_complet_poe2` de `forge::atelier`. Pour chaque cas, on applique le craft N fois dans
// l'émulateur et on mesure la part d'essais où la famille visée est sur l'objet (Désécration : parmi les
// options de la révélation), à comparer au moteur exact et au modèle du solveur d'Augur.
//
//     CAS=cas.json cargo test -p poe2-core --release --lib exporter_test_complet_poe2 -- --ignored
//     node tools/theorycraft/coe_test_complet_poe2.js cas.json [essais] > resultats.json
//
// Utilise le Chromium et le Playwright préinstallés. Aucune donnée de l'émulateur n'est copiée.
const { chromium } = require('playwright');
const fs = require('fs');

async function main() {
  const cas = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
  const essais = Number(process.argv[3] || 10000);
  const navigateur = await chromium.launch();
  const page = await navigateur.newPage();
  await page.goto('https://beta.craftofexile.com/?game=poe2', { waitUntil: 'networkidle', timeout: 120000 });
  const resultats = [];
  for (const c of cas) {
    const r = await page.evaluate(([c, essais]) => {
      const base = coedata.items.entries.find((x) => x.key === c.base);
      if (!base) return { absent: true };
      const idMod = (k) => DataManager.getEntryFromDict(coedata.mods, k)?.id;
      const cle = (id) => DataManager.getEntryFromId(coedata.mods, Number(id))?.key;
      const famille = new Set(c.famille);
      Emulator.setModpool(ModsManager.categorizeModpool(ModsManager.getModpool(base.class, true, true)));
      Notices.show = () => {};
      Emulator.consumeOmen = () => {};
      let vus = 0;
      let echecs = 0;
      for (let n = 0; n < essais; n++) {
        const item = new GameItem(base.id, c.ilvl, null, []);
        item.rarity = c.rarete === 'Rare' ? ItemRarity.RARE : ItemRarity.NORMAL;
        for (const k of c.depart) item.addModifier(idMod(k));
        item.updateItemState(false);
        Emulator.setItem(item);
        const p = new rollAffixParameters();
        if (c.niveau_min > 0) p._minimumModifierLevel = c.niveau_min;
        let ok;
        try {
          switch (c.craft) {
            case 'alchemy': ok = Emulator.poe2_alchemy(p, []); break;
            case 'transmute': ok = Emulator.poe2_transmute(p, []); break;
            case 'exalt':
            case 'exalt_greater': ok = Emulator.exalt(p, false, []); break;
            case 'chaos': ok = Emulator.poe2_chaos(p, []); break;
            case 'desecrate': {
              const h = { gnawed: 'poe2_desecrate_low', preserved: 'poe2_desecrate_mid', ancient: 'poe2_desecrate_high' }[c.os.split('_')[0]];
              ok = Emulator.poe2_desecrate(h, p, ['OmenOnAbyssAddSuffixes']);
              break;
            }
          }
        } catch (e) { ok = String(e); }
        if (ok !== true) { echecs++; continue; }
        let cles = item.getModifiers().map((m) => m.key);
        if (c.craft === 'desecrate') {
          const voile = item.getModifiers().find((m) => m.veiled && !m.revealed);
          if (!voile) { echecs++; continue; }
          cles = Emulator.getRevealPicks(voile.type, voile.params).map(cle);
        }
        if (cles.some((k) => famille.has(k))) vus++;
      }
      return { vus, echecs };
    }, [c, essais]);
    const coe = r.absent ? null : r.vus / essais;
    const sigma = coe === null ? null : Math.sqrt(Math.max(coe * (1 - coe), 1e-12) / essais);
    resultats.push({ classe: c.classe, base: c.base_nom, craft: c.craft, vise: c.vise, moteur: c.augur_moteur, modele: c.augur_modele, emulateur: coe, sigma, echecs: r.echecs ?? null, essais });
    console.error(`${c.classe} ${c.craft} : moteur ${(100 * c.augur_moteur).toFixed(2)} %, modèle ${(100 * c.augur_modele).toFixed(2)} %, émulateur ${coe === null ? 'base absente' : (100 * coe).toFixed(2) + ' %'} (échecs ${r.echecs ?? '-'})`);
  }
  console.log(JSON.stringify(resultats, null, 1));
  await navigateur.close();
}

main().catch((e) => { console.error(e); process.exit(1); });
